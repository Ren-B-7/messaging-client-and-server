use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming as IncomingBody;
use hyper::service::Service;
use hyper::{Method, Request, Response, StatusCode};
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::utils::error_response::deliver_error_json;

/// Admin service implementation
#[derive(Clone, Debug)]
pub struct AdminService {
    state: AppState,
    addr: SocketAddr,
}

impl AdminService {
    pub fn new(state: AppState, addr: SocketAddr) -> Self {
        Self { state, addr }
    }
}

impl Service<Request<IncomingBody>> for AdminService {
    type Response = Response<Full<Bytes>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<IncomingBody>) -> Self::Future {
        let state = self.state.clone();
        let addr = self.addr;

        Box::pin(async move {
            match admin_conn(req, addr, state).await {
                Ok(response) => Ok(response),
                Err(e) => {
                    error!("Admin handler error: {:?}", e);
                    // Fallback error response
                    match deliver_error_json(
                        "INTERNAL_ERROR",
                        "Internal Server Error",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    ) {
                        Ok(err_response) => Ok(err_response),
                        Err(delivery_err) => {
                            error!("Failed to deliver error response: {:?}", delivery_err);
                            // Last resort fallback
                            Ok(Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .header("content-type", "application/json")
                                .body(Full::new(Bytes::from(
                                    r#"{"status":"error","code":"INTERNAL_ERROR","message":"Internal Server Error"}"#
                                )))
                                .expect("Failed to build fallback error response"))
                        }
                    }
                }
            }
        })
    }
}

/// Main admin connection handler
async fn admin_conn(
    req: Request<IncomingBody>,
    addr: SocketAddr,
    state: AppState,
) -> Result<Response<Full<Bytes>>> {
    info!(
        "Admin request from {}: {} {}",
        addr,
        req.method(),
        req.uri()
    );

    // Clone path to avoid borrow issues
    let path: String = req.uri().path().to_string();

    // Route admin requests - wrap in error handler
    let result: Result<Response<Full<Bytes>>> = route_admin_request(req, addr, state, &path).await;

    match result {
        Ok(response) => Ok(response),
        Err(e) => {
            error!("Admin handler error for {}: {:?}", path, e);
            deliver_error_json(
                "INTERNAL_ERROR",
                "Internal Server Error",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
            .context("Failed to deliver INTERNAL_ERROR response")
        }
    }
}

/// Route admin requests
async fn route_admin_request(
    req: Request<IncomingBody>,
    _addr: SocketAddr,
    state: AppState,
    path: &str,
) -> Result<Response<Full<Bytes>>> {
    match (req.method(), path) {
        (&Method::GET, "/") | (&Method::GET, "/dashboard") => {
            info!("Serving admin dashboard");

            let total_users: u32 = 42;
            let active_sessions: u32 = 7;
            let banned_users: u32 = 2;

            let dashboard_json = serde_json::json!({
                "status": "success",
                "data": {
                    "title": "Admin Dashboard",
                    "stats": {
                        "total_users": total_users,
                        "active_sessions": active_sessions,
                        "banned_users": banned_users
                    }
                }
            });

            let json_string: String = dashboard_json.to_string();
            let json_bytes: Bytes = Bytes::from(json_string);

            let response: Response<Full<Bytes>> = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Full::new(json_bytes))
                .context("Failed to build dashboard response")?;

            Ok(response)
        }

        (&Method::GET, "/api/stats") | (&Method::GET, "/stats") => {
            info!("Serving admin stats");

            let max_connections: usize = state.config.server.max_connections;
            let bind: &String = &state.config.server.bind;
            let port_client: Option<u32> = state.config.server.port_client;
            let port_admin: Option<u32> = state.config.server.port_admin;
            let token_expiry: u64 = state.config.auth.token_expiry_minutes;
            let email_required: bool = state.config.auth.email_required;

            let stats_json = serde_json::json!({
                "status": "success",
                "data": {
                    "server": {
                        "max_connections": max_connections,
                        "bind": bind,
                        "port_client": port_client,
                        "port_admin": port_admin
                    },
                    "auth": {
                        "token_expiry_minutes": token_expiry,
                        "email_required": email_required
                    }
                }
            });

            let json_string: String = stats_json.to_string();
            let json_bytes: Bytes = Bytes::from(json_string);

            let response: Response<Full<Bytes>> = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Full::new(json_bytes))
                .context("Failed to build stats response")?;

            Ok(response)
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
                .context("Failed to deliver 404 response")
        }
    }
}

/// Handle get users list
async fn handle_get_users(_state: AppState) -> Result<Response<Full<Bytes>>> {
    // TODO: Fetch actual users from database
    let users_json = serde_json::json!({});

    let json_string: String = users_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build users response")?;

    Ok(response)
}

/// Handle ban user request
async fn handle_ban_user(
    req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    use http_body_util::BodyExt;
    use std::collections::HashMap;

    // Parse request body
    let collected_body = req.collect().await.context("Failed to read request body")?;

    let body: Bytes = collected_body.to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid user_id"))?;

    let reason: String = params
        .get("reason")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "No reason provided".to_string());

    info!("Banning user {} with reason: {}", user_id, reason);

    // TODO: Implement actual ban in database
    // crate::database::ban::ban_user(user_id, reason).await?;

    let message: String = format!("User {} has been banned", user_id);
    let response_json = serde_json::json!({
        "status": "success",
        "message": message,
        "data": {
            "user_id": user_id,
            "banned": true,
            "reason": reason
        }
    });

    let json_string: String = response_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build ban response")?;

    Ok(response)
}

/// Handle unban user request
async fn handle_unban_user(
    req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    use http_body_util::BodyExt;
    use std::collections::HashMap;

    let collected_body = req.collect().await.context("Failed to read request body")?;

    let body: Bytes = collected_body.to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid user_id"))?;

    info!("Unbanning user {}", user_id);

    // TODO: Implement actual unban in database
    // crate::database::ban::unban_user(user_id).await?;

    let message: String = format!("User {} has been unbanned", user_id);
    let response_json = serde_json::json!({
        "status": "success",
        "message": message,
        "data": {
            "user_id": user_id,
            "banned": false
        }
    });

    let json_string: String = response_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build unban response")?;

    Ok(response)
}

/// Handle delete user request
async fn handle_delete_user(
    _req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    // Extract user ID from path (simplified - in production use a proper router)
    let user_id: i64 = 123; // Placeholder

    info!("Deleting user {}", user_id);

    // TODO: Implement actual deletion in database
    // crate::database::users::delete_user(user_id).await?;

    let message: String = format!("User {} has been deleted", user_id);
    let response_json = serde_json::json!({
        "status": "success",
        "message": message
    });

    let json_string: String = response_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build delete response")?;

    Ok(response)
}
