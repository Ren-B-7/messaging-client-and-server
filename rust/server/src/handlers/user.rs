use std::collections::HashSet;
use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as taskContext, Poll};

use anyhow::Context;
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use tower::Service;
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::http::routes::{
    PathParams, Router, build_base_router, build_user_api_routes, forbidden, unauthorized,
};
use crate::handlers::http::{auth, utils::*};
use crate::handlers::sse::sse_helper;

/// User service implementation
#[derive(Clone, Debug)]
pub struct UserService {
    state: AppState,
    addr: SocketAddr,
    router: Arc<Router>,
}

impl UserService {
    pub fn new(state: AppState, addr: SocketAddr, router: Arc<Router>) -> Self {
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
    type Future = Pin<Box<dyn Future<Output = anyhow::Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut taskContext<'_>) -> Poll<anyhow::Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<IncomingBody>) -> Self::Future {
        let state = self.state.clone();
        let addr = self.addr;
        // Arc::clone gives us a cheap reference-counted handle — no move out of self.
        let router = Arc::clone(&self.router);

        Box::pin(async move {
            match user_conn(req, addr, state, &router).await {
                Ok(response) => Ok(response),
                Err(e) => {
                    error!("User handler error: {:?}", e);

                    let fallback = deliver_error_json(
                        "INTERNAL_ERROR",
                        "Internal Server Error",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                    .unwrap_or_else(|delivery_err| {
                        error!("Failed to deliver error response: {:?}", delivery_err);

                        let fallback_body = http_body_util::Full::new(Bytes::from(
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
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("User request from {}: {} {}", addr, req.method(), req.uri());

    let blocked_paths: HashSet<String> = state.config.read().await.paths.blocked_paths.clone();
    let path: String = req.uri().path().to_string();

    // CRITICAL: Block any /admin/* paths on the user service
    if path.starts_with("/admin") {
        warn!(
            "Admin path access attempt from user service {}: {}",
            addr, path
        );
        return unauthorized(&req);
    }

    if blocked_paths.contains(&path) {
        warn!("Blocked path access attempt from {}: {}", addr, path);
        return forbidden(&req);
    }

    router
        .route(req, state)
        .await
        .context("User routing failed")
}

/// Build the user-facing router.
///
/// Starts from the shared API base (`build_api_router_with_config`) and layers
/// on user-only pages, auth endpoints, and the SSE stream.  Admin paths are
/// never registered here; `user_conn` additionally hard-blocks any `/admin`
/// prefix as a defence-in-depth measure.
///
/// Called exactly once at startup; the result is wrapped in `Arc` in `main`.
pub fn build_user_router_with_config(
    web_dir_static: Option<String>,
    icons_dir_static: Option<String>,
) -> Router {
    // Start with shared base API routes, layer on user messaging/profile API,
    // then add HTML pages and the SSE stream.
    let base = build_base_router(web_dir_static, icons_dir_static);
    let mut router = build_user_api_routes(base);

    // Use the directory stored in the router (wrapped in Arc) instead of Box::leak.
    let web_dir = router.web_dir().unwrap_or_else(|| Arc::from(""));

    router = router
        // ── HTML pages ──────────────────────────────────────────────────────
        .get("/login", {
            let web_dir = web_dir.clone();
            move |_req, _| {
                let web_dir = web_dir.clone();
                async move {
                    let path = format!("{}/index.html", web_dir);
                    deliver_html_page(path).context("failed to deliver login page")
                }
            }
        })
        .get("/", {
            let web_dir = web_dir.clone();
            move |_req, _| {
                let web_dir = web_dir.clone();
                async move {
                    let path = format!("{}/index.html", web_dir);
                    deliver_html_page(path).context("failed to deliver home page")
                }
            }
        })
        .get("/index", {
            let web_dir = web_dir.clone();
            move |_req, _| {
                let web_dir = web_dir.clone();
                async move {
                    let path = format!("{}/index.html", web_dir);
                    deliver_html_page(path).context("failed to deliver index page")
                }
            }
        })
        .get("/register", {
            let web_dir = web_dir.clone();
            move |_req, _| {
                let web_dir = web_dir.clone();
                async move {
                    let path = format!("{}/register.html", web_dir);
                    deliver_html_page(path).context("failed to deliver register page")
                }
            }
        })
        .get_light("/settings", {
            let web_dir = web_dir.clone();
            move |_req, _, _| {
                let web_dir = web_dir.clone();
                async move {
                    let path = format!("{}/settings.html", web_dir);
                    deliver_html_page(path).context("failed to deliver settings page")
                }
            }
        })
        .get_light("/chat", {
            let web_dir = web_dir.clone();
            move |_req, _, _| {
                let web_dir = web_dir.clone();
                async move {
                    let path = format!("{}/chat.html", web_dir);
                    deliver_html_page(path).context("failed to deliver chat page")
                }
            }
        })
        // ── Auth ────────────────────────────────────────────────────────────
        .post("/api/login", |req, state| async move {
            auth::handle_login_api(req, state)
                .await
                .context("Login attempt failed")
        })
        .post("/login", |req, state| async move {
            auth::handle_login(req, state)
                .await
                .context("Login attempt failed")
        })
        .post("/api/register", |req, state| async move {
            auth::handle_register_api(req, state)
                .await
                .context("Register failed")
        })
        .post("/register", |req, state| async move {
            auth::handle_register(req, state)
                .await
                .context("Register failed")
        })
        // POST /api/files/upload — multipart upload (hard auth)
        .post_hard(
            "/api/files/upload",
            |req, state, user_id, _claims| async move {
                crate::handlers::http::messaging::files::handle_upload_file(req, state, user_id)
                    .await
                    .context("File upload failed")
            },
        )
        // GET /api/files?chat_id=N — list files in a chat (light auth)
        .get_light("/api/files", |req, state, claims| async move {
            crate::handlers::http::messaging::files::handle_get_chat_files(req, state, claims)
                .await
                .context("Get chat files failed")
        })
        // GET /api/files/:id — download a file (light auth)
        .get_light("/api/files/:id", |req, state, claims| async move {
            let file_id = req
                .extensions()
                .get::<PathParams>()
                .and_then(|p| p.get_i64("id"));
            match file_id {
                Some(id) => crate::handlers::http::messaging::files::handle_download_file(
                    req, state, claims, id,
                )
                .await
                .context("File download failed"),
                None => json_response::deliver_error_json(
                    "BAD_REQUEST",
                    "Invalid file id",
                    StatusCode::BAD_REQUEST,
                )
                .context("Bad request"),
            }
        })
        // DELETE /api/files/:id — delete own file (hard auth)
        .delete_hard(
            "/api/files/:id",
            |req, state, user_id, _claims| async move {
                let file_id = req
                    .extensions()
                    .get::<PathParams>()
                    .and_then(|p| p.get_i64("id"));
                match file_id {
                    Some(id) => crate::handlers::http::messaging::files::handle_delete_file(
                        req, state, user_id, id,
                    )
                    .await
                    .context("File delete failed"),
                    None => json_response::deliver_error_json(
                        "BAD_REQUEST",
                        "Invalid file id",
                        StatusCode::BAD_REQUEST,
                    )
                    .context("Bad request"),
                }
            },
        )
        // POST /api/profile/avatar — upload / replace profile picture (hard auth)
        .post_hard(
            "/api/profile/avatar",
            |req, state, user_id, _claims| async move {
                crate::handlers::http::profile::handle_upload_avatar(req, state, user_id)
                    .await
                    .context("Avatar upload failed")
            },
        )
        // GET /api/avatar/:user_id — serve any user's avatar image (light auth)
        .get_light("/api/avatar/:user_id", |req, state, claims| async move {
            let target_user_id = req
                .extensions()
                .get::<PathParams>()
                .and_then(|p| p.get_i64("user_id"));
            match target_user_id {
                Some(uid) => {
                    crate::handlers::http::profile::handle_get_avatar(req, state, claims, uid)
                        .await
                        .context("Avatar fetch failed")
                }
                None => crate::handlers::http::utils::json_response::deliver_error_json(
                    "BAD_REQUEST",
                    "Invalid user id",
                    hyper::StatusCode::BAD_REQUEST,
                )
                .context("Bad request"),
            }
        })
        // ── Real-time SSE stream ────────────────────────────────────────────
        //
        // Auth is handled inside handle_sse_subscribe (Bearer header or
        // auth_id cookie). Chat context is passed via query params:
        //   ?chat_id=<id>
        //
        // The get_light wrapper provides an initial JWT gate — the SSE handler
        // then decodes the same JWT a second time to extract the session_id
        // UUID, which is validated against the DB sessions table.
        //
        // On connect the handler:
        //   1. Decodes the JWT → extracts session_id
        //   2. Validates session_id against the DB
        //   3. Loads and replays chat history as history_message events
        //   4. Parks on the broadcast channel for live SSE events
        .get_light("/api/stream", |req, state, _claims| async move {
            sse_helper::handle_sse_subscribe(req, state)
                .await
                .map_err(|e| anyhow::anyhow!("SSE subscription failed: {:?}", e))
        });

    router
}
