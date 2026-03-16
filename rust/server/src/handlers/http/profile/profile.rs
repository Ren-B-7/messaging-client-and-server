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

use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::{Request, Response, StatusCode};
use multer::Multipart;
use tokio_rusqlite::rusqlite;
use tracing::{error, info, warn};

use shared::types::jwt::JwtClaims;
use shared::types::settings::*;
use shared::types::update::*;

use crate::AppState;
use crate::database::{login, password, register, utils};
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

    let user = match register::get_user_by_id(&state.db, claims.user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return deliver_error_json("NOT_FOUND", "User not found", StatusCode::NOT_FOUND);
        }
        Err(e) => return Err(anyhow::anyhow!("Database error: {}", e)),
    };

    // Resolve avatar URL — returns a usable path the browser can GET directly.
    let avatar_url = register::get_user_avatar(&state.db, claims.user_id)
        .await
        .ok()
        .flatten()
        .map(|_| format!("/api/avatar/{}", claims.user_id));

    let profile_json = serde_json::json!({
        "status": "success",
        "data": {
            "user_id":    user.id,
            "username":   user.username,
            "email":      user.email,
            "is_admin":   claims.is_admin,
            "created_at": user.created_at,
            "avatar_url": avatar_url,
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

    let update_data = match parse_update_body(req).await {
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

async fn parse_update_body(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<UpdateProfileData, ProfileError> {
    let body = req
        .collect()
        .await
        .map_err(|_| ProfileError::InternalError)?
        .to_bytes();

    serde_json::from_slice::<UpdateProfileData>(&body).map_err(|e| {
        error!("Failed to parse admin login JSON: {}", e);
        ProfileError::InternalError
    })
}

async fn update_user_profile(
    user_id: i64,
    data: &UpdateProfileData,
    state: &AppState,
) -> std::result::Result<(), ProfileError> {
    if let Some(ref new_username) = data.username {
        if !utils::is_valid_username(new_username) {
            return Err(ProfileError::InvalidUsername);
        }

        let exists = register::username_exists(&state.db, new_username.clone())
            .await
            .map_err(|e| {
                error!("Database error checking username: {}", e);
                ProfileError::DatabaseError
            })?;

        if exists {
            let current_user = register::get_user_by_id(&state.db, user_id)
                .await
                .map_err(|_| ProfileError::DatabaseError)?
                .ok_or(ProfileError::UserNotFound)?;

            if &current_user.username != new_username {
                return Err(ProfileError::UsernameTaken);
            }
        } else {
            register::update_username(&state.db, user_id, new_username.clone())
                .await
                .map_err(|e| {
                    error!("Database error updating username: {}", e);
                    ProfileError::DatabaseError
                })?;
        }
    }

    if let Some(ref new_email) = data.email {
        if !utils::is_valid_email(new_email) {
            return Err(ProfileError::InvalidEmail);
        }

        let exists = register::email_exists(&state.db, new_email.clone())
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

    match login::delete_session_by_id(&state.db, claims.session_id).await {
        Ok(_) => info!("Session deleted on logout"),
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
/// Hard-auth: `user_id` and `claims` are pre-verified by the router.
pub async fn handle_logout_all(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
    _claims: JwtClaims,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing logout-all for user {}", user_id);
    let secure_cookie = is_https(&req);
    match login::delete_all_user_sessions(&state.db, user_id).await {
        Ok(_) => info!("All sessions deleted for user {}", user_id),
        Err(e) => error!("Failed to delete sessions for user {}: {}", user_id, e),
    }
    let clear_cookie = create_session_cookie("auth_id", "", secure_cookie)
        .context("Failed to create clear-cookie header")?;
    let response_body = SettingsResponse::Success {
        message: "Logged out of all sessions successfully".to_string(),
    };
    Ok(deliver_serialized_json_with_cookie(
        &response_body,
        StatusCode::OK,
        clear_cookie,
    )?)
}

async fn parse_password_form(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<ChangePasswordData, SettingsError> {
    let body = req
        .collect()
        .await
        .map_err(|_| SettingsError::InternalError)?
        .to_bytes();

    serde_json::from_slice::<ChangePasswordData>(&body).map_err(|e| {
        error!("Failed to parse change password JSON: {}", e);
        SettingsError::InternalError
    })
}

fn validate_password_change(data: &ChangePasswordData) -> std::result::Result<(), SettingsError> {
    if data.new_password != data.confirm_password {
        return Err(SettingsError::PasswordMismatch);
    }
    if data.current_password == data.new_password {
        return Err(SettingsError::SamePassword);
    }
    if !utils::is_strong_password(&data.new_password) {
        return Err(SettingsError::PasswordTooWeak);
    }
    Ok(())
}

async fn change_user_password(
    user_id: i64,
    data: &ChangePasswordData,
    state: &AppState,
) -> std::result::Result<(), SettingsError> {
    let current_hash = password::get_password_hash(&state.db, user_id)
        .await
        .map_err(|e| {
            error!("Database error getting password hash: {}", e);
            SettingsError::DatabaseError
        })?
        .ok_or(SettingsError::DatabaseError)?;

    let current_valid =
        utils::verify_password(&current_hash, &data.current_password).map_err(|e| {
            error!("Password verification error: {}", e);
            SettingsError::InternalError
        })?;

    if !current_valid {
        warn!("Invalid current password for user {}", user_id);
        return Err(SettingsError::InvalidCurrentPassword);
    }

    let new_hash = utils::hash_password(&data.new_password).map_err(|e| {
        error!("Failed to hash new password: {}", e);
        SettingsError::InternalError
    })?;

    password::change_password(&state.db, user_id, new_hash)
        .await
        .map_err(|e| {
            error!("Database error updating password: {}", e);
            SettingsError::DatabaseError
        })?;

    // Revoke all sessions — any stolen token is now dead
    login::delete_all_user_sessions(&state.db, user_id)
        .await
        .map_err(|e| {
            error!("Failed to revoke sessions after password change: {}", e);
            SettingsError::DatabaseError
        })?;

    Ok(())
}
// ===========================================================================
// delete
// ===========================================================================

/// DELETE /api/user — permanently delete the authenticated user's own account.
///
/// Hard-auth: `user_id` and `claims` are pre-verified by the router (JWT + DB + IP).
/// Sequence:
///   1. Revoke every session for this user so no token can be reused.
///   2. Delete the user row (cascades to any FK-constrained child rows).
///   3. Clear the auth cookie so the browser drops the session immediately.
pub async fn handle_delete_profile(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
    _claims: JwtClaims,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing account deletion for user {}", user_id);

    let secure_cookie = is_https(&req);

    // Revoke all sessions first so any in-flight requests are dead.
    match login::delete_all_user_sessions(&state.db, user_id).await {
        Ok(_) => info!("All sessions revoked for deleted user {}", user_id),
        Err(e) => error!("Failed to revoke sessions for user {}: {}", user_id, e),
    }

    // Delete the user row.
    state
        .db
        .call(move |conn| {
            conn.execute("DELETE FROM users WHERE id = ?1", [user_id])?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .context("Failed to delete user account")?;

    info!("Account deleted for user {}", user_id);

    let clear_cookie = create_session_cookie("auth_id", "", secure_cookie)
        .context("Failed to create clear-cookie header")?;

    let response_body = SettingsResponse::Success {
        message: "Account deleted successfully".to_string(),
    };

    Ok(deliver_serialized_json_with_cookie(
        &response_body,
        StatusCode::OK,
        clear_cookie,
    )?)
}

// ===========================================================================
// avatar — upload
// ===========================================================================

/// POST /api/profile/avatar — replace the authenticated user's profile picture.
///
/// Accepts `multipart/form-data` with a single field named `avatar`.
/// Allowed types: JPEG, PNG, GIF, WebP.  Hard cap: 5 MiB.
///
/// Hard-auth: `user_id` is pre-verified by the router (JWT + DB + IP).
pub async fn handle_upload_avatar(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Avatar upload request from user {}", user_id);

    const MAX_AVATAR_BYTES: usize = 5 * 1024 * 1024; // 5 MiB

    // ── Parse multipart boundary ─────────────────────────────────────────────
    let content_type = req
        .headers()
        .get(hyper::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let boundary = multer::parse_boundary(content_type)
        .map_err(|e| anyhow::anyhow!("Invalid multipart boundary: {}", e))?;

    let body_stream = req.into_body().into_data_stream();
    let mut multipart = Multipart::new(body_stream, boundary);

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut detected_ext: Option<String> = None;

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| anyhow::anyhow!("Multipart read error: {}", e))?
    {
        if field.name().unwrap_or("") != "avatar" {
            // Drain unknown fields silently.
            while field
                .chunk()
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?
                .is_some()
            {}
            continue;
        }

        // Derive extension from MIME type reported by the client.
        let mime = field.content_type().map(|m| m.to_string());
        detected_ext = Some(
            match mime.as_deref().unwrap_or("") {
                "image/jpeg" | "image/jpg" => "jpg",
                "image/png" => "png",
                "image/gif" => "gif",
                "image/webp" => "webp",
                other => {
                    // Reject non-image content types up front.
                    if !other.is_empty() {
                        return deliver_error_json(
                            "INVALID_TYPE",
                            "Avatar must be a JPEG, PNG, GIF, or WebP image",
                            StatusCode::BAD_REQUEST,
                        );
                    }
                    "jpg" // fallback when client omits Content-Type
                }
            }
            .to_string(),
        );

        let mut buf = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(|e| anyhow::anyhow!("{}", e))? {
            buf.extend_from_slice(&chunk);
            if buf.len() > MAX_AVATAR_BYTES {
                return deliver_error_json(
                    "FILE_TOO_LARGE",
                    "Avatar must not exceed 5 MiB",
                    StatusCode::PAYLOAD_TOO_LARGE,
                );
            }
        }
        file_bytes = Some(buf);
    }

    let file_bytes = match file_bytes {
        Some(b) if !b.is_empty() => b,
        _ => {
            return deliver_error_json(
                "MISSING_FILE",
                "No avatar data received — send a field named 'avatar'",
                StatusCode::BAD_REQUEST,
            );
        }
    };

    let ext = detected_ext.unwrap_or_else(|| "jpg".to_string());

    // ── Write new file to disk ───────────────────────────────────────────────
    let uploads_dir = state.config.read().await.paths.uploads_dir.clone();
    let avatars_dir = format!("{}/avatars", uploads_dir);

    tokio::fs::create_dir_all(&avatars_dir)
        .await
        .context("Failed to create avatars directory")?;

    // Deterministic filename: <user_id>.<ext>
    // Writing this path overwrites the old avatar automatically when the
    // extension matches.  If the extension changed we delete the old file
    // first so stale files don't accumulate on disk.
    let filename = format!("{}.{}", user_id, ext);
    let new_path = format!("{}/{}", avatars_dir, filename);

    tokio::fs::write(&new_path, &file_bytes)
        .await
        .with_context(|| format!("Failed to write avatar to {}", new_path))?;

    // ── Swap out old file (best-effort) ──────────────────────────────────────
    if let Ok(Some(old_path)) = register::get_user_avatar(&state.db, user_id).await {
        if let Err(e) = tokio::fs::remove_file(&old_path).await {
            warn!("Could not remove old avatar {:?}: {}", old_path, e);
        }
    }

    // ── Persist new path ─────────────────────────────────────────────────────
    register::set_user_avatar(&state.db, user_id, new_path)
        .await
        .context("Failed to update avatar path in DB")?;

    info!("Avatar updated for user {}", user_id);

    deliver_serialized_json(
        &serde_json::json!({
            "status":     "success",
            "message":    "Avatar updated successfully",
            "avatar_url": format!("/api/avatar/{}", user_id),
        }),
        StatusCode::OK,
    )
}

// ===========================================================================
// avatar — serve
// ===========================================================================

/// GET /api/avatar/:user_id — serve a user's avatar image.
///
/// Returns 404 when the user has no avatar set, allowing the frontend to
/// fall back to initials gracefully.
///
/// Light-auth: `claims` are pre-verified by the router (JWT only).
/// The auth cookie is sent automatically on same-origin `<img>` requests.
pub async fn handle_get_avatar(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    _claims: JwtClaims,
    target_user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    // Look up stored path
    let avatar_path = match register::get_user_avatar(&state.db, target_user_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return deliver_error_json(
                "NOT_FOUND",
                "This user has no avatar",
                StatusCode::NOT_FOUND,
            );
        }
        Err(e) => return Err(anyhow::anyhow!("DB error fetching avatar: {}", e)),
    };

    // Read bytes from disk
    let bytes = match tokio::fs::read(&avatar_path).await {
        Ok(b) => b,
        Err(e) => {
            error!("Avatar file missing from disk ({:?}): {}", avatar_path, e);
            // Treat a missing file as "no avatar" — clean up the stale DB row.
            let _ = register::clear_user_avatar(&state.db, target_user_id).await;
            return deliver_error_json("NOT_FOUND", "Avatar not found", StatusCode::NOT_FOUND);
        }
    };

    // Derive MIME type from extension
    let mime = match std::path::Path::new(&avatar_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(hyper::header::CONTENT_TYPE, mime)
        // Cache for 5 minutes — short enough to show a fresh upload quickly,
        // long enough to avoid hammering the disk on every re-render.
        .header(hyper::header::CACHE_CONTROL, "public, max-age=300")
        .body(Full::new(Bytes::from(bytes)).boxed())
        .context("Failed to build avatar response")?)
}
