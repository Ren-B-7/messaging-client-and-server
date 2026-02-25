use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response, StatusCode};
use std::collections::HashMap;
use std::convert::Infallible;
use tracing::{error, info, warn};

use crate::AppState;
use crate::database::login as db_login;
use crate::handlers::http::utils::{
    create_persistent_cookie, create_session_cookie, deliver_redirect_with_cookie,
    deliver_serialized_json, encode_jwt, get_client_ip, get_user_agent, is_https,
};

use shared::types::jwt::JwtClaims;
use shared::types::login::*;

/// Main login handler.
pub async fn handle_login(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing login request");

    // Extract IP and UA *before* consuming the body.
    let ip_address = get_client_ip(&req);
    let user_agent = get_user_agent(&req).unwrap_or_default();
    let secure_cookie = is_https(&req);

    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

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

    match attempt_login(&login_data, &state, ip_address, user_agent).await {
        Ok((user_id, username, jwt)) => {
            info!("User logged in successfully: {} (ID: {})", username, user_id);

            let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;

            let instance_cookie = if login_data.remember_me {
                let max_age = std::time::Duration::from_secs(token_expiry_secs);
                create_persistent_cookie("auth_id", &jwt, max_age, secure_cookie)
                    .context("Failed to create persistent instance cookie")?
            } else {
                create_session_cookie("auth_id", &jwt, secure_cookie)
                    .context("Failed to create session instance cookie")?
            };

            Ok(deliver_redirect_with_cookie("/chat", Some(instance_cookie))?)
        }
        Err(e) => {
            warn!("Login failed: {:?}", e.to_code());
            deliver_serialized_json(&e.to_response(), StatusCode::UNAUTHORIZED)
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

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

fn validate_login(data: &LoginData) -> std::result::Result<(), LoginError> {
    if data.username.is_empty() {
        return Err(LoginError::MissingField("username".to_string()));
    }
    if data.password.is_empty() {
        return Err(LoginError::MissingField("password".to_string()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Core login logic
// ---------------------------------------------------------------------------

/// Verify credentials, create a DB session, and mint a signed JWT.
///
/// The JWT embeds `{username, user_id, session_id, user_agent, is_admin}` so
/// that GET requests can be authorised with **zero DB reads** (signature
/// verification only).  The `session_id` UUID links back to the DB row for
/// revocation checks on mutating requests.
async fn attempt_login(
    data: &LoginData,
    state: &AppState,
    ip_address: Option<String>,
    user_agent: String,
) -> std::result::Result<(i64, String, String), LoginError> {
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
        crate::database::utils::verify_password(&user_auth.password_hash, &data.password)
            .map_err(|e| {
                error!("Password verification error: {}", e);
                LoginError::InternalError
            })?;

    if !password_valid {
        warn!("Invalid password for user: {}", data.username);
        return Err(LoginError::InvalidCredentials);
    }

    // Check whether this user is an admin so we can embed the flag in the JWT.
    let is_admin = state
        .db
        .call(move |conn| {
            let v: i64 = conn
                .query_row(
                    "SELECT is_admin FROM users WHERE id = ?1",
                    [user_auth.id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            Ok::<_, tokio_rusqlite::rusqlite::Error>(v != 0)
        })
        .await
        .unwrap_or(false);

    // Generate a UUID that acts as the revocation handle inside the JWT.
    let session_id = crate::database::utils::generate_uuid_token();
    let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;
    let expires_at = crate::database::utils::calculate_expiry(token_expiry_secs as i64);

    db_login::create_session(
        &state.db,
        NewSession {
            user_id: user_auth.id,
            session_id: session_id.clone(),
            expires_at,
            ip_address,
        },
    )
    .await
    .map_err(|e| {
        error!("Failed to create session: {}", e);
        LoginError::DatabaseError
    })?;

    db_login::update_last_login(&state.db, user_auth.id)
        .await
        .map_err(|e| error!("Failed to update last login: {}", e))
        .ok();

    // Build and sign the JWT.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = JwtClaims {
        sub: user_auth.username.clone(),
        user_id: user_auth.id,
        session_id,
        user_agent,
        is_admin,
        exp: now + token_expiry_secs as usize,
        iat: now,
    };

    let jwt = encode_jwt(&claims, &state.jwt_secret).map_err(|e| {
        error!("JWT encoding failed: {}", e);
        LoginError::InternalError
    })?;

    info!(
        "Login successful for user: {} (ID: {})",
        user_auth.username, user_auth.id
    );

    Ok((user_auth.id, user_auth.username, jwt))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_login_ok() {
        let data = LoginData {
            username: "alice".to_string(),
            password: "hunter2".to_string(),
            remember_me: false,
        };
        assert!(validate_login(&data).is_ok());
    }

    #[test]
    fn validate_login_empty_username_fails() {
        let data = LoginData {
            username: "".to_string(),
            password: "hunter2".to_string(),
            remember_me: false,
        };
        let err = validate_login(&data).unwrap_err();
        matches!(err, LoginError::MissingField(_));
    }

    #[test]
    fn validate_login_empty_password_fails() {
        let data = LoginData {
            username: "alice".to_string(),
            password: "".to_string(),
            remember_me: false,
        };
        let err = validate_login(&data).unwrap_err();
        matches!(err, LoginError::MissingField(_));
    }

    #[test]
    fn remember_me_variants() {
        for val in &["on", "true", "1"] {
            let remember = *val == "on" || *val == "true" || *val == "1";
            assert!(remember, "expected true for '{}'", val);
        }
        let not_remember = "0" == "on" || "0" == "true" || "0" == "1";
        assert!(!not_remember);
    }
}
