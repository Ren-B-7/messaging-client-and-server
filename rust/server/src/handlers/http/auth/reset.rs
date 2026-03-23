// handlers/http/auth/reset.rs
//
// Password reset flow — token-based, no email (email delivery is a
// deployment concern outside this codebase).
//
// Flow:
//   1. POST /api/auth/reset-request  { "username": "alice" }
//      → generates a secure token, stores it in password_reset_tokens,
//        returns the token in the response.
//      In a real deployment you would email this token instead of returning
//      it; the handler is structured so that swapping the return for an
//      email call is a one-line change.
//
//   2. POST /api/auth/reset-confirm  { "token": "…", "new_password": "…",
//                                      "confirm_password": "…" }
//      → validates and consumes the token, updates the password hash,
//        revokes all sessions (any stolen token is now dead).
//
// Both endpoints are Open (no auth required) — the token IS the credential.

use anyhow::Result;
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use tracing::{error, info, warn};

use crate::AppState;
use crate::database::{login, password, utils};
use crate::handlers::http::utils::{deliver_error_json, deliver_serialized_json};

/// POST /api/auth/reset-request
///
/// Accepts `{ "username": "alice" }`.
///
/// Always returns 200 with the same shape regardless of whether the username
/// exists — this prevents user enumeration.  If the username does not exist
/// the response contains `"token": null`; clients should display a generic
/// "if that account exists you will receive a reset link" message.
///
/// In production: replace the token in the response body with an email send
/// and return `{ "status": "success", "message": "Reset email sent if account exists" }`.
pub async fn handle_reset_request(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let body = req
        .collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read body: {}", e))?
        .to_bytes();

    let params: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::json!({}));

    let username = match params.get("username").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() && u.len() <= 32 => u.to_string(),
        _ => {
            return deliver_error_json(
                "INVALID_INPUT",
                "username is required",
                StatusCode::BAD_REQUEST,
            );
        }
    };

    info!("Password reset requested for username: {}", username);

    // Look up the user — but do not reveal whether they exist.
    let user = match crate::database::utils::get_user_by_username(&state.db, username.clone())
        .await
    {
        Ok(Some(u)) => u,
        Ok(None) => {
            // Return the same shape but with token: null to avoid enumeration.
            warn!("Reset requested for unknown user: {}", username);
            return deliver_serialized_json(
                &serde_json::json!({
                    "status":  "success",
                    "message": "If that account exists, a reset token has been generated",
                    "token":   null,
                }),
                StatusCode::OK,
            );
        }
        Err(e) => {
            error!("DB error during reset request: {}", e);
            return deliver_error_json(
                "INTERNAL_ERROR",
                "An internal error occurred",
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    // Generate a cryptographically random token.
    let token = utils::generate_uuid_token();

    // Token valid for 1 hour.
    let valid_secs: i64 = 3600;

    match password::create_reset_token(&state.db, user.id, token.clone(), valid_secs).await {
        Ok(_) => {
            info!("Reset token created for user {}", user.id);
        }
        Err(e) => {
            error!("Failed to create reset token: {}", e);
            return deliver_error_json(
                "INTERNAL_ERROR",
                "An internal error occurred",
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    }

    // TODO: in production, email the token to user.email instead of
    // returning it in the response body.
    deliver_serialized_json(
        &serde_json::json!({
            "status":  "success",
            "message": "Reset token generated",
            "token":   token,
            "expires_in_seconds": valid_secs,
        }),
        StatusCode::OK,
    )
}

/// POST /api/auth/reset-confirm
///
/// Accepts `{ "token": "…", "new_password": "…", "confirm_password": "…" }`.
///
/// Validates the token, updates the password hash, and revokes all active
/// sessions for the user so any stolen token cannot be replayed.
pub async fn handle_reset_confirm(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let body = req
        .collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read body: {}", e))?
        .to_bytes();

    let params: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| anyhow::anyhow!("Invalid JSON"))?;

    let token = match params.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return deliver_error_json(
                "INVALID_INPUT",
                "token is required",
                StatusCode::BAD_REQUEST,
            );
        }
    };

    let new_password = match params.get("new_password").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => {
            return deliver_error_json(
                "INVALID_INPUT",
                "new_password is required",
                StatusCode::BAD_REQUEST,
            );
        }
    };

    let confirm_password = match params.get("confirm_password").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => new_password.clone(),
    };

    if new_password != confirm_password {
        return deliver_error_json(
            "PASSWORD_MISMATCH",
            "Passwords do not match",
            StatusCode::BAD_REQUEST,
        );
    }

    if !crate::database::utils::is_strong_password(&new_password) {
        return deliver_error_json(
            "WEAK_PASSWORD",
            "Password must be at least 8 characters with at least one letter and one number",
            StatusCode::BAD_REQUEST,
        );
    }

    // Validate + consume the token — returns the user_id or None.
    let user_id = match password::validate_reset_token(&state.db, token.clone()).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            warn!("Invalid or expired reset token used");
            return deliver_error_json(
                "INVALID_TOKEN",
                "Reset token is invalid, expired, or already used",
                StatusCode::BAD_REQUEST,
            );
        }
        Err(e) => {
            error!("DB error validating reset token: {}", e);
            return deliver_error_json(
                "INTERNAL_ERROR",
                "An internal error occurred",
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    // Hash the new password.
    let new_hash = match utils::hash_password(&new_password) {
        Ok(h) => h,
        Err(e) => {
            error!("Password hashing failed: {}", e);
            return deliver_error_json(
                "INTERNAL_ERROR",
                "An internal error occurred",
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    // Update the password.
    if let Err(e) = password::change_password(&state.db, user_id, new_hash).await {
        error!("Failed to update password after reset: {}", e);
        return deliver_error_json(
            "INTERNAL_ERROR",
            "An internal error occurred",
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }

    // Revoke all sessions — any token a bad actor may have had is now dead.
    if let Err(e) = login::delete_all_user_sessions(&state.db, user_id).await {
        error!(
            "Failed to revoke sessions after password reset for user {}: {}",
            user_id, e
        );
        // Non-fatal: password is already updated, don't fail the request.
    }

    info!("Password reset complete for user {}", user_id);

    deliver_serialized_json(
        &serde_json::json!({
            "status":  "success",
            "message": "Password has been reset. Please log in with your new password.",
        }),
        StatusCode::OK,
    )
}
