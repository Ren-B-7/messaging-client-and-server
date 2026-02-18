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
    create_persistent_cookie, create_session_cookie, deliver_redirect_with_cookie,
    deliver_serialized_json,
};
use shared::types::login::*;

/// Login request data (supports both form-encoded and JSON)
/// Main login handler
pub async fn handle_login(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing login request");

    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let login_data = if content_type.contains("application/json") {
        match parse_login_json(req).await {
            Ok(data) => data,
            Err(e) => {
                warn!("Login JSON parsing failed: {:?}", e.to_code());
                return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
            }
        }
    } else {
        match parse_login_form(req).await {
            Ok(data) => data,
            Err(e) => {
                warn!("Login form parsing failed: {:?}", e.to_code());
                return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
            }
        }
    };

    if let Err(e) = validate_login(&login_data) {
        warn!("Login validation failed: {:?}", e.to_code());
        return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
    }

    match attempt_login(&login_data, &state).await {
        Ok((user_id, username, token)) => {
            info!(
                "User logged in successfully: {} (ID: {})",
                username, user_id
            );

            let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;

            // The session token is stored in the cookie so the user is authenticated
            // on the /chat page they're being redirected to.
            let instance_cookie = if login_data.remember_me {
                let max_age = std::time::Duration::from_secs(token_expiry_secs);
                create_persistent_cookie("instance_id", &token, max_age, true)
                    .context("Failed to create persistent instance cookie")?
            } else {
                create_session_cookie("instance_id", &token, true)
                    .context("Failed to create session instance cookie")?
            };

            // Redirect to /chat with the session cookie set
            let response = deliver_redirect_with_cookie("/chat", Some(instance_cookie))?;

            Ok(response)
        }
        Err(e) => {
            warn!("Login failed: {:?}", e.to_code());
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
        error!("Failed to parse login JSON: {}", e);
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

/// Attempt to log in the user using the database
async fn attempt_login(
    data: &LoginData,
    state: &AppState,
) -> std::result::Result<(i64, String, String), LoginError> {
    use crate::database::login as db_login;

    info!("Attempting login for user: {}", data.username);

    let user_auth = db_login::get_user_auth(&state.db, data.username.clone())
        .await
        .map_err(|e| {
            error!("Database error getting user auth: {}", e);
            LoginError::DatabaseError
        })?
        .ok_or_else(|| {
            warn!("User not found: {}", data.username);
            LoginError::InvalidCredentials
        })?;

    if user_auth.is_banned {
        warn!("Banned user attempted login: {}", data.username);
        return Err(LoginError::UserBanned);
    }

    let password_valid =
        crate::database::utils::verify_password(&user_auth.password_hash, &data.password).map_err(
            |e| {
                error!("Password verification error: {}", e);
                LoginError::InternalError
            },
        )?;

    if !password_valid {
        warn!("Invalid password for user: {}", data.username);
        return Err(LoginError::InvalidCredentials);
    }

    let token = crate::database::utils::generate_uuid_token();
    let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;
    let expires_at = crate::database::utils::calculate_expiry(token_expiry_secs as i64);

    db_login::create_session(
        &state.db,
        db_login::NewSession {
            user_id: user_auth.id,
            session_token: token.clone(),
            expires_at,
            ip_address: None,
            user_agent: None,
        },
    )
    .await
    .map_err(|e| {
        error!("Failed to create session: {}", e);
        LoginError::DatabaseError
    })?;

    db_login::update_last_login(&state.db, user_auth.id)
        .await
        .map_err(|e| {
            error!("Failed to update last login: {}", e);
        })
        .ok();

    info!(
        "Login successful for user: {} (ID: {})",
        user_auth.username, user_auth.id
    );

    Ok((user_auth.id, user_auth.username, token))
}
