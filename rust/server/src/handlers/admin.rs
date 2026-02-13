use std::net::SocketAddr;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Method, Request, Response, StatusCode, body::Incoming};
use serde::Serialize;
use tracing::{error, info, warn};

use crate::AppState;

/// Standard error response
#[derive(Serialize)]
struct ErrorResponse {
    status: String,
    code: String,
    message: String,
}

impl ErrorResponse {
    fn new(code: &str, message: &str) -> Self {
        Self {
            status: "error".to_string(),
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

/// Main admin connection handler
pub async fn admin_conn(
    req: Request<Incoming>,
    addr: SocketAddr,
    state: AppState,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    info!(
        "Admin request from {}: {} {}",
        addr,
        req.method(),
        req.uri()
    );

    // Clone path to avoid borrow issues
    let path = req.uri().path().to_string();

    // Route admin requests - wrap in error handler
    let result = route_admin_request(req, addr, state, &path).await;

    match result {
        Ok(response) => Ok(response),
        Err(e) => {
            error!("Admin handler error for {}: {:?}", path, e);
            deliver_error_json(
                "INTERNAL_ERROR",
                "Internal Server Error",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

/// Route admin requests
async fn route_admin_request(
    req: Request<Incoming>,
    _addr: SocketAddr,
    state: AppState,
    path: &str,
) -> Result<Response<Full<Bytes>>> {
    match (req.method(), path) {
        (&Method::GET, "/") | (&Method::GET, "/dashboard") => {
            info!("Serving admin dashboard");

            let dashboard_json = serde_json::json!({
                "status": "success",
                "data": {
                    "title": "Admin Dashboard",
                    "stats": {
                        "total_users": 42,
                        "active_sessions": 7,
                        "banned_users": 2
                    }
                }
            });

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Full::new(Bytes::from(dashboard_json.to_string())))
                .context("Failed to build dashboard response")?)
        }

        (&Method::GET, "/api/stats") | (&Method::GET, "/stats") => {
            info!("Serving admin stats");

            let stats_json = serde_json::json!({
                "status": "success",
                "data": {
                    "server": {
                        "max_connections": state.config.server.max_connections,
                        "bind": state.config.server.bind,
                        "port_client": state.config.server.port_client,
                        "port_admin": state.config.server.port_admin
                    },
                    "auth": {
                        "token_expiry_minutes": state.config.auth.token_expiry_minutes,
                        "email_required": state.config.auth.email_required
                    }
                }
            });

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Full::new(Bytes::from(stats_json.to_string())))
                .context("Failed to build stats response")?)
        }

        (&Method::GET, "/api/users") | (&Method::GET, "/users") => {
            info!("Serving user list");
            handle_get_users(state).await
        }

        (&Method::POST, "/api/users/ban") | (&Method::POST, "/users/ban") => {
            info!("Processing ban user request");
            handle_ban_user(req, state).await
        }

        (&Method::POST, "/api/users/unban") | (&Method::POST, "/users/unban") => {
            info!("Processing unban user request");
            handle_unban_user(req, state).await
        }

        (&Method::DELETE, "/api/users/:id") => {
            info!("Processing delete user request");
            handle_delete_user(req, state).await
        }

        _ => {
            warn!("Admin 404: {}", path);
            deliver_error_json("NOT_FOUND", "Endpoint not found", StatusCode::NOT_FOUND)
                .map_err(|e| anyhow::anyhow!("Failed to deliver 404: {:?}", e))
        }
    }
}

/// Handle get users list
async fn handle_get_users(_state: AppState) -> Result<Response<Full<Bytes>>> {
    // TODO: Fetch actual users from database
    let users_json = serde_json::json!({
        "status": "success",
        "data": {
            "users": [
                {
                    "id": 1,
                    "username": "admin",
                    "email": "admin@example.com",
                    "banned": false,
                    "created_at": "2024-01-01T00:00:00Z"
                },
                {
                    "id": 2,
                    "username": "demo",
                    "email": "demo@example.com",
                    "banned": false,
                    "created_at": "2024-01-15T00:00:00Z"
                }
            ],
            "total": 2
        }
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(users_json.to_string())))
        .context("Failed to build users response")?)
}

/// Handle ban user request
async fn handle_ban_user(
    req: Request<Incoming>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    use http_body_util::BodyExt;
    use std::collections::HashMap;

    // Parse request body
    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    let user_id = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid user_id"))?;

    let reason = params
        .get("reason")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "No reason provided".to_string());

    info!("Banning user {} with reason: {}", user_id, reason);

    // TODO: Implement actual ban in database
    // crate::database::ban::ban_user(user_id, reason).await?;

    let response_json = serde_json::json!({
        "status": "success",
        "message": format!("User {} has been banned", user_id),
        "data": {
            "user_id": user_id,
            "banned": true,
            "reason": reason
        }
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(response_json.to_string())))
        .context("Failed to build ban response")?)
}

/// Handle unban user request
async fn handle_unban_user(
    req: Request<Incoming>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    use http_body_util::BodyExt;
    use std::collections::HashMap;

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    let user_id = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid user_id"))?;

    info!("Unbanning user {}", user_id);

    // TODO: Implement actual unban in database
    // crate::database::ban::unban_user(user_id).await?;

    let response_json = serde_json::json!({
        "status": "success",
        "message": format!("User {} has been unbanned", user_id),
        "data": {
            "user_id": user_id,
            "banned": false
        }
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(response_json.to_string())))
        .context("Failed to build unban response")?)
}

/// Handle delete user request
async fn handle_delete_user(
    _req: Request<Incoming>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    // Extract user ID from path (simplified - in production use a proper router)
    let user_id = 123; // Placeholder

    info!("Deleting user {}", user_id);

    // TODO: Implement actual deletion in database
    // crate::database::users::delete_user(user_id).await?;

    let response_json = serde_json::json!({
        "status": "success",
        "message": format!("User {} has been deleted", user_id)
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(response_json.to_string())))
        .context("Failed to build delete response")?)
}

/// Deliver JSON error response
fn deliver_error_json(
    code: &str,
    message: &str,
    status: StatusCode,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let error = ErrorResponse::new(code, message);
    let json = serde_json::to_string(&error).unwrap_or_else(|_| {
        r#"{"status":"error","code":"INTERNAL_ERROR","message":"Failed to serialize error"}"#
            .to_string()
    });

    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(json)))
        .unwrap())
}
