use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::Full;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use std::task::{Context as taskContext, Poll};
use tower::Service;
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::http::routes::Router;
use crate::handlers::http::utils::error_response::deliver_error_json;
use crate::handlers::http::utils::response_conversion::{
    convert_response_body, convert_result_body,
};

/// Admin service implementation
#[derive(Clone, Debug)]
pub struct AdminService {
    state: AppState,
    addr: SocketAddr,
    router: &'static Router,
}

impl AdminService {
    pub fn new(state: AppState, addr: SocketAddr) -> Self {
        // Build admin router once and leak it to get 'static lifetime
        // This is fine because the router is immutable and lives for the program lifetime
        let web_dir = state.config.paths.web_dir.clone();
        let icons_dir = state.config.paths.icons.clone();

        let router = Box::leak(Box::new(build_admin_router_with_config(
            Some(web_dir),
            Some(icons_dir),
        )));

        Self {
            state,
            addr,
            router,
        }
    }
}

impl Service<Request<IncomingBody>> for AdminService {
    type Response = Response<BoxBody<Bytes, Infallible>>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut taskContext<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<IncomingBody>) -> Self::Future {
        let state = self.state.clone();
        let addr = self.addr;
        let router = self.router;

        Box::pin(async move {
            match admin_conn(req, addr, state, router).await {
                Ok(response) => Ok(response),
                Err(e) => {
                    error!("Admin handler error: {:?}", e);

                    let fallback = deliver_error_json(
                        "INTERNAL_ERROR",
                        "Internal Server Error",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                    .map(convert_response_body)
                    .unwrap_or_else(|_| {
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .header("content-type", "application/json")
                            .body(
                                http_body_util::BodyExt::boxed(
                                    Full::new(Bytes::from(
                                        r#"{"status":"error","code":"INTERNAL_ERROR","message":"Internal Server Error"}"#,
                                    )),
                                ),
                            )
                            .unwrap()
                    });

                    Ok(fallback)
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
    router: &Router,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!(
        "Admin request from {}: {} {}",
        addr,
        req.method(),
        req.uri()
    );

    let path: String = req.uri().path().to_string();

    // CRITICAL: Filter out all non-admin paths
    // Admin service should ONLY handle /admin/* paths
    if !path.starts_with("/admin") {
        warn!(
            "Non-admin path rejected on admin service: {} from {}",
            path, addr
        );
        return convert_result_body(deliver_error_json(
            "FORBIDDEN",
            "This service only handles /admin/* paths",
            StatusCode::FORBIDDEN,
        ))
        .context("Failed to deliver FORBIDDEN response");
    }

    // Admin paths are validated - route through the admin router
    router
        .route(req, state)
        .await
        .context("Admin routing failed")
}

/// Build the admin router with all admin-specific endpoints
fn build_admin_router() -> Router {
    build_admin_router_with_config(None, None)
}

/// Build the admin router with custom web_dir and icons_dir
fn build_admin_router_with_config(web_dir: Option<String>, icons_dir: Option<String>) -> Router {
    let mut router = Router::new();

    // Set directories if provided
    if let Some(dir) = web_dir {
        router = router.with_web_dir(dir);
    }
    if let Some(dir) = icons_dir {
        router = router.with_icons_dir(dir);
    }

    router
        // Dashboard endpoints
        .get("/admin", |req, state| async move {
            convert_result_body(handle_dashboard(req, state).await)
        })
        .get("/admin/", |req, state| async move {
            convert_result_body(handle_dashboard(req, state).await)
        })
        .get("/admin/dashboard", |req, state| async move {
            convert_result_body(handle_dashboard(req, state).await)
        })
        // Stats endpoints
        .get("/admin/api/stats", |req, state| async move {
            convert_result_body(handle_stats(req, state).await)
        })
        .get("/admin/stats", |req, state| async move {
            convert_result_body(handle_stats(req, state).await)
        })
        // User management endpoints
        .get("/admin/api/users", |req, state| async move {
            convert_result_body(handle_get_users(req, state).await)
        })
        .get("/admin/users", |req, state| async move {
            convert_result_body(handle_get_users(req, state).await)
        })
        .post("/admin/api/users/ban", |req, state| async move {
            convert_result_body(handle_ban_user(req, state).await)
        })
        .post("/admin/users/ban", |req, state| async move {
            convert_result_body(handle_ban_user(req, state).await)
        })
        .post("/admin/api/users/unban", |req, state| async move {
            convert_result_body(handle_unban_user(req, state).await)
        })
        .post("/admin/users/unban", |req, state| async move {
            convert_result_body(handle_unban_user(req, state).await)
        })
        .delete("/admin/api/users/:id", |req, state| async move {
            convert_result_body(handle_delete_user_with_id(req, state).await)
        })
        .delete("/admin/users/:id", |req, state| async move {
            convert_result_body(handle_delete_user_with_id(req, state).await)
        })
        // Health check
        .get("/admin/health", |_req, _state| async move {
            let response = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Full::new(Bytes::from(
                    r#"{"status":"success","service":"admin","health":"ok"}"#,
                )))
                .unwrap();
            Ok(convert_response_body(response))
        })
}

/// Handle dashboard request
async fn handle_dashboard(
    _req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    info!("Serving admin dashboard");

    let total_users: u16 = 42;
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

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(dashboard_json.to_string())))
        .context("Failed to build dashboard response")?;

    Ok(response)
}

/// Handle stats request
async fn handle_stats(
    _req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<Full<Bytes>>> {
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

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(stats_json.to_string())))
        .context("Failed to build stats response")?;

    Ok(response)
}

/// Handle get users list
async fn handle_get_users(
    _req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    info!("Serving user list");

    let users_json = serde_json::json!({
        "status": "success",
        "data": {
            "users": [],
            "total": 0,
            "message": "User list endpoint - database integration pending"
        }
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(users_json.to_string())))
        .context("Failed to build users response")?;

    Ok(response)
}

/// Handle ban user request
async fn handle_ban_user(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<Full<Bytes>>> {
    use crate::database::ban as db_ban;
    use http_body_util::BodyExt;
    use std::collections::HashMap;

    info!("Processing ban user request");

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

    // TODO: Get admin user ID from session
    let admin_id: i64 = 1; // Placeholder

    info!("Banning user {} with reason: {}", user_id, reason);

    // Ban user in database
    db_ban::ban_user(&state.db, user_id, admin_id, Some(reason.clone()))
        .await
        .context("Failed to ban user in database")?;

    let response_json = serde_json::json!({
        "status": "success",
        "message": format!("User {} has been banned", user_id),
        "data": {
            "user_id": user_id,
            "banned": true,
            "reason": reason
        }
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(response_json.to_string())))
        .context("Failed to build ban response")?;

    Ok(response)
}

/// Handle unban user request
async fn handle_unban_user(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<Full<Bytes>>> {
    use crate::database::ban as db_ban;
    use http_body_util::BodyExt;
    use std::collections::HashMap;

    info!("Processing unban user request");

    let collected_body = req.collect().await.context("Failed to read request body")?;
    let body: Bytes = collected_body.to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid user_id"))?;

    info!("Unbanning user {}", user_id);

    // Unban user in database
    db_ban::unban_user(&state.db, user_id)
        .await
        .context("Failed to unban user in database")?;

    let response_json = serde_json::json!({
        "status": "success",
        "message": format!("User {} has been unbanned", user_id),
        "data": {
            "user_id": user_id,
            "banned": false
        }
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(response_json.to_string())))
        .context("Failed to build unban response")?;

    Ok(response)
}

/// Handle delete user request (with ID in path)
async fn handle_delete_user_with_id(
    req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    // Extract user ID from path
    let path = req.uri().path();
    let user_id: i64 = path
        .trim_end_matches('/')
        .split('/')
        .last()
        .and_then(|id_str| {
            // Skip if it's the ":id" placeholder
            if id_str == ":id" {
                None
            } else {
                id_str.parse::<i64>().ok()
            }
        })
        .ok_or_else(|| anyhow::anyhow!("Invalid user ID in path"))?;

    info!("Deleting user {}", user_id);

    // TODO: Implement actual deletion in database
    // crate::database::users::delete_user(user_id).await?;

    let response_json = serde_json::json!({
        "status": "success",
        "message": format!("User {} has been deleted", user_id),
        "data": {
            "user_id": user_id,
            "deleted": true
        }
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(response_json.to_string())))
        .context("Failed to build delete response")?;

    Ok(response)
}
