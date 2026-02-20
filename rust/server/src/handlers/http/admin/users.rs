use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use tracing::{info, warn};

use crate::AppState;
use crate::handlers::http::utils::deliver_serialized_json;

/// Extract and validate an admin session token from the request.
/// Returns the admin's user_id on success.
pub async fn require_admin(
    req: &Request<IncomingBody>,
    state: &AppState,
) -> std::result::Result<i64, ()> {
    use crate::database::login as db_login;

    let token = req
        .headers()
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
        });

    let token = match token {
        Some(t) => t.to_string(),
        None => return Err(()),
    };

    db_login::validate_admin_session(&state.db, token)
        .await
        .ok()
        .flatten()
        .ok_or(())
}

/// GET /admin/api/users
pub async fn handle_get_users(
    _req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Serving user list");

    let users_json = serde_json::json!({
        "status": "success",
        "data": {
            "users":   [],
            "total":   0,
            "message": "User list endpoint — database integration pending"
        }
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(users_json.to_string())).boxed())
        .context("Failed to build users response")?;

    Ok(response)
}

// ---------------------------------------------------------------------------
// Ban / Unban
// ---------------------------------------------------------------------------

/// POST /admin/api/users/ban
/// Body: user_id=<id>&reason=<reason>  (form or JSON)
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
                    "status": "error",
                    "code": "UNAUTHORIZED",
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

    info!(
        "Admin {} banning user {} — reason: {}",
        admin_id, user_id, reason
    );

    db_ban::ban_user(&state.db, user_id, admin_id, Some(reason.clone()))
        .await
        .context("Failed to ban user in database")?;

    deliver_serialized_json(
        &serde_json::json!({
            "status": "success",
            "message": format!("User {} has been banned", user_id),
            "data": { "user_id": user_id, "banned": true, "reason": reason }
        }),
        StatusCode::OK,
    )
}

/// POST /admin/api/users/unban
/// Body: user_id=<id>  (form or JSON)
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
                    "status": "error",
                    "code": "UNAUTHORIZED",
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

    deliver_serialized_json(
        &serde_json::json!({
            "status": "success",
            "message": format!("User {} has been unbanned", user_id),
            "data": { "user_id": user_id, "banned": false }
        }),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// Delete user
// ---------------------------------------------------------------------------

/// DELETE /admin/api/users/:id
pub async fn handle_delete_user(
    req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let path = req.uri().path();

    let user_id: i64 = path
        .trim_end_matches('/')
        .split('/')
        .last()
        .filter(|s| *s != ":id")
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user ID in path"))?;

    info!("Deleting user {}", user_id);

    // TODO: db integration
    // crate::database::users::delete_user(&state.db, user_id).await?;

    deliver_serialized_json(
        &serde_json::json!({
            "status": "success",
            "message": format!("User {} has been deleted", user_id),
            "data": { "user_id": user_id, "deleted": true }
        }),
        StatusCode::OK,
    )
}

/// POST /admin/api/users/promote
/// Body: user_id=<id>  (form or JSON)
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
                    "status": "error",
                    "code": "UNAUTHORIZED",
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
                "status": "error",
                "code": "INVALID_TARGET",
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

    info!(
        "Admin {} promoted user {} ({})",
        admin_id, user.username, user_id
    );

    deliver_serialized_json(
        &serde_json::json!({
            "status": "success",
            "message": format!("{} is now an admin", user.username),
            "data": { "user_id": user_id, "username": user.username }
        }),
        StatusCode::OK,
    )
}

/// POST /admin/api/users/demote
/// Body: user_id=<id>  (form or JSON)
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
                    "status": "error",
                    "code": "UNAUTHORIZED",
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
                "status": "error",
                "code": "INVALID_TARGET",
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

    info!(
        "Admin {} demoted user {} ({})",
        admin_id, user.username, user_id
    );

    deliver_serialized_json(
        &serde_json::json!({
            "status": "success",
            "message": format!("{} is no longer an admin", user.username),
            "data": { "user_id": user_id, "username": user.username }
        }),
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
