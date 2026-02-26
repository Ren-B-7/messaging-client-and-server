// handlers/http/profile.rs
//
// Consolidates get, update, and settings into one flat file so all imports
// are shared and the compiler sees a single translation unit.
//
// Public surface:
//
//   handle_get_profile(req, state, claims)            Light-auth (JWT only)
//   handle_update_profile(req, state, user_id)        Hard-auth  (JWT + DB + IP)
//   handle_change_password(req, state, user_id)       Hard-auth
//   handle_logout(req, state, user_id, claims)        Hard-auth
//   handle_logout_all(req, state, user_id)            Hard-auth
//
// Auth is performed by the router before any handler is called.
// No handler touches decode_jwt_claims or validate_jwt_secure internally.

use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::{Request, Response, StatusCode};
use tokio_rusqlite::rusqlite;
use tracing::{error, info, warn};

use shared::types::jwt::JwtClaims;
use shared::types::settings::*;
use shared::types::update::*;

use crate::AppState;
use crate::handlers::http::utils::{
    create_session_cookie, deliver_error_json, deliver_serialized_json,
    deliver_serialized_json_with_cookie, is_https,
};

// ===========================================================================
// get
// ===========================================================================

/// GET /api/profile — return the authenticated user's profile.
///
/// Light-auth: `claims` are pre-verified by the router (JWT only, no DB).
/// A DB read is still needed to fetch email / created_at, but auth itself
/// costs nothing.
pub async fn handle_get_profile(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing get profile for user {}", claims.user_id);

    use crate::database::register as db_register;

    let user = match db_register::get_user_by_id(&state.db, claims.user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return deliver_error_json("NOT_FOUND", "User not found", StatusCode::NOT_FOUND);
        }
        Err(e) => return Err(anyhow::anyhow!("Database error: {}", e)),
    };

    let profile_json = serde_json::json!({
        "status": "success",
        "data": {
            "user_id":    user.id,
            "username":   user.username,
            "email":      user.email,
            "is_admin":   claims.is_admin,
            "created_at": user.created_at,
        }
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(profile_json.to_string())).boxed())
        .context("Failed to build profile response")?)
}

// ===========================================================================
// update
// ===========================================================================

/// PUT /api/profile  or  POST /api/profile/update — update the user's profile.
///
/// Hard-auth: `user_id` is pre-verified by the router (JWT + DB + IP).
pub async fn handle_update_profile(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing update profile for user {}", user_id);

    let update_data = match parse_update_form(req).await {
        Ok(data) => data,
        Err(err) => {
            warn!("Profile update parsing failed: {:?}", err.to_code());
            return deliver_serialized_json(&err.to_update_response(), StatusCode::BAD_REQUEST);
        }
    };

    match update_user_profile(user_id, &update_data, &state).await {
        Ok(_) => {
            info!("Profile updated for user {}", user_id);
            deliver_serialized_json(
                &UpdateResponse::Success {
                    message: "Profile updated successfully".to_string(),
                },
                StatusCode::OK,
            )
        }
        Err(err) => {
            error!("Failed to update profile: {:?}", err.to_code());
            deliver_serialized_json(&err.to_update_response(), StatusCode::BAD_REQUEST)
        }
    }
}

async fn parse_update_form(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<UpdateProfileData, ProfileError> {
    let body = req
        .collect()
        .await
        .map_err(|_| ProfileError::InternalError)?
        .to_bytes();

    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    let username = params
        .get("username")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let email = params
        .get("email")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(UpdateProfileData { username, email })
}

async fn update_user_profile(
    user_id: i64,
    data: &UpdateProfileData,
    state: &AppState,
) -> std::result::Result<(), ProfileError> {
    use crate::database::register as db_register;

    if let Some(ref new_username) = data.username {
        if !crate::database::utils::is_valid_username(new_username) {
            return Err(ProfileError::InvalidUsername);
        }

        let exists = db_register::username_exists(&state.db, new_username.clone())
            .await
            .map_err(|e| {
                error!("Database error checking username: {}", e);
                ProfileError::DatabaseError
            })?;

        if exists {
            let current_user = db_register::get_user_by_id(&state.db, user_id)
                .await
                .map_err(|_| ProfileError::DatabaseError)?
                .ok_or(ProfileError::UserNotFound)?;

            if &current_user.username != new_username {
                return Err(ProfileError::UsernameTaken);
            }
        } else {
            db_register::update_username(&state.db, user_id, new_username.clone())
                .await
                .map_err(|e| {
                    error!("Database error updating username: {}", e);
                    ProfileError::DatabaseError
                })?;
        }
    }

    if let Some(ref new_email) = data.email {
        if !crate::database::utils::is_valid_email(new_email) {
            return Err(ProfileError::InvalidEmail);
        }

        let exists = db_register::email_exists(&state.db, new_email.clone())
            .await
            .map_err(|e| {
                error!("Database error checking email: {}", e);
                ProfileError::DatabaseError
            })?;

        if exists {
            return Err(ProfileError::EmailTaken);
        }

        let email_to_set = new_email.clone();
        state
            .db
            .call(move |conn| {
                conn.execute(
                    "UPDATE users SET email = ?1 WHERE id = ?2",
                    rusqlite::params![email_to_set, user_id],
                )?;
                Ok::<_, rusqlite::Error>(())
            })
            .await
            .map_err(|e| {
                error!("Database error updating email: {}", e);
                ProfileError::DatabaseError
            })?;
    }

    Ok(())
}

// ===========================================================================
// settings
// ===========================================================================

/// POST /api/settings/password — change the authenticated user's password.
///
/// Hard-auth: `user_id` is pre-verified by the router (JWT + DB + IP).
pub async fn handle_change_password(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing change password for user {}", user_id);

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
            info!("Password changed for user {}", user_id);
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

/// POST /api/logout — invalidate the current session.
///
/// Hard-auth: `user_id` and `claims` are pre-verified by the router.
/// `claims.session_id` is the revocation key — the router already confirmed
/// this session exists in the DB, so the delete is guaranteed to hit a real row.
pub async fn handle_logout(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    _user_id: i64,
    claims: JwtClaims,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing logout for session {}", claims.session_id);

    let secure_cookie = is_https(&req);

    use crate::database::login as db_login;
    match db_login::delete_session_by_id(&state.db, claims.session_id).await {
        Ok(_)  => info!("Session deleted on logout"),
        Err(e) => error!("Failed to delete session: {}", e),
    }

    let clear_cookie = create_session_cookie("auth_id", "", secure_cookie)
        .context("Failed to create clear-cookie header")?;

    let response_body = SettingsResponse::Success {
        message: "Logged out successfully".to_string(),
    };
    Ok(deliver_serialized_json_with_cookie(
        &response_body,
        StatusCode::OK,
        clear_cookie,
    )?)
}

/// POST /api/settings/logout-all — revoke every session for this user.
///
/// Hard-auth: `user_id` is pre-verified by the router (JWT + DB + IP).
pub async fn handle_logout_all(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing logout-all for user {}", user_id);

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
            error!("Failed to delete sessions for user {}: {}", user_id, e);
            deliver_serialized_json(
                &SettingsError::DatabaseError.to_response(),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

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

fn validate_password_change(
    data: &ChangePasswordData,
) -> std::result::Result<(), SettingsError> {
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

    let new_hash = crate::database::utils::hash_password(&data.new_password).map_err(|e| {
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

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    // ── update: form parsing ──────────────────────────────────────────────────

    #[test]
    fn parse_update_both_fields() {
        let body = b"username=alice&email=alice@example.com";
        let params: std::collections::HashMap<String, String> =
            form_urlencoded::parse(body.as_ref()).into_owned().collect();
        let username = params
            .get("username")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let email = params
            .get("email")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        assert_eq!(username, Some("alice".to_string()));
        assert_eq!(email, Some("alice@example.com".to_string()));
    }

    #[test]
    fn parse_update_username_only() {
        let body = b"username=bob";
        let params: std::collections::HashMap<String, String> =
            form_urlencoded::parse(body.as_ref()).into_owned().collect();
        let username = params
            .get("username")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let email = params
            .get("email")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        assert_eq!(username, Some("bob".to_string()));
        assert!(email.is_none());
    }

    #[test]
    fn parse_update_empty_fields_become_none() {
        let body = b"username=&email=";
        let params: std::collections::HashMap<String, String> =
            form_urlencoded::parse(body.as_ref()).into_owned().collect();
        let username = params
            .get("username")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let email = params
            .get("email")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        assert!(username.is_none(), "empty username should be None");
        assert!(email.is_none(), "empty email should be None");
    }

    #[test]
    fn parse_update_whitespace_trimmed() {
        let body = b"username=%20alice%20";
        let params: std::collections::HashMap<String, String> =
            form_urlencoded::parse(body.as_ref()).into_owned().collect();
        let username = params
            .get("username")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        assert_eq!(username, Some("alice".to_string()));
    }
}
