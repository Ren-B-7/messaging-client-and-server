use std::collections::HashSet;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use std::task::{Context as taskContext, Poll};
use tower::Service;
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::http::routes::{Router, build_api_router_with_config};
use crate::handlers::http::{admin::*, auth::*, utils::*};

/// User service implementation
#[derive(Clone, Debug)]
pub struct UserService {
    state: AppState,
    addr: SocketAddr,
    router: &'static Router,
}

impl UserService {
    pub async fn new(state: AppState, addr: SocketAddr) -> Self {
        let cfg = state.config.read().await.clone();
        let router = build_user_router_with_config(Some(cfg.paths.web_dir), Some(cfg.paths.icons));

        let router_ref: &'static Router = Box::leak(Box::new(router));

        Self {
            state,
            addr,
            router: router_ref,
        }
    }
}

impl Service<Request<IncomingBody>> for UserService {
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
            match user_conn(req, addr, state, router).await {
                Ok(response) => Ok(response),
                Err(e) => {
                    error!("User handler error: {:?}", e);

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

/// Main user connection handler
async fn user_conn(
    req: Request<IncomingBody>,
    addr: SocketAddr,
    state: AppState,
    router: &Router,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("User request from {}: {} {}", addr, req.method(), req.uri());

    let blocked_paths: &HashSet<String> = &state.config.read().await.paths.blocked_paths.clone();
    let path: String = req.uri().path().to_string();

    // CRITICAL: Block any /admin/* paths on the user service
    if path.starts_with("/admin") {
        warn!(
            "Admin path access attempt from user service {}: {}",
            addr, path
        );
        return deliver_error_json("FORBIDDEN", "Access Denied", StatusCode::FORBIDDEN)
            .context("Failed to deliver FORBIDDEN error response");
    }

    // Check if path is in the blocked paths list
    if blocked_paths.contains(&path) {
        warn!("Blocked path access attempt from {}: {}", addr, path);
        return deliver_error_json("FORBIDDEN", "Access Denied", StatusCode::FORBIDDEN)
            .context("Failed to deliver FORBIDDEN error response");
    }

    // Route through the user router
    router
        .route(req, state)
        .await
        .context("User routing failed")
}

pub fn build_user_router_with_config(
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
        // ── Dashboard / HTML shell ──────────────────────────────────────────
        .get("/login", move |_req, _| async move {
            let path = format!("{}/index.html", web_dir);
            deliver_html_page(path).context("failed to deliver login page")
        })
        .get("/", move |_req, _| async move {
            let path = format!("{}/index.html", web_dir);
            deliver_html_page(path).context("failed to deliver home page")
        })
        .get("/index", move |_req, _| async move {
            let path = format!("{}/index.html", web_dir);
            deliver_html_page(path).context("failed to deliver index page")
        })
        .get("/register", move |_req, _| async move {
            let path = format!("{}/register.html", web_dir);
            deliver_html_page(path).context("failed to deliver register page")
        })
        .get("/settings", move |_req, _| async move {
            let path = format!("{}/settings.html", web_dir);
            deliver_html_page(path).context("failed to deliver settings page")
        })
        .get("/chat", move |_req, _| async move {
            let path = format!("{}/chat.html", web_dir);
            deliver_html_page(path).context("failed to deliver chat page")
        });

    router
}
