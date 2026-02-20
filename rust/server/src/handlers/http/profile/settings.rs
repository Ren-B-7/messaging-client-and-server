use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{Request, Response, StatusCode};
use tracing::{error, info, warn};

pub use shared::types::settings::*;

use crate::AppState;
use crate::handlers::http::utils::{
    create_session_cookie, deliver_serialized_json, deliver_serialized_json_with_cookie,
};

/// Change password handler
pub async fn handle_change_password(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing change password request");

    // Extract user_id from session
    let user_id = match extract_user_from_request(&req, &state).await {
        Ok(id) => id,
        Err(err) => {
            warn!("Unauthorized password change attempt");
            return deliver_serialized_json(&err.to_response(), StatusCode::UNAUTHORIZED);
        }
    };

    // Parse password change data
    let password_data = match parse_password_form(req).await {
        Ok(data) => data,
        Err(err) => {
            warn!("Password change parsing failed: {:?}", err.to_code());
            return deliver_serialized_json(&err.to_response(), StatusCode::BAD_REQUEST);
        }
    };

    // Validate passwords
    if let Err(err) = validate_password_change(&password_data) {
        warn!("Password change validation failed: {:?}", err.to_code());
        return deliver_serialized_json(&err.to_response(), StatusCode::BAD_REQUEST);
    }

    // Attempt password change
    match change_user_password(user_id, &password_data, &state).await {
        Ok(_) => {
            info!("Password changed successfully for user {}", user_id);

            let response = SettingsResponse::Success {
                message: "Password changed successfully".to_string(),
            };

            deliver_serialized_json(&response, StatusCode::OK)
        }
        Err(err) => {
            error!("Failed to change password: {:?}", err.to_code());
            deliver_serialized_json(&err.to_response(), StatusCode::BAD_REQUEST)
        }
    }
}

/// Logout handler (delete session)
pub async fn handle_logout(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing logout request");

    // Extract token from request
    let token = match extract_token_from_request(&req) {
        Some(t) => t,
        None => {
            warn!("Logout attempt without token");
            return deliver_serialized_json(
                &SettingsResponse::Success {
                    message: "Logged out".to_string(),
                },
                StatusCode::OK,
            );
        }
    };

    // Delete session
    use crate::database::login as db_login;

    match db_login::delete_session(&state.db, token.to_string()).await {
        Ok(_) => {
            info!("User logged out successfully");

            // Create cookie to clear auth token
            let clear_cookie = create_session_cookie("auth_id", "", true)
                .context("Failed to create session instance cookie")?;

            let response_body = SettingsResponse::Success {
                message: "Logged out successfully".to_string(),
            };
            let response =
                deliver_serialized_json_with_cookie(&response_body, StatusCode::OK, clear_cookie)
                    .unwrap();

            Ok(response)
        }
        Err(e) => {
            error!("Failed to delete session: {}", e);
            deliver_serialized_json(
                &SettingsError::DatabaseError.to_response(),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

/// Logout all devices handler
pub async fn handle_logout_all(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing logout all devices request");

    // Extract user_id from session
    let user_id = match extract_user_from_request(&req, &state).await {
        Ok(id) => id,
        Err(err) => {
            warn!("Unauthorized logout all attempt");
            return deliver_serialized_json(&err.to_response(), StatusCode::UNAUTHORIZED);
        }
    };

    // Delete all user sessions
    use crate::database::login as db_login;

    match db_login::delete_all_user_sessions(&state.db, user_id).await {
        Ok(_) => {
            info!("All sessions deleted for user {}", user_id);

            let response = SettingsResponse::Success {
                message: "Logged out from all devices".to_string(),
            };

            deliver_serialized_json(&response, StatusCode::OK)
        }
        Err(e) => {
            error!("Failed to delete sessions: {}", e);
            deliver_serialized_json(
                &SettingsError::DatabaseError.to_response(),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

/// Extract authenticated user from request
async fn extract_user_from_request(
    req: &Request<hyper::body::Incoming>,
    state: &AppState,
) -> std::result::Result<i64, SettingsError> {
    use crate::database::login as db_login;

    let token = extract_token_from_request(req).ok_or(SettingsError::Unauthorized)?;

    // Validate session token
    let user_id = db_login::validate_session(&state.db, token.to_string())
        .await
        .map_err(|_| SettingsError::DatabaseError)?
        .ok_or(SettingsError::Unauthorized)?;

    Ok(user_id)
}

/// Extract token from request headers
fn extract_token_from_request(req: &Request<hyper::body::Incoming>) -> Option<&str> {
    req.headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .or_else(|| {
            req.headers()
                .get("cookie")
                .and_then(|h| h.to_str().ok())
                .and_then(|cookies| {
                    cookies
                        .split(';')
                        .find(|c| c.trim().starts_with("auth_id="))
                        .and_then(|c| c.split('=').nth(1))
                })
        })
}

/// Parse password change form
async fn parse_password_form(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<ChangePasswordData, SettingsError> {
    let body = req
        .collect()
        .await
        .map_err(|_| SettingsError::InternalError)?
        .to_bytes();

    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    let current_password = params
        .get("current_password")
        .ok_or(SettingsError::MissingField("current_password".to_string()))?
        .to_string();

    let new_password = params
        .get("new_password")
        .ok_or(SettingsError::MissingField("new_password".to_string()))?
        .to_string();

    let confirm_password = params
        .get("confirm_password")
        .ok_or(SettingsError::MissingField("confirm_password".to_string()))?
        .to_string();

    Ok(ChangePasswordData {
        current_password,
        new_password,
        confirm_password,
    })
}

/// Validate password change data
fn validate_password_change(data: &ChangePasswordData) -> std::result::Result<(), SettingsError> {
    // Check if new passwords match
    if data.new_password != data.confirm_password {
        return Err(SettingsError::PasswordMismatch);
    }

    // Check if new password is different from current
    if data.current_password == data.new_password {
        return Err(SettingsError::SamePassword);
    }

    // Validate new password strength
    if !crate::database::utils::is_strong_password(&data.new_password) {
        return Err(SettingsError::PasswordTooWeak);
    }

    Ok(())
}

/// Change user password in database
async fn change_user_password(
    user_id: i64,
    data: &ChangePasswordData,
    state: &AppState,
) -> std::result::Result<(), SettingsError> {
    use crate::database::password as db_password;

    // Get current password hash
    let current_hash = db_password::get_password_hash(&state.db, user_id)
        .await
        .map_err(|e| {
            error!("Database error getting password hash: {}", e);
            SettingsError::DatabaseError
        })?
        .ok_or(SettingsError::DatabaseError)?;

    // Verify current password
    let current_valid =
        crate::database::utils::verify_password(&current_hash, &data.current_password).map_err(
            |e| {
                error!("Password verification error: {}", e);
                SettingsError::InternalError
            },
        )?;

    if !current_valid {
        warn!("Invalid current password for user {}", user_id);
        return Err(SettingsError::InvalidCurrentPassword);
    }

    // Hash new password
    let new_hash = crate::database::utils::hash_password(&data.new_password).map_err(|e| {
        error!("Failed to hash new password: {}", e);
        SettingsError::InternalError
    })?;

    // Update password
    db_password::change_password(&state.db, user_id, new_hash)
        .await
        .map_err(|e| {
            error!("Database error updating password: {}", e);
            SettingsError::DatabaseError
        })?;

    // Optionally: Delete all other sessions to force re-login
    // use crate::database::login as db_login;
    // db_login::delete_all_user_sessions(&state.db, user_id).await.ok();

    Ok(())
}
