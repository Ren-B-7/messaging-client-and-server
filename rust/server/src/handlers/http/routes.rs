use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Context, Result};
use base64::prelude::{BASE64_STANDARD, Engine as _};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{Method, Request, Response, StatusCode};
use tracing::{info, warn};

use crate::AppState;
use crate::handlers::http::utils::headers::{decode_jwt_claims, validate_jwt_secure};
use crate::handlers::http::{messaging, profile, utils::*};

use shared::types::cache::*;
use shared::types::jwt::JwtClaims;

// ---------------------------------------------------------------------------
// Path Parameters
// ---------------------------------------------------------------------------

/// Extracted path parameters from a route match (e.g. ":id").
#[derive(Debug, Clone, Default)]
pub struct PathParams(HashMap<String, String>);

impl PathParams {
    pub fn get(&self, name: &str) -> Option<&String> {
        self.0.get(name)
    }

    pub fn get_i64(&self, name: &str) -> Option<i64> {
        self.0.get(name).and_then(|s| s.parse().ok())
    }
}

// ---------------------------------------------------------------------------
// Handler type aliases
// ---------------------------------------------------------------------------
//
// Three security tiers:
//
//   RouteHandler  — no auth.  Receives (req, state).
//                   Use for: /login, /register, /health, static files.
//
//   LightHandler  — JWT signature + expiry, zero DB reads.
//                   Receives (req, state, claims).
//                   Use for: GET / HEAD routes that only read data.
//
//   HardHandler   — JWT decode + DB session lookup + IP binding.
//                   Receives (req, state, user_id, claims).
//                   Use for: POST / PUT / DELETE — anything that mutates state.

type RouteHandler = Box<
    dyn Fn(
            Request<hyper::body::Incoming>,
            AppState,
        )
            -> Pin<Box<dyn Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send>>
        + Send
        + Sync,
>;

type LightHandler = Box<
    dyn Fn(
            Request<hyper::body::Incoming>,
            AppState,
            JwtClaims,
        )
            -> Pin<Box<dyn Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send>>
        + Send
        + Sync,
>;

type HardHandler = Box<
    dyn Fn(
            Request<hyper::body::Incoming>,
            AppState,
            i64,       // user_id  — extracted and verified by the router
            JwtClaims, // full claims in case the handler needs them
        )
            -> Pin<Box<dyn Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send>>
        + Send
        + Sync,
>;

// ---------------------------------------------------------------------------
// RouteKind
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
    path: String,
    kind: RouteKind,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub struct Router {
    routes: Vec<Route>,
    web_dir: Option<Arc<str>>,
    icons_dir: Option<Arc<str>>,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router")
            .field("routes_count", &self.routes.len())
            .field("web_dir", &self.web_dir)
            .field("icons_dir", &self.icons_dir)
            .finish()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            web_dir: None,
            icons_dir: None,
        }
    }

    pub fn with_web_dir(mut self, web_dir: String) -> Self {
        self.web_dir = Some(Arc::from(web_dir));
        self
    }

    pub fn with_icons_dir(mut self, icons_dir: String) -> Self {
        self.icons_dir = Some(Arc::from(icons_dir));
        self
    }

    pub fn web_dir(&self) -> Option<Arc<str>> {
        self.web_dir.clone()
    }

    // ── Open (no auth) ────────────────────────────────────────────────────────

    /// GET with no authentication — use for public pages and health checks.
    pub fn get<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::GET,
            path: path.to_string(),
            kind: RouteKind::Open(Box::new(move |req, state| Box::pin(handler(req, state)))),
        });
        self
    }

    /// POST with no authentication — use only for login / register.
    pub fn post<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::POST,
            path: path.to_string(),
            kind: RouteKind::Open(Box::new(move |req, state| Box::pin(handler(req, state)))),
        });
        self
    }

    /// DELETE with no authentication — rarely needed; prefer `delete_hard`.
    pub fn delete<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::DELETE,
            path: path.to_string(),
            kind: RouteKind::Open(Box::new(move |req, state| Box::pin(handler(req, state)))),
        });
        self
    }

    // ── Light auth (JWT signature + expiry, zero DB reads) ───────────────────

    /// GET guarded by **light** JWT auth.
    pub fn get_light<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState, JwtClaims) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::GET,
            path: path.to_string(),
            kind: RouteKind::Light(Box::new(move |req, state, claims| {
                Box::pin(handler(req, state, claims))
            })),
        });
        self
    }

    /// HEAD guarded by **light** JWT auth.
    pub fn head_light<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState, JwtClaims) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::HEAD,
            path: path.to_string(),
            kind: RouteKind::Light(Box::new(move |req, state, claims| {
                Box::pin(handler(req, state, claims))
            })),
        });
        self
    }

    // ── Hard auth (JWT + DB session lookup + IP binding) ─────────────────────

    /// POST guarded by **hard** auth.
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
            path: path.to_string(),
            kind: RouteKind::Hard(Box::new(move |req, state, uid, claims| {
                Box::pin(handler(req, state, uid, claims))
            })),
        });
        self
    }

    /// GET guarded by **hard** auth.
    pub fn get_hard<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState, i64, JwtClaims) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::GET,
            path: path.to_string(),
            kind: RouteKind::Hard(Box::new(move |req, state, uid, claims| {
                Box::pin(handler(req, state, uid, claims))
            })),
        });
        self
    }

    /// PUT guarded by **hard** auth.
    pub fn put_hard<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState, i64, JwtClaims) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::PUT,
            path: path.to_string(),
            kind: RouteKind::Hard(Box::new(move |req, state, uid, claims| {
                Box::pin(handler(req, state, uid, claims))
            })),
        });
        self
    }

    /// PATCH guarded by **hard** auth.
    pub fn patch_hard<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState, i64, JwtClaims) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::PATCH,
            path: path.to_string(),
            kind: RouteKind::Hard(Box::new(move |req, state, uid, claims| {
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
            path: path.to_string(),
            kind: RouteKind::Hard(Box::new(move |req, state, uid, claims| {
                Box::pin(handler(req, state, uid, claims))
            })),
        });
        self
    }

    // ── Dispatch ──────────────────────────────────────────────────────────────

    pub async fn route(
        &self,
        mut req: Request<hyper::body::Incoming>,
        state: AppState,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>> {
        let method = req.method().clone();
        let path = req.uri().path().to_string();

        for route in &self.routes {
            if route.method != method {
                continue;
            }

            if let Some(params) = Self::extract_params(&route.path, &path) {
                // Store parameters in request extensions so handlers can access them.
                req.extensions_mut().insert(params);

                return match &route.kind {
                    // ── Open ──────────────────────────────────────────────────────
                    RouteKind::Open(h) => h(req, state).await,

                    // ── Light: JWT decode only, no DB ─────────────────────────────
                    RouteKind::Light(h) => match decode_jwt_claims(&req, &state.jwt_secret) {
                        Ok(claims) => h(req, state, claims).await,
                        Err(reason) => {
                            warn!("Light-auth rejected {} {}: {}", method, path, reason);
                            // Token is missing or cryptographically invalid — always 401.
                            unauthorized(&req)
                        }
                    },

                    // ── Hard: JWT + DB session lookup + IP binding ────────────────
                    RouteKind::Hard(h) => match validate_jwt_secure(&req, &state).await {
                        Ok((user_id, claims)) => h(req, state, user_id, claims).await,
                        Err(reason) => {
                            warn!("Hard-auth rejected {} {}: {}", method, path, reason);
                            // Token decoded successfully but the session was revoked or the
                            // request arrived from a different IP — the user is authenticated
                            // but not permitted, so 403 is more accurate than 401.
                            if reason.contains("Session") || reason.contains("IP") {
                                forbidden(&req)
                            } else {
                                unauthorized(&req)
                            }
                        }
                    },
                };
            }
        }

        // No registered route matched — try static file fallback for GET.
        if method == Method::GET
            && let Some(static_response) = self.try_serve_static(&path, &state).await?
        {
            return Ok(static_response);
        }

        not_found(&req)
    }

    // ── Path matching & Param Extraction ──────────────────────────────────────

    pub fn extract_params(route_path: &str, request_path: &str) -> Option<PathParams> {
        // Strip query string from incoming request path before comparing.
        let clean = request_path.split('?').next().unwrap_or(request_path);

        // Exact match (optimisation).
        if route_path == clean {
            return Some(PathParams::default());
        }

        // Segment-by-segment matching for `:param` wildcards.
        // e.g.  "/admin/users/:id"  matches  "/admin/users/42"
        let route_segs: Vec<&str> = route_path.split('/').collect();
        let path_segs: Vec<&str> = clean.split('/').collect();

        if route_segs.len() != path_segs.len() {
            return None;
        }

        let mut params = HashMap::new();

        for (r, p) in route_segs.iter().zip(path_segs.iter()) {
            if let Some(name) = r.strip_prefix(':') {
                params.insert(name.to_string(), p.to_string());
            } else if r != p {
                return None;
            }
        }

        Some(PathParams(params))
    }

    // ── Static file fallback ──────────────────────────────────────────────────

    async fn try_serve_static(
        &self,
        path: &str,
        state: &AppState,
    ) -> Result<Option<Response<BoxBody<Bytes, Infallible>>>> {
        let cfg = state.config.read().await.clone();

        let web_dir = self
            .web_dir
            .as_ref()
            .map(|s| s.as_ref())
            .unwrap_or(cfg.paths.web_dir.as_str())
            .trim_end_matches('/');

        let icons = self
            .icons_dir
            .as_ref()
            .map(|s| s.as_ref())
            .unwrap_or(cfg.paths.icons.as_str())
            .trim_start_matches('/')
            .trim_end_matches('/');

        match path {
            "/" | "/index.html" => {
                let file_path = format!("{}/index.html", web_dir);
                Ok(Some(
                    deliver_html_page(&file_path).context("Failed to deliver HTML page")?,
                ))
            }

            path if path.starts_with("/static/") => {
                let file_path = format!("{}{}", web_dir, path);
                Ok(Some(
                    deliver_page_with_status(&file_path, StatusCode::OK, CacheStrategy::LongTerm)
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
                info!("icons: {}", icons);
                let file_path = format!("{}/{}{}", web_dir, icons, path);
                Ok(Some(
                    deliver_page_with_status(&file_path, StatusCode::OK, CacheStrategy::LongTerm)
                        .context("Failed to deliver browser icon")?,
                ))
            }

            path if path.starts_with("/non-static/") => {
                let file_path = format!("{}{}", web_dir, path);
                Ok(Some(
                    deliver_page_with_status(&file_path, StatusCode::OK, CacheStrategy::ShortTerm)
                        .context("Failed to deliver non-static file")?,
                ))
            }

            // ── Error page ────────────────────────────────────────────────────
            //
            // /error          — generic error page; payload arrives via ?e= or
            //                   ?code=&title=&subtitle=&hint= query params that
            //                   error.html reads entirely client-side.
            //
            // /error/:code    — convenience alias so server-side redirects can
            //                   use a clean URL like /error/404 while still
            //                   serving the same error.html shell. The JS in
            //                   error.html parses the numeric segment itself.
            "/error" => {
                let file_path = format!("{}/error.html", web_dir);
                Ok(Some(
                    deliver_html_page(&file_path).context("Failed to deliver error page")?,
                ))
            }

            path if {
                let segs: Vec<&str> = path.splitn(4, '/').collect();
                segs.len() == 3 && segs[1] == "error" && !segs[2].is_empty()
            } =>
            {
                // e.g. /error/404, /error/403, /error/500
                // Serve error.html regardless of the code segment — the page
                // reads window.location.pathname to extract it client-side.
                let file_path = format!("{}/error.html", web_dir);
                Ok(Some(
                    deliver_html_page(&file_path).context("Failed to deliver error page")?,
                ))
            }

            path if path.ends_with(".html") => {
                let file_path = format!("{}{}", web_dir, path);
                Ok(Some(
                    deliver_html_page(&file_path).context("Failed to deliver HTML file")?,
                ))
            }

            _ => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` when the request was made by a browser navigating directly
/// (i.e. the `Accept` header prefers `text/html` over `application/json`).
/// Programmatic `fetch()` / `XMLHttpRequest` callers send `application/json`
/// or `*/*` without an explicit HTML preference, so they continue to receive
/// JSON error bodies as before.
fn prefers_html(req: &Request<hyper::body::Incoming>) -> bool {
    req.headers()
        .get(hyper::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|accept| {
            // Walk the comma-separated media-range list and return true only
            // when `text/html` appears before `application/json` (or when
            // `application/json` is absent altogether).
            let mut html_q = -1.0f32;
            let mut json_q = -1.0f32;

            for part in accept.split(',') {
                let part = part.trim();
                let (media, params) = match part.split_once(';') {
                    Some((m, p)) => (m.trim(), p.trim()),
                    None => (part, ""),
                };
                let q: f32 = if let Some(stripped) = params.strip_prefix("q=") {
                    stripped.parse().unwrap_or(1.0)
                } else {
                    1.0
                };
                match media {
                    "text/html" => html_q = html_q.max(q),
                    "application/json" => json_q = json_q.max(q),
                    _ => {}
                }
            }

            // A browser that sends `text/html,application/xhtml+xml,...`
            // will have html_q=1.0, json_q=-1.0 → redirect.
            // A fetch() with `application/json` will have json_q=1.0,
            // html_q=-1.0 → JSON body.
            html_q > json_q
        })
        .unwrap_or(false)
}

/// Build a base64-encoded error payload URL for `/error`.
///
/// The payload matches the schema consumed by `error.html`:
/// `{ code, title, subtitle, hint, primary: { label, href } }`
fn error_redirect_url(
    code: u16,
    title: &str,
    subtitle: &str,
    hint: &str,
    primary_label: &str,
    primary_href: &str,
) -> String {
    let json = serde_json::json!({
        "code": code,
        "title": title,
        "subtitle": subtitle,
        "hint": hint,
        "primary": {
            "label": primary_label,
            "href": primary_href
        }
    })
    .to_string();

    // base64 encode — use the standard alphabet without padding issues in URLs
    let encoded = BASE64_STANDARD.encode(json.as_bytes());
    format!("/error?e={}", encoded)
}

pub fn unauthorized(
    req: &Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    if prefers_html(req) {
        let location = error_redirect_url(
            401,
            "Unauthorised",
            "You need to sign in to access this page.",
            "Please log in and try again.",
            "Sign In",
            "/login",
        );
        return deliver_redirect(&location).context("Failed to build 401 redirect response");
    }

    deliver_error_json(
        "UNAUTHORIZED",
        "Authentication required",
        StatusCode::UNAUTHORIZED,
    )
    .context("Failed to deliver 401 response")
}

pub fn forbidden(
    req: &Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    if prefers_html(req) {
        let location = error_redirect_url(
            403,
            "Forbidden",
            "You don't have permission to view this resource.",
            "Contact an administrator if you believe this is a mistake.",
            "Go Home",
            "/",
        );
        return deliver_redirect(&location).context("Failed to build 403 redirect response");
    }

    deliver_error_json(
        "FORBIDDEN",
        "Insufficient privileges",
        StatusCode::FORBIDDEN,
    )
    .context("Failed to deliver 403 response")
}

pub fn not_found(
    req: &Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    // Only redirect browsers navigating to page-like URLs.
    // API endpoints (/api/*), source maps (*.map), and well-known paths
    // always return JSON — never redirect, as those callers do not render HTML.
    let path = req.uri().path();
    let is_api = path.starts_with("/api/");
    let is_source_map = path.ends_with(".map");
    let is_well_known = path.starts_with("/.well-known/");

    if !is_api && !is_source_map && !is_well_known && prefers_html(req) {
        let location = error_redirect_url(
            404,
            "Page Not Found",
            "The page you're looking for doesn't exist.",
            "It may have been moved, renamed, or deleted.",
            "Go Home",
            "/",
        );
        return deliver_redirect(&location).context("Failed to build 404 redirect response");
    }

    deliver_error_json("NOT_FOUND", "Endpoint not found", StatusCode::NOT_FOUND)
        .context("Failed to deliver 404 response")
}

// ---------------------------------------------------------------------------
// Router Compositions
//
// We use a layered approach to build routers:
//
//   1. build_base_router      — public / health / basic config (shared by all)
//   2. build_user_api_routes  — messaging, groups, profile (user-only)
//
// The admin server calls #1 and then adds its own admin-specific routes.
// The user server calls #1 and #2.
// ---------------------------------------------------------------------------

pub fn build_base_router(web_dir: Option<String>, icons_dir: Option<String>) -> Router {
    let mut router = Router::new();
    if let Some(dir) = web_dir {
        router = router.with_web_dir(dir);
    }
    if let Some(dir) = icons_dir {
        router = router.with_icons_dir(dir);
    }

    router
        // ── Public: no auth ──────────────────────────────────────────────────
        .get("/api/config", |_req, state| async move {
            let data = serde_json::json!({
                "email_required":       state.config.read().await.auth.email_required,
                "token_expiry_minutes": state.config.read().await.auth.token_expiry_minutes,
            });
            deliver_success_json(Some(data), None, StatusCode::OK)
        })
        .get("/health", |_req, _state| async move {
            deliver_success_json(
                Some(serde_json::json!({"health":"ok"})),
                None,
                StatusCode::IM_A_TEAPOT,
            )
        })
        // ── Password reset (Public) ──────────────────────────────────────────
        .post("/api/auth/reset-request", |req, state| async move {
            crate::handlers::http::auth::reset::handle_reset_request(req, state)
                .await
                .context("Reset request failed")
        })
        .post("/api/auth/reset-confirm", |req, state| async move {
            crate::handlers::http::auth::reset::handle_reset_confirm(req, state)
                .await
                .context("Reset confirm failed")
        })
}

pub fn build_user_api_routes(router: Router) -> Router {
    router
        // ── Light auth: JWT decode only, zero DB reads ────────────────────────
        .get_light("/api/profile", |req, state, claims| async move {
            profile::handle_get_profile(req, state, claims)
                .await
                .context("Profile get failed")
        })
        .get_light("/api/messages", |req, state, claims| async move {
            messaging::handle_get_messages(req, state, claims)
                .await
                .context("Message get failed")
        })
        .get_light("/api/chats", |req, state, claims| async move {
            messaging::handle_get_chats(req, state, claims)
                .await
                .context("Chat get failed")
        })
        .get_light("/api/groups", |req, state, claims| async move {
            messaging::handle_get_groups(req, state, claims)
                .await
                .context("Group get failed")
        })
        .get_light("/api/groups/:id/members", |req, state, claims| async move {
            let chat_id = req
                .extensions()
                .get::<PathParams>()
                .and_then(|p| p.get_i64("id"));
            match chat_id {
                Some(id) => messaging::handle_get_members(req, state, claims, id)
                    .await
                    .context("Get members failed"),
                None => json_response::deliver_error_json(
                    "BAD_REQUEST",
                    "Invalid group id",
                    StatusCode::BAD_REQUEST,
                )
                .context("Bad request"),
            }
        })
        // ── Hard auth: Messaging ─────────────────────────────────────────────
        .post_hard(
            "/api/messages/send",
            |req, state, user_id, _claims| async move {
                messaging::handle_send_message(req, state, user_id)
                    .await
                    .context("Message send failed")
            },
        )
        .post_hard(
            "/api/messages/:id/read",
            |req, state, user_id, _claims| async move {
                let message_id = req
                    .extensions()
                    .get::<PathParams>()
                    .and_then(|p| p.get_i64("id"));
                match message_id {
                    Some(id) => messaging::handle_mark_read(req, state, user_id, id)
                        .await
                        .context("Mark read failed"),
                    None => json_response::deliver_error_json(
                        "BAD_REQUEST",
                        "Invalid message id",
                        StatusCode::BAD_REQUEST,
                    )
                    .context("Bad request"),
                }
            },
        )
        .post_hard("/api/typing", |req, state, user_id, _claims| async move {
            messaging::handle_typing(req, state, user_id)
                .await
                .context("Typing indicator failed")
        })
        .delete_hard(
            "/api/messages/:id",
            |req, state, user_id, _claims| async move {
                let message_id = req
                    .extensions()
                    .get::<PathParams>()
                    .and_then(|p| p.get_i64("id"));
                match message_id {
                    Some(id) => messaging::handle_delete_message(req, state, user_id, id)
                        .await
                        .context("Message delete failed"),
                    None => json_response::deliver_error_json(
                        "BAD_REQUEST",
                        "Invalid message id",
                        StatusCode::BAD_REQUEST,
                    )
                    .context("Bad request"),
                }
            },
        )
        .get_light("/api/unread", |req, state, claims| async move {
            messaging::handle_get_unread(req, state, claims)
                .await
                .context("Unread count failed")
        })
        // ── Chats / groups ───────────────────────────────────────────────────
        .post_hard("/api/chats", |req, state, user_id, _claims| async move {
            messaging::handle_create_chat(req, state, user_id)
                .await
                .context("Create chat failed")
        })
        .post_hard("/api/groups", |req, state, user_id, _claims| async move {
            messaging::handle_create_group(req, state, user_id)
                .await
                .context("Create group failed")
        })
        .post_hard(
            "/api/groups/:id/members",
            |req, state, user_id, _claims| async move {
                let chat_id = req
                    .extensions()
                    .get::<PathParams>()
                    .and_then(|p| p.get_i64("id"));
                match chat_id {
                    Some(id) => messaging::handle_add_member(req, state, user_id, id)
                        .await
                        .context("Add member failed"),
                    None => json_response::deliver_error_json(
                        "BAD_REQUEST",
                        "Invalid group id",
                        StatusCode::BAD_REQUEST,
                    )
                    .context("Bad request"),
                }
            },
        )
        .delete_hard(
            "/api/groups/:id/members",
            |req, state, user_id, _claims| async move {
                let chat_id = req
                    .extensions()
                    .get::<PathParams>()
                    .and_then(|p| p.get_i64("id"));
                match chat_id {
                    Some(id) => messaging::handle_remove_member(req, state, user_id, id)
                        .await
                        .context("Remove member failed"),
                    None => json_response::deliver_error_json(
                        "BAD_REQUEST",
                        "Invalid group id",
                        StatusCode::BAD_REQUEST,
                    )
                    .context("Bad request"),
                }
            },
        )
        .patch_hard(
            "/api/groups/:id",
            |req, state, user_id, _claims| async move {
                let chat_id = req
                    .extensions()
                    .get::<PathParams>()
                    .and_then(|p| p.get_i64("id"));
                match chat_id {
                    Some(id) => messaging::handle_rename_group(req, state, user_id, id)
                        .await
                        .context("Rename group failed"),
                    None => json_response::deliver_error_json(
                        "BAD_REQUEST",
                        "Invalid group id",
                        StatusCode::BAD_REQUEST,
                    )
                    .context("Bad request"),
                }
            },
        )
        .delete_hard(
            "/api/groups/:id",
            |req, state, user_id, _claims| async move {
                let chat_id = req
                    .extensions()
                    .get::<PathParams>()
                    .and_then(|p| p.get_i64("id"));
                match chat_id {
                    Some(id) => messaging::handle_delete_group(req, state, user_id, id)
                        .await
                        .context("Delete group failed"),
                    None => json_response::deliver_error_json(
                        "BAD_REQUEST",
                        "Invalid group id",
                        StatusCode::BAD_REQUEST,
                    )
                    .context("Bad request"),
                }
            },
        )
        .get_light("/api/users/search", |req, state, claims| async move {
            messaging::handle_search_users(req, state, claims)
                .await
                .context("User search failed")
        })
        // ── Profile / Settings ───────────────────────────────────────────────
        .post_hard(
            "/api/profile/update",
            |req, state, user_id, _claims| async move {
                profile::handle_update_profile(req, state, user_id)
                    .await
                    .context("Profile update failed")
            },
        )
        .put_hard("/api/profile", |req, state, user_id, _claims| async move {
            profile::handle_update_profile(req, state, user_id)
                .await
                .context("Profile update failed")
        })
        .post_hard(
            "/api/settings/password",
            |req, state, user_id, _claims| async move {
                profile::handle_change_password(req, state, user_id)
                    .await
                    .context("Password change failed")
            },
        )
        .post_hard(
            "/api/settings/logout-all",
            |req, state, user_id, claims| async move {
                profile::handle_logout_all(req, state, user_id, claims)
                    .await
                    .context("Logout-all failed")
            },
        )
        .post_hard("/api/logout", |req, state, user_id, claims| async move {
            profile::handle_logout(req, state, user_id, claims)
                .await
                .context("Logout failed")
        })
        .delete_hard(
            "/api/settings/delete",
            |req, state, user_id, claims| async move {
                profile::handle_delete_profile(req, state, user_id, claims)
                    .await
                    .context("Password change failed")
            },
        )
}
