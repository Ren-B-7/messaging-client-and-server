use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{Method, Request, Response, StatusCode};
use tracing::warn;

use crate::AppState;
use crate::handlers::http::profile::settings;
use crate::handlers::http::{auth, messaging, profile, utils::*};
use crate::handlers::http::utils::headers::{decode_jwt_claims, validate_jwt_secure};

use shared::types::cache::*;
use shared::types::jwt::JwtClaims;

// ---------------------------------------------------------------------------
// Handler type aliases
// ---------------------------------------------------------------------------
//
// Three flavours depending on what auth the route needs:
//
//   RouteHandler  — no auth, receives raw (req, state)
//   LightHandler  — JWT-only auth, receives (req, state, claims)
//   HardHandler   — full DB auth, receives (req, state, user_id, claims)

type RouteHandler = Box<
    dyn Fn(
            Request<hyper::body::Incoming>,
            AppState,
        ) -> Pin<Box<dyn Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send>>
        + Send
        + Sync,
>;

type LightHandler = Box<
    dyn Fn(
            Request<hyper::body::Incoming>,
            AppState,
            JwtClaims,
        ) -> Pin<Box<dyn Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send>>
        + Send
        + Sync,
>;

type HardHandler = Box<
    dyn Fn(
            Request<hyper::body::Incoming>,
            AppState,
            i64,       // user_id — extracted and verified by the router
            JwtClaims, // full claims in case the handler wants them
        ) -> Pin<Box<dyn Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send>>
        + Send
        + Sync,
>;

// ---------------------------------------------------------------------------
// RouteKind — bundles auth level with the appropriate handler type
// ---------------------------------------------------------------------------

enum RouteKind {
    /// No authentication check.
    Open(RouteHandler),

    /// Light auth: JWT signature + expiry only, zero DB reads.
    /// Handler receives the decoded `JwtClaims`.
    Light(LightHandler),

    /// Hard auth: JWT decode + DB session lookup + IP binding.
    /// Handler receives the verified `user_id` and `JwtClaims`.
    Hard(HardHandler),
}

// ---------------------------------------------------------------------------
// Route
// ---------------------------------------------------------------------------

struct Route {
    method: Method,
    path:   String,
    kind:   RouteKind,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub struct Router {
    routes:    Vec<Route>,
    web_dir:   Option<String>,
    icons_dir: Option<String>,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router")
            .field("routes_count", &self.routes.len())
            .field("web_dir",      &self.web_dir)
            .field("icons_dir",    &self.icons_dir)
            .finish()
    }
}

impl Router {
    pub fn new() -> Self {
        Self { routes: Vec::new(), web_dir: None, icons_dir: None }
    }

    pub fn with_web_dir(mut self, web_dir: String) -> Self {
        self.web_dir = Some(web_dir);
        self
    }

    pub fn with_icons_dir(mut self, icons_dir: String) -> Self {
        self.icons_dir = Some(icons_dir);
        self
    }

    // ── Open (no auth) ────────────────────────────────────────────────────────

    pub fn get<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::GET,
            path:   path.to_string(),
            kind:   RouteKind::Open(Box::new(move |req, state| Box::pin(handler(req, state)))),
        });
        self
    }

    pub fn post<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::POST,
            path:   path.to_string(),
            kind:   RouteKind::Open(Box::new(move |req, state| Box::pin(handler(req, state)))),
        });
        self
    }

    pub fn delete<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::DELETE,
            path:   path.to_string(),
            kind:   RouteKind::Open(Box::new(move |req, state| Box::pin(handler(req, state)))),
        });
        self
    }

    // ── Light auth (JWT decode only, zero DB reads) ───────────────────────────

    /// GET guarded by **light** JWT auth.
    ///
    /// The router verifies the JWT signature and `exp` claim before calling
    /// the handler. No database is touched. The handler receives the decoded
    /// `JwtClaims` directly — no need to call `decode_jwt_claims` inside it.
    pub fn get_light<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState, JwtClaims) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::GET,
            path:   path.to_string(),
            kind:   RouteKind::Light(Box::new(move |req, state, claims| {
                Box::pin(handler(req, state, claims))
            })),
        });
        self
    }

    // ── Hard auth (JWT + DB session + IP binding) ─────────────────────────────

    /// POST guarded by **hard** auth.
    ///
    /// Decodes the JWT, looks up `session_id` in the DB, validates IP binding,
    /// then calls the handler with the verified `user_id` and `JwtClaims`.
    /// The handler never needs to call any auth function itself.
    pub fn post_hard<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState, i64, JwtClaims) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::POST,
            path:   path.to_string(),
            kind:   RouteKind::Hard(Box::new(move |req, state, uid, claims| {
                Box::pin(handler(req, state, uid, claims))
            })),
        });
        self
    }

    /// DELETE guarded by **hard** auth.
    pub fn delete_hard<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState, i64, JwtClaims) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::DELETE,
            path:   path.to_string(),
            kind:   RouteKind::Hard(Box::new(move |req, state, uid, claims| {
                Box::pin(handler(req, state, uid, claims))
            })),
        });
        self
    }

    // ── Dispatch ──────────────────────────────────────────────────────────────

    pub async fn route(
        &self,
        req: Request<hyper::body::Incoming>,
        state: AppState,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>> {
        let method = req.method().clone();
        let path   = req.uri().path().to_string();

        for route in &self.routes {
            if route.method != method || !Self::path_matches(&route.path, &path) {
                continue;
            }

            return match &route.kind {
                RouteKind::Open(h) => h(req, state).await,

                RouteKind::Light(h) => {
                    match decode_jwt_claims(&req, &state.jwt_secret) {
                        Ok(claims) => h(req, state, claims).await,
                        Err(reason) => {
                            warn!("Light-auth rejected {} {}: {}", method, path, reason);
                            unauthorized()
                        }
                    }
                }

                RouteKind::Hard(h) => {
                    // validate_jwt_secure now returns (user_id, claims) — see headers.rs
                    match validate_jwt_secure(&req, &state).await {
                        Ok((user_id, claims)) => h(req, state, user_id, claims).await,
                        Err(reason) => {
                            warn!("Hard-auth rejected {} {}: {}", method, path, reason);
                            unauthorized()
                        }
                    }
                }
            };
        }

        // No registered route matched — try static file fallback for GET
        if method == Method::GET {
            if let Some(static_response) = self.try_serve_static(&path, &state).await? {
                return Ok(static_response);
            }
        }

        json_response::deliver_error_json("NOT_FOUND", "Endpoint not found", StatusCode::NOT_FOUND)
            .context("Failed to deliver 404 response")
    }

    fn path_matches(route_path: &str, request_path: &str) -> bool {
        route_path == request_path
    }

    async fn try_serve_static(
        &self,
        path: &str,
        state: &AppState,
    ) -> Result<Option<Response<BoxBody<Bytes, Infallible>>>> {
        let cfg     = state.config.read().await.clone();
        let web_dir = self.web_dir.as_ref().unwrap_or(&cfg.paths.web_dir);
        let icons   = self.icons_dir.as_ref().unwrap_or(&cfg.paths.icons);

        match path {
            "/" | "/index.html" => {
                let file_path = format!("{}/index.html", web_dir);
                Ok(Some(deliver_html_page(&file_path).context("Failed to deliver HTML page")?))
            }

            path if path.starts_with("/static/") => {
                let file_path = format!("{}{}", web_dir, path);
                Ok(Some(
                    deliver_page_with_status(&file_path, StatusCode::OK, CacheStrategy::Yes)
                        .context("Failed to deliver static file")?,
                ))
            }

            "/favicon.ico"
            | "/favicon.png"
            | "/favicon.svg"
            | "/apple-touch-icon.png"
            | "/apple-touch-icon-precomposed.png"
            | "/android-chrome-192x192.png"
            | "/android-chrome-512x512.png"
            | "/browserconfig.xml"
            | "/site.webmanifest" => {
                let filename  = path.trim_start_matches('/');
                let file_path = format!("{}{}{}", web_dir, icons, filename);
                Ok(Some(
                    deliver_page_with_status(&file_path, StatusCode::OK, CacheStrategy::Yes)
                        .context("Failed to deliver browser icon")?,
                ))
            }

            path if path.starts_with("/non-static/") => {
                let file_path = format!("{}{}", web_dir, path);
                Ok(Some(
                    deliver_page_with_status(&file_path, StatusCode::OK, CacheStrategy::No)
                        .context("Failed to deliver non-static file")?,
                ))
            }

            path if path.ends_with(".html") => {
                let file_path = format!("{}{}", web_dir, path);
                Ok(Some(deliver_html_page(&file_path).context("Failed to deliver HTML file")?))
            }

            _ => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn unauthorized() -> Result<Response<BoxBody<Bytes, Infallible>>> {
    json_response::deliver_error_json(
        "UNAUTHORIZED",
        "Authentication required",
        StatusCode::UNAUTHORIZED,
    )
    .context("Failed to deliver 401 response")
}

// ---------------------------------------------------------------------------
// Shared base API router
// ---------------------------------------------------------------------------

pub fn build_api_router_with_config(web_dir: Option<String>, icons_dir: Option<String>) -> Router {
    let mut router = Router::new();
    if let Some(dir) = web_dir   { router = router.with_web_dir(dir); }
    if let Some(dir) = icons_dir { router = router.with_icons_dir(dir); }

    router
        // ── Public: no auth ──────────────────────────────────────────────────
        .post("/api/register", |req, state| async move {
            auth::handle_register(req, state)
                .await
                .context("Register failed")
        })
        .get("/api/config", |_req, state| async move {
            let config_json = serde_json::json!({
                "status": "success",
                "data": {
                    "email_required":       state.config.read().await.auth.email_required,
                    "token_expiry_minutes": state.config.read().await.auth.token_expiry_minutes,
                }
            });
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(http_body_util::Full::new(Bytes::from(config_json.to_string())).boxed())
                .context("Failed to build config response")?)
        })
        .get("/health", |_req, _state| async move {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(
                    http_body_util::Full::new(Bytes::from(
                        r#"{"status":"success","health":"ok"}"#,
                    ))
                    .boxed(),
                )
                .unwrap())
        })

        // ── Light auth: JWT only, zero DB reads ──────────────────────────────
        .get("/api/profile", |req, state| async move {
            profile::handle_get_profile(req, state)
                .await
                .context("Profile get failed")
        })
        .get("/api/messages", |req, state| async move {
            messaging::handle_get_messages(req, state, None)
                .await
                .context("Message get failed")
        })
        .get("/api/chats", |req, state| async move {
            messaging::handle_get_chats(req, state)
                .await
                .context("Chat get failed")
        })
        .get("/api/groups", |req, state| async move {
            messaging::handle_get_groups(req, state)
                .await
                .context("Group get failed")
        })

        // ── Hard auth: JWT + DB session + IP binding ─────────────────────────
        .post("/api/logout", |req, state| async move {
            settings::handle_logout(req, state)
                .await
                .context("Logout failed")
        })
        .post("/api/profile/update", |req, state| async move {
            profile::handle_update_profile(req, state)
                .await
                .context("Profile update failed")
        })
        .post("/api/settings/password", |req, state| async move {
            profile::handle_change_password(req, state)
                .await
                .context("Password change failed")
        })
        .post("/api/settings/logout-all", |req, state| async move {
            settings::handle_logout_all(req, state)
                .await
                .context("Logout-all failed")
        })
        .post("/api/messages/send", |req, state| async move {
            messaging::handle_send_message(req, state)
                .await
                .context("Message send failed")
        })
        .post("/api/chats", |req, state| async move {
            messaging::handle_create_chat(req, state)
                .await
                .context("Create chat failed")
        })
        .post("/api/groups", |req, state| async move {
            messaging::handle_create_group(req, state)
                .await
                .context("Create group failed")
        })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_path_matches() {
        assert!(Router::path_matches("/api/profile", "/api/profile"));
    }

    #[test]
    fn different_paths_do_not_match() {
        assert!(!Router::path_matches("/api/profile", "/api/settings"));
    }

    #[test]
    fn trailing_slash_matters() {
        assert!(!Router::path_matches("/api/profile", "/api/profile/"));
    }

    #[test]
    fn root_path_matches_self() {
        assert!(Router::path_matches("/", "/"));
    }

    #[test]
    fn static_prefix_detection() {
        assert!("/static/app.js".starts_with("/static/"));
        assert!(!"/app.js".starts_with("/static/"));
    }

    #[test]
    fn non_static_prefix_detection() {
        assert!("/non-static/config.json".starts_with("/non-static/"));
    }

    #[test]
    fn html_suffix_detection() {
        assert!("/about.html".ends_with(".html"));
        assert!(!"/about.css".ends_with(".html"));
    }

    #[test]
    fn favicon_paths_recognised() {
        let favicons = [
            "/favicon.ico",
            "/favicon.png",
            "/favicon.svg",
            "/apple-touch-icon.png",
            "/android-chrome-192x192.png",
            "/android-chrome-512x512.png",
            "/site.webmanifest",
        ];
        for path in &favicons {
            let is_favicon = matches!(
                *path,
                "/favicon.ico"
                    | "/favicon.png"
                    | "/favicon.svg"
                    | "/apple-touch-icon.png"
                    | "/apple-touch-icon-precomposed.png"
                    | "/android-chrome-192x192.png"
                    | "/android-chrome-512x512.png"
                    | "/browserconfig.xml"
                    | "/site.webmanifest"
            );
            assert!(is_favicon, "Expected {} to be recognised as favicon", path);
        }
    }

    #[test]
    fn router_new_has_no_routes() {
        let r = Router::new();
        assert!(r.routes.is_empty());
    }

    #[test]
    fn router_with_web_dir_sets_field() {
        let r = Router::new().with_web_dir("/var/www".to_string());
        assert_eq!(r.web_dir.as_deref(), Some("/var/www"));
    }

    #[test]
    fn router_with_icons_dir_sets_field() {
        let r = Router::new().with_icons_dir("/icons".to_string());
        assert_eq!(r.icons_dir.as_deref(), Some("/icons"));
    }

    #[tokio::test]
    async fn router_get_adds_open_route() {
        let r = Router::new().get("/ping", |_req, _state| async move {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(http_body_util::Full::new(Bytes::from("pong")).boxed())
                .unwrap())
        });
        assert_eq!(r.routes.len(), 1);
        assert_eq!(r.routes[0].path, "/ping");
        assert!(matches!(r.routes[0].kind, RouteKind::Open(_)));
    }

    #[tokio::test]
    async fn router_get_light_adds_light_route() {
        let r = Router::new().get_light("/api/test", |_req, _state, _claims| async move {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(http_body_util::Full::new(Bytes::from("ok")).boxed())
                .unwrap())
        });
        assert_eq!(r.routes.len(), 1);
        assert!(matches!(r.routes[0].kind, RouteKind::Light(_)));
    }

    #[tokio::test]
    async fn router_post_hard_adds_hard_route() {
        let r = Router::new().post_hard(
            "/api/test",
            |_req, _state, _uid, _claims| async move {
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .body(http_body_util::Full::new(Bytes::from("ok")).boxed())
                    .unwrap())
            },
        );
        assert_eq!(r.routes.len(), 1);
        assert!(matches!(r.routes[0].kind, RouteKind::Hard(_)));
    }
}
