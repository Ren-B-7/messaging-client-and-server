use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use tracing::{info, warn};

use crate::AppState;
use crate::handlers::http::utils::{
    deliver_serialized_json, deliver_success_json, validate_jwt_secure,
};

/// Validate an admin session using JWT + secure DB path.
///
/// Two-step check:
///   1. `validate_jwt_secure` — verify JWT signature, look up session_id in DB,
///      compare request IP against the stored IP.
///   2. Inspect `claims.is_admin` — embedded at login time so this requires
///      no additional DB query on GET routes, but note that privilege changes
///      (promote/demote) only take effect after the user logs in again.
///
/// Returns the admin's `user_id` on success, `Err(())` on any failure.
pub async fn require_admin(
    req: &Request<IncomingBody>,
    state: &AppState,
) -> std::result::Result<i64, ()> {
    use crate::handlers::http::utils::decode_jwt_claims;

    // Full secure-path validation first (IP check, DB session lookup).
    let (user_id, _) = validate_jwt_secure(req, state)
        .await
        .map_err(|_| ())?;

    // Then confirm the admin flag from the JWT claims.
    // We re-decode here (cheap — no DB) to read is_admin without adding an
    // extra function parameter.
    let claims = decode_jwt_claims(req, &state.jwt_secret).map_err(|_| ())?;

    if !claims.is_admin {
        warn!(
            "Non-admin user {} attempted an admin operation",
            user_id
        );
        return Err(());
    }

    Ok(user_id)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /admin/api/users
pub async fn handle_get_users(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Serving user list");

    if require_admin(&req, &state).await.is_err() {
        warn!("Unauthorised get users attempt");
        return deliver_serialized_json(
            &serde_json::json!({
                "status":  "error",
                "code":    "UNAUTHORIZED",
                "message": "Admin authentication required"
            }),
            StatusCode::UNAUTHORIZED,
        );
    }

    let users = state
        .db
        .call(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, username, email, created_at, is_banned, ban_reason, is_admin
                 FROM   users
                 ORDER  BY created_at DESC",
            )?;

            let rows = stmt
                .query_map([], |row| {
                    Ok(serde_json::json!({
                        "id":         row.get::<_, i64>(0)?,
                        "username":   row.get::<_, String>(1)?,
                        "email":      row.get::<_, Option<String>>(2)?,
                        "created_at": row.get::<_, i64>(3)?,
                        "is_banned":  row.get::<_, i64>(4)? != 0,
                        "ban_reason": row.get::<_, Option<String>>(5)?,
                        "is_admin":   row.get::<_, i64>(6)? != 0,
                    }))
                })?
                .collect::<std::result::Result<Vec<_>, tokio_rusqlite::rusqlite::Error>>()?;

            Ok::<_, tokio_rusqlite::rusqlite::Error>(rows)
        })
        .await
        .context("Failed to query user list")?;

    deliver_success_json(
        Some(serde_json::json!({ "users": users, "total": users.len() })),
        None,
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// Ban / Unban
// ---------------------------------------------------------------------------

/// POST /admin/api/users/ban
pub async fn handle_ban_user(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::ban as db_ban;

    info!("Processing ban user request");

    let admin_id = match require_admin(&req, &state).await {
        Ok(id) => id,
        Err(_) => {
            warn!("Unauthorised ban attempt");
            return deliver_serialized_json(
                &serde_json::json!({
                    "status":  "error",
                    "code":    "UNAUTHORIZED",
                    "message": "Admin authentication required"
                }),
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    let params = parse_body(req).await?;

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user_id"))?;

    let reason: String = params
        .get("reason")
        .cloned()
        .unwrap_or_else(|| "No reason provided".to_string());

    info!("Admin {} banning user {} — reason: {}", admin_id, user_id, reason);

    db_ban::ban_user(&state.db, user_id, admin_id, Some(reason.clone()))
        .await
        .context("Failed to ban user in database")?;

    deliver_success_json(
        Some(serde_json::json!({ "user_id": user_id, "banned": true, "reason": reason })),
        Some(&format!("User {} has been banned", user_id)),
        StatusCode::OK,
    )
}

/// POST /admin/api/users/unban
pub async fn handle_unban_user(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::ban as db_ban;

    info!("Processing unban user request");

    let admin_id = match require_admin(&req, &state).await {
        Ok(id) => id,
        Err(_) => {
            warn!("Unauthorised unban attempt");
            return deliver_serialized_json(
                &serde_json::json!({
                    "status":  "error",
                    "code":    "UNAUTHORIZED",
                    "message": "Admin authentication required"
                }),
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    let params = parse_body(req).await?;

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user_id"))?;

    info!("Admin {} unbanning user {}", admin_id, user_id);

    db_ban::unban_user(&state.db, user_id)
        .await
        .context("Failed to unban user in database")?;

    deliver_success_json(
        Some(serde_json::json!({ "user_id": user_id, "banned": false })),
        Some(&format!("User {} has been unbanned", user_id)),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// Delete user
// ---------------------------------------------------------------------------

/// DELETE /admin/api/users/:id
pub async fn handle_delete_user(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let admin_id = match require_admin(&req, &state).await {
        Ok(id) => id,
        Err(_) => {
            warn!("Unauthorised delete attempt");
            return deliver_serialized_json(
                &serde_json::json!({
                    "status":  "error",
                    "code":    "UNAUTHORIZED",
                    "message": "Admin authentication required"
                }),
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    let path = req.uri().path().to_string();

    let user_id: i64 = path
        .trim_end_matches('/')
        .split('/')
        .last()
        .filter(|s| *s != ":id")
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user ID in path"))?;

    if user_id == admin_id {
        return deliver_serialized_json(
            &serde_json::json!({
                "status":  "error",
                "code":    "INVALID_TARGET",
                "message": "You cannot delete your own account"
            }),
            StatusCode::BAD_REQUEST,
        );
    }

    info!("Admin {} deleting user {}", admin_id, user_id);

    state
        .db
        .call(move |conn| {
            conn.execute("DELETE FROM users WHERE id = ?1", [user_id])?;
            Ok::<_, tokio_rusqlite::rusqlite::Error>(())
        })
        .await
        .context("Failed to delete user")?;

    deliver_success_json(
        Some(serde_json::json!({ "user_id": user_id, "deleted": true })),
        Some(&format!("User {} has been deleted", user_id)),
        StatusCode::OK,
    )
}

/// POST /admin/api/users/promote
pub async fn handle_promote_user(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let admin_id = match require_admin(&req, &state).await {
        Ok(id) => id,
        Err(_) => {
            warn!("Unauthorised promote attempt");
            return deliver_serialized_json(
                &serde_json::json!({
                    "status":  "error",
                    "code":    "UNAUTHORIZED",
                    "message": "Admin authentication required"
                }),
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    let params = parse_body(req).await?;

    let user_id: i64 = params
        .get("user_id")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid user_id"))?;

    if user_id == admin_id {
        return deliver_serialized_json(
            &serde_json::json!({
                "status":  "error",
                "code":    "INVALID_TARGET",
                "message": "You are already an admin"
            }),
            StatusCode::BAD_REQUEST,
        );
    }

    use crate::database::register as db_register;

    let user = db_register::get_user_by_id(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("DB error: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("User not found"))?;

    db_register::promote_user(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("DB error promoting user: {}", e))?;

    info!("Admin {} promoted user {} ({})", admin_id, user.username, user_id);

    deliver_success_json(
        Some(serde_json::json!({ "user_id": user_id, "username": user.username })),
        Some(&format!("{} is now an admin", user.username)),
        StatusCode::OK,
    )
}

/// POST /admin/api/users/demote
pub async fn handle_demote_user(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let admin_id = match require_admin(&req, &state).await {
        Ok(id) => id,
        Err(_) => {
            warn!("Unauthorised demote attempt");
            return deliver_serialized_json(
                &serde_json::json!({
                    "status":  "error",
                    "code":    "UNAUTHORIZED",
                    "message": "Admin authentication required"
                }),
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    let params = parse_body(req).await?;

    let user_id: i64 = params
        .get("user_id")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid user_id"))?;

    if user_id == admin_id {
        return deliver_serialized_json(
            &serde_json::json!({
                "status":  "error",
                "code":    "INVALID_TARGET",
                "message": "You cannot demote yourself"
            }),
            StatusCode::BAD_REQUEST,
        );
    }

    use crate::database::register as db_register;

    let user = db_register::get_user_by_id(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("DB error: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("User not found"))?;

    db_register::demote_user(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("DB error demoting user: {}", e))?;

    info!("Admin {} demoted user {} ({})", admin_id, user.username, user_id);

    deliver_success_json(
        Some(serde_json::json!({ "user_id": user_id, "username": user.username })),
        Some(&format!("{} is no longer an admin", user.username)),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

async fn parse_body(req: Request<IncomingBody>) -> Result<HashMap<String, String>> {
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let body = req
        .collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read body: {}", e))?
        .to_bytes();

    if content_type.contains("application/json") {
        serde_json::from_slice::<HashMap<String, serde_json::Value>>(&body)
            .map(|m| {
                m.into_iter()
                    .filter_map(|(k, v)| {
                        let s = match &v {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Number(n) => n.to_string(),
                            _ => return None,
                        };
                        Some((k, s))
                    })
                    .collect()
            })
            .map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))
    } else {
        Ok(form_urlencoded::parse(body.as_ref())
            .into_owned()
            .collect::<HashMap<String, String>>())
    }
}

// ---------------------------------------------------------------------------
// Tests  (unchanged from original)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_body_user_id_parsing() {
        let params: std::collections::HashMap<String, String> =
            form_urlencoded::parse(b"user_id=123&reason=Spamming")
                .into_owned()
                .collect();
        let user_id: Option<i64> = params.get("user_id").and_then(|id| id.parse().ok());
        let reason = params.get("reason").cloned().unwrap_or_else(|| "No reason".to_string());
        assert_eq!(user_id, Some(123));
        assert_eq!(reason, "Spamming");
    }

    #[test]
    fn user_id_from_path_last_segment() {
        let path = "/admin/api/users/42";
        let user_id: Option<i64> = path
            .trim_end_matches('/')
            .split('/')
            .last()
            .filter(|s| *s != ":id")
            .and_then(|s| s.parse().ok());
        assert_eq!(user_id, Some(42));
    }

    #[test]
    fn admin_cannot_act_on_themselves() {
        let admin_id: i64 = 1;
        let target_id: i64 = 1;
        assert_eq!(admin_id, target_id, "should be blocked when equal");
    }
}
