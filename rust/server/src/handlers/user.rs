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
use crate::handlers::http::routes::{Router, build_user_router_with_config};
use crate::handlers::http::utils::error_response::deliver_error_json;
use crate::handlers::http::utils::response_conversion::{
    convert_response_body, convert_result_body,
};

/// User service implementation
#[derive(Clone, Debug)]
pub struct UserService {
    state: AppState,
    addr: SocketAddr,
    router: &'static Router,
}

impl UserService {
    pub fn new(state: AppState, addr: SocketAddr) -> Self {
        // Build user router once and leak it to get 'static lifetime
        // This is fine because the router is immutable and lives for the program lifetime
        let web_dir = state.config.paths.web_dir.clone();
        let icons_dir = state.config.paths.icons.clone();

        let router = Box::leak(Box::new(build_user_router_with_config(
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
                    .map(convert_response_body)
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

    let blocked_paths: &HashSet<String> = &state.config.paths.blocked_paths;
    let path: String = req.uri().path().to_string();

    // CRITICAL: Block any /admin/* paths on the user service
    if path.starts_with("/admin") {
        warn!(
            "Admin path access attempt from user service {}: {}",
            addr, path
        );
        return convert_result_body(deliver_error_json(
            "FORBIDDEN",
            "Access Denied",
            StatusCode::FORBIDDEN,
        ))
        .context("Failed to deliver FORBIDDEN error response");
    }

    // Check if path is in the blocked paths list
    if blocked_paths.contains(&path) {
        warn!("Blocked path access attempt from {}: {}", addr, path);
        return convert_result_body(deliver_error_json(
            "FORBIDDEN",
            "Access Denied",
            StatusCode::FORBIDDEN,
        ))
        .context("Failed to deliver FORBIDDEN error response");
    }

    // Route through the user router
    router
        .route(req, state)
        .await
        .context("User routing failed")
}
