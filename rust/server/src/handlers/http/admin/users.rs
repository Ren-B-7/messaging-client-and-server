use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use tracing::info;

use crate::AppState;
use crate::database::{ban, register, utils};
use crate::handlers::http::routes::PathParams;
use crate::handlers::http::utils::json_response::*;

use sqlx::FromRow;

#[derive(FromRow)]
struct UserRow {
    id: i64,
    username: String,
    email: Option<String>,
    created_at: i64,
    is_banned: i64, // SQLite BOOLEAN is i64
    ban_reason: Option<String>,
    is_admin: i64,
}

/// GET /admin/api/users — list all users.
pub async fn handle_get_users(
    _req: Request<IncomingBody>,
    state: AppState,
    _admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Serving user list");

    let rows: Vec<UserRow> = sqlx::query_as(
        "SELECT id, username, email, created_at, is_banned, ban_reason, is_admin
         FROM   users
         ORDER  BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    .context("Failed to query user list")?;

    let users: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id":         r.id,
                "username":   r.username,
                "email":      r.email,
                "created_at": r.created_at,
                "is_banned":  r.is_banned != 0,
                "ban_reason": r.ban_reason,
                "is_admin":   r.is_admin != 0,
            })
        })
        .collect();

    deliver_success_json(
        Some(serde_json::json!({ "users": users, "total": users.len() })),
        None,
        StatusCode::OK,
    )
}

/// GET /admin/api/sessions — list all active sessions.
pub async fn handle_get_sessions(
    _req: Request<IncomingBody>,
    state: AppState,
    _admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Serving session list");

    let now = utils::get_timestamp();

    let rows: Vec<(i64, i64, String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT s.id, s.user_id, u.username, s.ip_address,
                s.created_at, s.expires_at
         FROM   sessions s
         JOIN   users u ON u.id = s.user_id
         WHERE  s.expires_at > ?
         ORDER  BY s.created_at DESC",
    )
    .bind(now)
    .fetch_all(&state.db)
    .await
    .context("Failed to query sessions")?;

    let sessions: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id":         r.0,
                "user_id":    r.1,
                "username":   r.2,
                "ip_address": r.3,
                "created_at": r.4,
                "expires_at": r.5,
            })
        })
        .collect();

    deliver_success_json(
        Some(serde_json::json!({ "sessions": sessions, "total": sessions.len() })),
        None,
        StatusCode::OK,
    )
}

/// POST /admin/api/users/ban — ban a user.
pub async fn handle_ban_user(
    req: Request<IncomingBody>,
    state: AppState,
    admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Admin {} processing ban request", admin_id);

    let params = parse_body(req).await?;

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user_id"))?;

    let reason: String = params
        .get("reason")
        .cloned()
        .unwrap_or_else(|| "No reason provided".to_string());

    let reason = reason.replace('\0', "");
    let reason = if reason.len() > 500 {
        reason.chars().take(500).collect()
    } else {
        reason
    };

    info!(
        "Admin {} banning user {} — reason: {}",
        admin_id, user_id, reason
    );

    ban::ban_user(&state.db, user_id, admin_id, Some(reason.clone()))
        .await
        .context("Failed to ban user in database")?;

    deliver_success_json(
        Some(serde_json::json!({
            "user_id": user_id,
            "banned":  true,
            "reason":  reason,
        })),
        Some(&format!("User {} has been banned", user_id)),
        StatusCode::OK,
    )
}

/// POST /admin/api/users/unban — unban a user.
pub async fn handle_unban_user(
    req: Request<IncomingBody>,
    state: AppState,
    admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Admin {} processing unban request", admin_id);

    let params = parse_body(req).await?;

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user_id"))?;

    info!("Admin {} unbanning user {}", admin_id, user_id);

    ban::unban_user(&state.db, user_id)
        .await
        .context("Failed to unban user in database")?;

    deliver_success_json(
        Some(serde_json::json!({ "user_id": user_id, "banned": false })),
        Some(&format!("User {} has been unbanned", user_id)),
        StatusCode::OK,
    )
}

/// DELETE /admin/api/users/:id — permanently delete a user.
pub async fn handle_delete_user(
    req: Request<IncomingBody>,
    state: AppState,
    admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let user_id: i64 = req
        .extensions()
        .get::<PathParams>()
        .and_then(|p| p.get_i64("id"))
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user ID in path"))?;

    if user_id == admin_id {
        return deliver_serialized_json(
            &serde_json::json!({
                "status":  "error",
                "code":    "INVALID_TARGET",
                "message": "You cannot delete your own account",
            }),
            StatusCode::BAD_REQUEST,
        );
    }

    info!("Admin {} deleting user {}", admin_id, user_id);

    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&state.db)
        .await
        .context("Failed to delete user")?;

    deliver_success_json(
        Some(serde_json::json!({ "user_id": user_id, "deleted": true })),
        Some(&format!("User {} has been deleted", user_id)),
        StatusCode::OK,
    )
}

/// POST /admin/api/users/promote — grant admin privileges to a user.
pub async fn handle_promote_user(
    req: Request<IncomingBody>,
    state: AppState,
    admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
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
                "message": "You are already an admin",
            }),
            StatusCode::BAD_REQUEST,
        );
    }

    let user = utils::get_user_by_id(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("DB error: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("User not found"))?;

    register::promote_user(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("DB error promoting user: {}", e))?;

    info!(
        "Admin {} promoted user {} ({})",
        admin_id, user.username, user_id
    );

    deliver_success_json(
        Some(serde_json::json!({
            "user_id":  user_id,
            "username": user.username,
        })),
        Some(&format!("{} is now an admin", user.username)),
        StatusCode::OK,
    )
}

/// POST /admin/api/users/demote — revoke admin privileges from a user.
pub async fn handle_demote_user(
    req: Request<IncomingBody>,
    state: AppState,
    admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
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
                "message": "You cannot demote yourself",
            }),
            StatusCode::BAD_REQUEST,
        );
    }

    let user = utils::get_user_by_id(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("DB error: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("User not found"))?;

    register::demote_user(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("DB error demoting user: {}", e))?;

    info!(
        "Admin {} demoted user {} ({})",
        admin_id, user.username, user_id
    );

    deliver_success_json(
        Some(serde_json::json!({
            "user_id":  user_id,
            "username": user.username,
        })),
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
