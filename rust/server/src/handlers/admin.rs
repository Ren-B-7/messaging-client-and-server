use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::Full;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use std::task::{Context as taskContext, Poll};
use tower::Service;
use tracing::{error, info, warn};

use crate::AppState;
use crate::database::ban as db_ban;
use crate::handlers::http::routes::{Router, build_api_router_with_config};
use crate::handlers::http::utils::*;

/// Admin service implementation
#[derive(Clone, Debug)]
pub struct AdminService {
    state: AppState,
    addr: SocketAddr,
    router: &'static Router,
}

impl AdminService {
    pub fn new(state: AppState, addr: SocketAddr) -> Self {
        let router = build_admin_router_with_config(
            Some(state.config.paths.web_dir.clone()),
            Some(state.config.paths.icons.clone()),
        );

        let router_ref: &'static Router = Box::leak(Box::new(router));

        Self {
            state,
            addr,
            router: router_ref,
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
                    .unwrap_or_else(|delivery_err| {
                        error!(
                            "Failed to deliver error response: {:?}",
                            delivery_err
                        );

                        let fallback_body =
                            http_body_util::Full::new(Bytes::from(
                                r#"{"status":"error","code":"INTERNAL_ERROR","message":"Internal Server Error"}"#,
                            ))
                            .boxed();

                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .header("content-type", "application/json")
                            .body(fallback_body)
                            .unwrap()
                    });

                    Ok(fallback)
                }
            }
        })
    }
}

/// Main admin connection handler — mirrors user_conn structure exactly
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

    router
        .route(req, state)
        .await
        .context("Admin routing failed")
}

// ── Route handlers ──────────────────────────────────────────────────────────

/// Serve server and auth configuration stats
async fn handle_stats(
    _req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Serving admin stats");

    let stats_json = serde_json::json!({
        "status": "success",
        "data": {
            "server": {
                "max_connections": state.config.server.max_connections,
                "bind":            state.config.server.bind,
                "port_client":     state.config.server.port_client,
                "port_admin":      state.config.server.port_admin,
            },
            "auth": {
                "token_expiry_minutes": state.config.auth.token_expiry_minutes,
                "email_required":       state.config.auth.email_required,
            }
        }
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(stats_json.to_string())).boxed())
        .context("Failed to build stats response")?;

    Ok(response)
}

/// Return the full user list
async fn handle_get_users(
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

/// Ban a user by `user_id` (form-encoded body: `user_id`, `reason`)
async fn handle_ban_user(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::ban as db_ban;
    use std::collections::HashMap;

    info!("Processing ban user request");

    let body: Bytes = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user_id"))?;

    let reason: String = params
        .get("reason")
        .cloned()
        .unwrap_or_else(|| "No reason provided".to_string());

    // TODO: replace placeholder with session-derived admin ID
    let admin_id: i64 = 1;

    info!("Banning user {} with reason: {}", user_id, reason);

    db_ban::ban_user(&state.db, user_id, admin_id, Some(reason.clone()))
        .await
        .context("Failed to ban user in database")?;

    let response_json = serde_json::json!({
        "status": "success",
        "message": format!("User {} has been banned", user_id),
        "data": {
            "user_id": user_id,
            "banned":  true,
            "reason":  reason,
        }
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(response_json.to_string())).boxed())
        .context("Failed to build ban response")?;

    Ok(response)
}

/// Unban a user by `user_id` (form-encoded body: `user_id`)
async fn handle_unban_user(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing unban user request");

    let body: Bytes = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user_id"))?;

    info!("Unbanning user {}", user_id);

    db_ban::unban_user(&state.db, user_id)
        .await
        .context("Failed to unban user in database")?;

    let response_json = serde_json::json!({
        "status": "success",
        "message": format!("User {} has been unbanned", user_id),
        "data": {
            "user_id": user_id,
            "banned":  false,
        }
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(response_json.to_string())).boxed())
        .context("Failed to build unban response")?;

    Ok(response)
}

async fn handle_delete_user(
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

    let response_json = serde_json::json!({
        "status": "success",
        "message": format!("User {} has been deleted", user_id),
        "data": {
            "user_id": user_id,
            "deleted": true,
        }
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(response_json.to_string())).boxed())
        .context("Failed to build delete response")?;

    Ok(response)
}

pub fn build_admin_router_with_config(
    web_dir_static: Option<String>,
    icons_dir_static: Option<String>,
) -> Router {
    // Leak paths for use in async closures that require 'static lifetime
    let web_dir: &'static str = web_dir_static
        .clone()
        .map(|d| -> &'static str { Box::leak(d.into_boxed_str()) })
        .unwrap_or("");

    let mut router = build_api_router_with_config(web_dir_static, icons_dir_static);

    router = router
        .get("/", move |_req, _state| async move {
            let path = format!("{}/index.html", web_dir);
            deliver_html_page(path).context("failed to deliver admin dashboard page")
        })
        .get("/index.html", move |_req, _state| async move {
            let path = format!("{}/index.html", web_dir);
            deliver_html_page(path).context("failed to deliver admin dashboard page")
        })
        .get("/index", move |_req, _state| async move {
            let path = format!("{}/index.html", web_dir);
            deliver_html_page(path).context("failed to deliver admin dashboard page")
        })
        .get("/login", move |_req, _state| async move {
            let path = format!("{}/index.html", web_dir);
            deliver_html_page(path).context("failed to deliver admin dashboard page")
        })
        .get("/admin", move |_req, _state| async move {
            let path = format!("{}/admin.html", web_dir);
            deliver_html_page(path).context("failed to deliver admin dashboard page")
        })
        .get("/admin/", move |_req, _state| async move {
            let path = format!("{}/admin.html", web_dir);
            deliver_html_page(path).context("failed to deliver admin dashboard page")
        })
        .get("/admin/dashboard", move |_req, _state| async move {
            let path = format!("{}/admin.html", web_dir);
            deliver_html_page(path).context("failed to deliver admin dashboard page")
        })
        // ── Stats ───────────────────────────────────────────────────────────
        .get("/admin/stats", |req, state| async move {
            handle_stats(req, state).await
        })
        .get("/admin/api/stats", |req, state| async move {
            handle_stats(req, state).await
        })
        // ── User list ────────────────────────────────────────────────────────
        .get("/admin/users", |req, state| async move {
            handle_get_users(req, state).await
        })
        .get("/admin/api/users", |req, state| async move {
            handle_get_users(req, state).await
        })
        // ── Ban / unban ──────────────────────────────────────────────────────
        .post("/admin/ban", |req, state| async move {
            handle_ban_user(req, state).await
        })
        .post("/admin/api/users/ban", |req, state| async move {
            handle_ban_user(req, state).await
        })
        .post("/admin/unban", |req, state| async move {
            handle_unban_user(req, state).await
        })
        .post("/admin/api/users/unban", |req, state| async move {
            handle_unban_user(req, state).await
        })
        // ── Delete user ──────────────────────────────────────────────────────
        .delete("/admin/users/:id", |req, state| async move {
            handle_delete_user(req, state).await
        })
        .delete("/admin/api/users/:id", |req, state| async move {
            handle_delete_user(req, state).await
        })
        // ── Health check ─────────────────────────────────────────────────────
        .get("/admin/health", |_req, _state| async move {
            let body = r#"{"status":"success","service":"admin","health":"ok"}"#;
            let response = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Full::new(Bytes::from(body)).boxed())
                .unwrap();
            Ok(response)
        });

    router
}
