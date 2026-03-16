use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::http::utils::{
    create_persistent_cookie, create_session_cookie, deliver_redirect_with_cookie,
    deliver_serialized_json, encode_jwt, get_client_ip, get_user_agent, is_https,
};
use shared::types::jwt::JwtClaims;
use shared::types::login::*;

/// POST /admin/api/login
pub async fn handle_login(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing admin login request");

    let ip_address = get_client_ip(&req);
    let user_agent = get_user_agent(&req).unwrap_or_default();
    let secure_cookie = is_https(&req);

    let login_data = match parse_body(req).await {
        Ok(data) => data,
        Err(e) => {
            warn!("Admin login parsing failed: {:?}", e.to_code());
            return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
        }
    };

    if let Err(e) = validate_login(&login_data) {
        warn!("Admin login validation failed: {:?}", e.to_code());
        return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
    }

    match attempt_login(&login_data, &state, ip_address, user_agent).await {
        Ok((user_id, username, jwt)) => {
            info!(
                "Admin logged in successfully: {} (ID: {})",
                username, user_id
            );

            let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;

            // JWT is stored in the cookie so subsequent requests are authenticated.
            let instance_cookie = if login_data.remember_me {
                let max_age = std::time::Duration::from_secs(token_expiry_secs);
                create_persistent_cookie("auth_id", &jwt, max_age, secure_cookie)
                    .context("Failed to create persistent instance cookie")?
            } else {
                create_session_cookie("auth_id", &jwt, secure_cookie)
                    .context("Failed to create session instance cookie")?
            };

            Ok(deliver_redirect_with_cookie(
                "/admin",
                Some(instance_cookie),
            )?)
        }
        Err(e) => {
            warn!("Admin login failed: {:?}", e.to_code());
            deliver_serialized_json(&e.to_response(), StatusCode::UNAUTHORIZED)
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing / validation
// ---------------------------------------------------------------------------

async fn parse_body(
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

fn validate_login(data: &LoginData) -> std::result::Result<(), LoginError> {
    if data.username.is_empty() {
        return Err(LoginError::MissingField("username".to_string()));
    }
    if data.username.len() > 32 {
        return Err(LoginError::MissingField("username".to_string()));
    }
    if data.password.is_empty() {
        return Err(LoginError::MissingField("password".to_string()));
    }
    if data.password.len() > 1024 {
        return Err(LoginError::MissingField("password".to_string()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Core admin login logic
// ---------------------------------------------------------------------------

/// Verify admin credentials, create a DB session, and mint a signed JWT with
/// `is_admin: true`.
async fn attempt_login(
    data: &LoginData,
    state: &AppState,
    ip_address: Option<String>,
    user_agent: String,
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

    let session_id = crate::database::utils::generate_uuid_token();
    let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;
    let expires_at = crate::database::utils::calculate_expiry(token_expiry_secs as i64);

    db_login::create_admin_session(
        &state.db,
        NewSession {
            user_id: admin_auth.id,
            session_id: session_id.clone(),
            expires_at,
            ip_address,
        },
    )
    .await
    .map_err(|e| {
        error!("Failed to create admin session: {}", e);
        LoginError::DatabaseError
    })?;

    db_login::update_admin_last_login(&state.db, admin_auth.id)
        .await
        .map_err(|e| error!("Failed to update admin last login: {}", e))
        .ok();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = JwtClaims {
        sub: admin_auth.username.clone(),
        user_id: admin_auth.id,
        session_id,
        user_agent,
        is_admin: true,
        exp: now + token_expiry_secs as usize,
        iat: now,
    };

    let jwt = encode_jwt(&claims, &state.jwt_secret).map_err(|e| {
        error!("JWT encoding failed: {}", e);
        LoginError::InternalError
    })?;

    info!(
        "Admin login successful for user: {} (ID: {})",
        admin_auth.username, admin_auth.id
    );

    Ok((admin_auth.id, admin_auth.username, jwt))
}
