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
use crate::handlers::http::routes::{Router, build_api_router_with_config};
use crate::handlers::http::{admin::*, auth::*, utils::*};

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
            deliver_html_page(path).context("failed to deliver admin login page")
        })
        .get("/index.html", move |_req, _state| async move {
            let path = format!("{}/index.html", web_dir);
            deliver_html_page(path).context("failed to deliver admin login page")
        })
        .get("/index", move |_req, _state| async move {
            let path = format!("{}/index.html", web_dir);
            deliver_html_page(path).context("failed to deliver admin login page")
        })
        .get("/login", move |_req, _state| async move {
            let path = format!("{}/index.html", web_dir);
            deliver_html_page(path).context("failed to deliver admin login page")
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
        // ── Promote / demote ─────────────────────────────────────────────────
        .post("/admin/api/users/promote", |req, state| async move {
            handle_promote_user(req, state)
                .await
                .context("Promote failed")
        })
        .post("/admin/api/users/demote", |req, state| async move {
            handle_demote_user(req, state)
                .await
                .context("Demote failed")
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
