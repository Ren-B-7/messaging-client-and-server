use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use std::task::{Context as taskContext, Poll};
use tower::Service;
use tracing::{error, info};

use crate::AppState;
use crate::handlers::http::routes::{Router, build_api_router_with_config};
use crate::handlers::http::{
    admin, auth,
    utils::{deliver_page::*, json_response::*},
};

/// Admin service implementation
#[derive(Clone, Debug)]
pub struct AdminService {
    state: AppState,
    addr: SocketAddr,
    router: Arc<Router>,
}

impl AdminService {
    pub fn new(state: AppState, addr: SocketAddr, router: Arc<Router>) -> Self {
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
        // Arc::clone gives us a cheap reference-counted handle — no move out of self.
        let router = Arc::clone(&self.router);

        Box::pin(async move {
            match admin_conn(req, addr, state, &router).await {
                Ok(response) => Ok(response),
                Err(e) => {
                    error!("Admin handler error: {:?}", e);

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

    router
        .route(req, state)
        .await
        .context("Admin routing failed")
}

/// Build the admin-facing router.
///
/// Starts from the shared API base (`build_api_router_with_config`), layers on
/// admin-only API endpoints (`build_admin_api_routes`), and finally registers
/// the admin HTML pages and login routes.
///
/// Called exactly once at startup; the result is wrapped in `Arc` in `main`.
pub fn build_admin_router_with_config(
    web_dir_static: Option<String>,
    icons_dir_static: Option<String>,
) -> Router {
    // Leak paths for use in async closures that require 'static lifetime.
    // Safe here because this function is called exactly once at startup.
    let web_dir: &'static str = web_dir_static
        .clone()
        .map(|d| -> &'static str { Box::leak(d.into_boxed_str()) })
        .unwrap_or("");

    // Start with shared API routes, layer on admin-specific API routes, then
    // add admin HTML pages and login endpoints.
    let base = build_api_router_with_config(web_dir_static, icons_dir_static);
    let mut router = build_admin_api_routes(base);

    router = router
        // ── HTML pages ──────────────────────────────────────────────────────
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
        // ── Health ──────────────────────────────────────────────────────────
        .get("/admin/health", |_req, _state| async move {
            deliver_serialized_json(
                &serde_json::json!({"status":"success","service":"admin","health":"ok"}),
                StatusCode::OK,
            )
        })
        // ── Login ───────────────────────────────────────────────────────────
        .post("/admin/api/login", |req, state| async move {
            auth::handle_admin_login_api(req, state)
                .await
                .context("Login attempt failed")
        })
        .post("/api/login", |req, state| async move {
            auth::handle_admin_login(req, state)
                .await
                .context("Login attempt failed")
        })
        .post("/login", |req, state| async move {
            auth::handle_admin_login(req, state)
                .await
                .context("Login attempt failed")
        });

    router
}

// ---------------------------------------------------------------------------
// Admin-only API routes
//
// `build_admin_api_routes` is an **append-only** function: it receives an
// already-constructed `Router` (typically the result of
// `build_api_router_with_config`) and chains admin-only endpoints onto it.
// It must never be called on the user service router.
//
// Every route is hard-auth-gated at the router level, and the handler
// additionally checks `claims.is_admin` — two independent privilege checks.
// ---------------------------------------------------------------------------
pub fn build_admin_api_routes(router: Router) -> Router {
    router
        // ── Stats ────────────────────────────────────────────────────────────
        .get_hard("/admin/stats", |req, state, user_id, claims| async move {
            if (!claims.is_admin) || (claims.user_id != user_id) {
                return deliver_error_json(
                    "FORBIDDEN",
                    "Insufficient privileges",
                    StatusCode::FORBIDDEN,
                );
            }
            admin::handle_server_config(req, state, user_id).await
        })
        .get_hard(
            "/admin/api/stats",
            |req, state, user_id, claims| async move {
                if (!claims.is_admin) || (claims.user_id != user_id) {
                    return deliver_error_json(
                        "FORBIDDEN",
                        "Insufficient privileges",
                        StatusCode::FORBIDDEN,
                    );
                }
                admin::handle_server_config(req, state, user_id).await
            },
        )
        // ── Metrics ──────────────────────────────────────────────────────────
        .get_hard("/admin/metrics", |req, state, user_id, claims| async move {
            if (!claims.is_admin) || (claims.user_id != user_id) {
                return deliver_error_json(
                    "FORBIDDEN",
                    "Insufficient privileges",
                    StatusCode::FORBIDDEN,
                );
            }
            admin::handle_metrics(req, state, user_id).await
        })
        .get_hard(
            "/admin/api/metrics",
            |req, state, user_id, claims| async move {
                if (!claims.is_admin) || (claims.user_id != user_id) {
                    return deliver_error_json(
                        "FORBIDDEN",
                        "Insufficient privileges",
                        StatusCode::FORBIDDEN,
                    );
                }
                admin::handle_metrics(req, state, user_id).await
            },
        )
        .get_hard(
            "/admin/api/config",
            |req, state, user_id, claims| async move {
                if (!claims.is_admin) || (claims.user_id != user_id) {
                    return deliver_error_json(
                        "FORBIDDEN",
                        "Insufficient privileges",
                        StatusCode::FORBIDDEN,
                    );
                }
                admin::handle_patch_config(req, state, user_id)
                    .await
                    .context("Update server config failed")
            },
        )
        .post_hard(
            "/admin/api/config",
            |req, state, user_id, claims| async move {
                if (!claims.is_admin) || (claims.user_id != user_id) {
                    return deliver_error_json(
                        "FORBIDDEN",
                        "Insufficient privileges",
                        StatusCode::FORBIDDEN,
                    );
                }
                admin::handle_get_config(req, state, user_id)
                    .await
                    .context("Get server config failed")
            },
        )
        // ── User list ─────────────────────────────────────────────────────────
        .get_light("/admin/users", |req, state, claims| async move {
            if !claims.is_admin {
                return deliver_error_json(
                    "FORBIDDEN",
                    "Insufficient privileges",
                    StatusCode::FORBIDDEN,
                );
            }
            admin::handle_get_users(req, state, 0).await
        })
        .get_light("/admin/api/users", |req, state, claims| async move {
            if !claims.is_admin {
                return deliver_error_json(
                    "FORBIDDEN",
                    "Insufficient privileges",
                    StatusCode::FORBIDDEN,
                );
            }
            admin::handle_get_users(req, state, 0).await
        })
        // ── Session list ──────────────────────────────────────────────────────
        .get_light("/admin/sessions", |req, state, claims| async move {
            if !claims.is_admin {
                return deliver_error_json(
                    "FORBIDDEN",
                    "Insufficient privileges",
                    StatusCode::FORBIDDEN,
                );
            }
            admin::handle_get_sessions(req, state, 0).await
        })
        .get_light("/admin/api/sessions", |req, state, claims| async move {
            if !claims.is_admin {
                return deliver_error_json(
                    "FORBIDDEN",
                    "Insufficient privileges",
                    StatusCode::FORBIDDEN,
                );
            }
            admin::handle_get_sessions(req, state, 0).await
        })
        // ── Ban / unban ───────────────────────────────────────────────────────
        .post_hard("/admin/ban", |req, state, user_id, claims| async move {
            if !claims.is_admin {
                return deliver_error_json(
                    "FORBIDDEN",
                    "Insufficient privileges",
                    StatusCode::FORBIDDEN,
                );
            }
            admin::handle_ban_user(req, state, user_id).await
        })
        .post_hard(
            "/admin/api/users/ban",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return deliver_error_json(
                        "FORBIDDEN",
                        "Insufficient privileges",
                        StatusCode::FORBIDDEN,
                    );
                }
                admin::handle_ban_user(req, state, user_id).await
            },
        )
        .post_hard("/admin/unban", |req, state, user_id, claims| async move {
            if !claims.is_admin {
                return deliver_error_json(
                    "FORBIDDEN",
                    "Insufficient privileges",
                    StatusCode::FORBIDDEN,
                );
            }
            admin::handle_unban_user(req, state, user_id).await
        })
        .post_hard(
            "/admin/api/users/unban",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return deliver_error_json(
                        "FORBIDDEN",
                        "Insufficient privileges",
                        StatusCode::FORBIDDEN,
                    );
                }
                admin::handle_unban_user(req, state, user_id).await
            },
        )
        // ── Delete user ───────────────────────────────────────────────────────
        .delete_hard(
            "/admin/users/:id",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return deliver_error_json(
                        "FORBIDDEN",
                        "Insufficient privileges",
                        StatusCode::FORBIDDEN,
                    );
                }
                admin::handle_delete_user(req, state, user_id).await
            },
        )
        .delete_hard(
            "/admin/api/users/:id",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return deliver_error_json(
                        "FORBIDDEN",
                        "Insufficient privileges",
                        StatusCode::FORBIDDEN,
                    );
                }
                admin::handle_delete_user(req, state, user_id).await
            },
        )
        // ── Promote / demote ──────────────────────────────────────────────────
        .post_hard(
            "/admin/api/users/promote",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return deliver_error_json(
                        "FORBIDDEN",
                        "Insufficient privileges",
                        StatusCode::FORBIDDEN,
                    );
                }
                admin::handle_promote_user(req, state, user_id)
                    .await
                    .context("Promote failed")
            },
        )
        .post_hard(
            "/admin/api/users/demote",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return deliver_error_json(
                        "FORBIDDEN",
                        "Insufficient privileges",
                        StatusCode::FORBIDDEN,
                    );
                }
                admin::handle_demote_user(req, state, user_id)
                    .await
                    .context("Demote failed")
            },
        )
        // ── Config reload (SIGHUP) ────────────────────────────────────────────
        .post_hard(
            "/admin/api/reload",
            |_req, _state, user_id, claims| async move {
                if (!claims.is_admin) || (claims.user_id != user_id) {
                    return deliver_error_json(
                        "FORBIDDEN",
                        "Insufficient privileges",
                        StatusCode::FORBIDDEN,
                    );
                }
                admin::handle_reload_config(_req, user_id).await
            },
        )
}
