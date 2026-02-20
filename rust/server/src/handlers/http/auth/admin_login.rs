use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response, StatusCode};
use std::collections::HashMap;
use std::convert::Infallible;
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::http::utils::{
    self, deliver_serialized_json, deliver_serialized_json_with_cookie,
};
use shared::types::login::*;

/// Main admin login handler
pub async fn handle_login(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing admin login request");

    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let login_data = if content_type.contains("application/json") {
        match parse_login_json(req).await {
            Ok(data) => data,
            Err(e) => {
                warn!("Admin login JSON parsing failed: {:?}", e.to_code());
                return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
            }
        }
    } else {
        match parse_login_form(req).await {
            Ok(data) => data,
            Err(e) => {
                warn!("Admin login form parsing failed: {:?}", e.to_code());
                return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
            }
        }
    };

    if let Err(e) = validate_login(&login_data) {
        warn!("Admin login validation failed: {:?}", e.to_code());
        return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
    }

    match attempt_login(&login_data, &state).await {
        Ok((user_id, username, token)) => {
            info!(
                "Admin logged in successfully: {} (ID: {})",
                username, user_id
            );

            let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;

            // The session token is stored both in the cookie and returned in the JSON
            // body so the frontend can send it as a Bearer header on subsequent requests.
            let instance_cookie = if login_data.remember_me {
                let max_age = std::time::Duration::from_secs(token_expiry_secs);
                utils::create_persistent_cookie("auth_id", &token, max_age, true)
                    .context("Failed to create persistent instance cookie")?
            } else {
                utils::create_session_cookie("auth_id", &token, true)
                    .context("Failed to create session instance cookie")?
            };

            let response_data = LoginResponse::Success {
                user_id,
                username,
                token,
                expires_in: token_expiry_secs,
                message: "Admin login successful".to_string(),
            };

            Ok(deliver_serialized_json_with_cookie(
                &response_data,
                StatusCode::OK,
                instance_cookie,
            )?)
        }
        Err(e) => {
            warn!("Admin login failed: {:?}", e.to_code());
            deliver_serialized_json(&e.to_response(), StatusCode::UNAUTHORIZED)
        }
    }
}

/// Parse login JSON data
async fn parse_login_json(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<LoginData, LoginError> {
    let body = req
        .collect()
        .await
        .map_err(|_| LoginError::InternalError)?
        .to_bytes();

    serde_json::from_slice::<LoginData>(&body).map_err(|e| {
        error!("Failed to parse admin login JSON: {}", e);
        LoginError::InternalError
    })
}

/// Parse login form data
async fn parse_login_form(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<LoginData, LoginError> {
    let body = req
        .collect()
        .await
        .map_err(|_| LoginError::InternalError)?
        .to_bytes();

    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    let username = params
        .get("username")
        .or_else(|| params.get("email"))
        .ok_or(LoginError::MissingField("username".to_string()))?
        .trim()
        .to_string();

    let password = params
        .get("password")
        .ok_or(LoginError::MissingField("password".to_string()))?
        .to_string();

    let remember_me = params
        .get("remember_me")
        .or_else(|| params.get("remember"))
        .map(|v| v == "on" || v == "true" || v == "1")
        .unwrap_or(false);

    Ok(LoginData {
        username,
        password,
        remember_me,
    })
}

/// Validate login data
fn validate_login(data: &LoginData) -> std::result::Result<(), LoginError> {
    if data.username.is_empty() {
        return Err(LoginError::MissingField("username".to_string()));
    }
    if data.password.is_empty() {
        return Err(LoginError::MissingField("password".to_string()));
    }
    Ok(())
}

/// Attempt to log in the admin user using the database
async fn attempt_login(
    data: &LoginData,
    state: &AppState,
) -> std::result::Result<(i64, String, String), LoginError> {
    use crate::database::login as db_login;

    info!("Attempting admin login for user: {}", data.username);

    let admin_auth = db_login::get_admin_auth(&state.db, data.username.clone())
        .await
        .map_err(|e| {
            error!("Database error getting admin auth: {}", e);
            LoginError::DatabaseError
        })?
        .ok_or_else(|| {
            warn!("Admin not found: {}", data.username);
            LoginError::InvalidCredentials
        })?;

    if admin_auth.is_banned {
        warn!("Banned admin attempted login: {}", data.username);
        return Err(LoginError::UserBanned);
    }

    let password_valid =
        crate::database::utils::verify_password(&admin_auth.password_hash, &data.password)
            .map_err(|e| {
                error!("Password verification error: {}", e);
                LoginError::InternalError
            })?;

    if !password_valid {
        warn!("Invalid password for admin: {}", data.username);
        return Err(LoginError::InvalidCredentials);
    }

    let token = crate::database::utils::generate_session_token();
    let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;
    let expires_at = crate::database::utils::calculate_expiry(token_expiry_secs as i64);

    db_login::create_admin_session(
        &state.db,
        db_login::NewSession {
            user_id: admin_auth.id,
            session_token: token.clone(),
            expires_at,
            ip_address: None,
            user_agent: None,
        },
    )
    .await
    .map_err(|e| {
        error!("Failed to create admin session: {}", e);
        LoginError::DatabaseError
    })?;

    db_login::update_admin_last_login(&state.db, admin_auth.id)
        .await
        .map_err(|e| {
            error!("Failed to update admin last login: {}", e);
        })
        .ok();

    info!(
        "Admin login successful for user: {} (ID: {})",
        admin_auth.username, admin_auth.id
    );

    Ok((admin_auth.id, admin_auth.username, token))
}
