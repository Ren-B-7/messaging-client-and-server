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
    create_session_cookie, decode_jwt_claims, deliver_serialized_json,
    deliver_serialized_json_with_cookie, is_https, validate_jwt_secure,
};

/// Change password handler.
pub async fn handle_change_password(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing change password request");

    // SECURE PATH: state-changing — validate IP/UA via DB.
    let user_id = match validate_jwt_secure(&req, &state).await {
        Ok((id, _)) => id,
        Err(_) => {
            warn!("Unauthorized password change attempt");
            return deliver_serialized_json(
                &SettingsError::Unauthorized.to_response(),
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    let password_data = match parse_password_form(req).await {
        Ok(data) => data,
        Err(err) => {
            warn!("Password change parsing failed: {:?}", err.to_code());
            return deliver_serialized_json(&err.to_response(), StatusCode::BAD_REQUEST);
        }
    };

    if let Err(err) = validate_password_change(&password_data) {
        warn!("Password change validation failed: {:?}", err.to_code());
        return deliver_serialized_json(&err.to_response(), StatusCode::BAD_REQUEST);
    }

    match change_user_password(user_id, &password_data, &state).await {
        Ok(_) => {
            info!("Password changed successfully for user {}", user_id);
            deliver_serialized_json(
                &SettingsResponse::Success {
                    message: "Password changed successfully".to_string(),
                },
                StatusCode::OK,
            )
        }
        Err(err) => {
            error!("Failed to change password: {:?}", err.to_code());
            deliver_serialized_json(&err.to_response(), StatusCode::BAD_REQUEST)
        }
    }
}

/// Logout handler — delete the session row so the JWT's `session_id` becomes
/// invalid even before the token expires.
pub async fn handle_logout(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing logout request");

    let secure_cookie = is_https(&req);

    // Decode the JWT to extract the session_id revocation handle.
    // We don't need the full secure-path DB check here — we just want the
    // session_id so we can delete it.  An invalid/expired JWT simply means
    // there's nothing to delete, so we respond OK either way.
    let session_id_opt = decode_jwt_claims(&req, &state.jwt_secret)
        .ok()
        .map(|c| c.session_id);

    if let Some(session_id) = session_id_opt {
        use crate::database::login as db_login;
        match db_login::delete_session_by_id(&state.db, session_id).await {
            Ok(_) => info!("Session deleted on logout"),
            Err(e) => error!("Failed to delete session: {}", e),
        }
    } else {
        warn!("Logout with no valid JWT — nothing to delete");
    }

    // Clear the auth_id cookie regardless.
    let clear_cookie = create_session_cookie("auth_id", "", secure_cookie)
        .context("Failed to create clear-cookie header")?;

    let response_body = SettingsResponse::Success {
        message: "Logged out successfully".to_string(),
    };
    Ok(deliver_serialized_json_with_cookie(&response_body, StatusCode::OK, clear_cookie)?)
}

/// Logout all devices — delete every session row for this user.
pub async fn handle_logout_all(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing logout all devices request");

    // SECURE PATH: state-changing.
    let user_id = match validate_jwt_secure(&req, &state).await {
        Ok((id, _)) => id,
        Err(_) => {
            warn!("Unauthorized logout-all attempt");
            return deliver_serialized_json(
                &SettingsError::Unauthorized.to_response(),
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    use crate::database::login as db_login;

    match db_login::delete_all_user_sessions(&state.db, user_id).await {
        Ok(_) => {
            info!("All sessions deleted for user {}", user_id);
            deliver_serialized_json(
                &SettingsResponse::Success {
                    message: "Logged out from all devices".to_string(),
                },
                StatusCode::OK,
            )
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

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

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

fn validate_password_change(data: &ChangePasswordData) -> std::result::Result<(), SettingsError> {
    if data.new_password != data.confirm_password {
        return Err(SettingsError::PasswordMismatch);
    }
    if data.current_password == data.new_password {
        return Err(SettingsError::SamePassword);
    }
    if !crate::database::utils::is_strong_password(&data.new_password) {
        return Err(SettingsError::PasswordTooWeak);
    }
    Ok(())
}

async fn change_user_password(
    user_id: i64,
    data: &ChangePasswordData,
    state: &AppState,
) -> std::result::Result<(), SettingsError> {
    use crate::database::password as db_password;

    let current_hash = db_password::get_password_hash(&state.db, user_id)
        .await
        .map_err(|e| {
            error!("Database error getting password hash: {}", e);
            SettingsError::DatabaseError
        })?
        .ok_or(SettingsError::DatabaseError)?;

    let current_valid =
        crate::database::utils::verify_password(&current_hash, &data.current_password)
            .map_err(|e| {
                error!("Password verification error: {}", e);
                SettingsError::InternalError
            })?;

    if !current_valid {
        warn!("Invalid current password for user {}", user_id);
        return Err(SettingsError::InvalidCurrentPassword);
    }

    let new_hash =
        crate::database::utils::hash_password(&data.new_password).map_err(|e| {
            error!("Failed to hash new password: {}", e);
            SettingsError::InternalError
        })?;

    db_password::change_password(&state.db, user_id, new_hash)
        .await
        .map_err(|e| {
            error!("Database error updating password: {}", e);
            SettingsError::DatabaseError
        })?;

    Ok(())
}
