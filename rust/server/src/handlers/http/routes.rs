use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;

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
    web_dir: Option<String>,
    icons_dir: Option<String>,
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
        self.web_dir = Some(web_dir);
        self
    }

    pub fn with_icons_dir(mut self, icons_dir: String) -> Self {
        self.icons_dir = Some(icons_dir);
        self
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
        req: Request<hyper::body::Incoming>,
        state: AppState,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>> {
        let method = req.method().clone();
        let path = req.uri().path().to_string();

        for route in &self.routes {
            if route.method != method || !Self::path_matches(&route.path, &path) {
                continue;
            }

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

        // No registered route matched — try static file fallback for GET.
        if method == Method::GET
            && let Some(static_response) = self.try_serve_static(&path, &state).await?
        {
            return Ok(static_response);
        }

        not_found(&req)
    }

    // ── Path matching ─────────────────────────────────────────────────────────

    pub fn path_matches(route_path: &str, request_path: &str) -> bool {
        // Strip query string from incoming request path before comparing.
        let clean = request_path.split('?').next().unwrap_or(request_path);

        // Exact match.
        if route_path == clean {
            return true;
        }

        // Segment-by-segment matching for `:param` wildcards.
        // e.g.  "/admin/users/:id"  matches  "/admin/users/42"
        let route_segs: Vec<&str> = route_path.split('/').collect();
        let path_segs: Vec<&str> = clean.split('/').collect();

        if route_segs.len() != path_segs.len() {
            return false;
        }

        route_segs
            .iter()
            .zip(path_segs.iter())
            .all(|(r, p)| r.starts_with(':') || r == p)
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
            .unwrap_or(&cfg.paths.web_dir)
            .trim_end_matches('/');
        let icons = self
            .icons_dir
            .as_ref()
            .unwrap_or(&cfg.paths.icons)
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
    // Hand-roll minimal JSON to avoid pulling in serde just for this path.
    let json = format!(
        r#"{{"code":{code},"title":"{title}","subtitle":"{subtitle}","hint":"{hint}","primary":{{"label":"{primary_label}","href":"{primary_href}"}}}}"#,
        code = code,
        title = title,
        subtitle = subtitle,
        hint = hint,
        primary_label = primary_label,
        primary_href = primary_href,
    );
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
        return Response::builder()
            .status(StatusCode::FOUND)
            .header(hyper::header::LOCATION, location)
            .header(hyper::header::CACHE_CONTROL, "no-store")
            .body(http_body_util::Empty::new().map_err(|e| match e {}).boxed())
            .context("Failed to build 401 redirect response");
    }

    json_response::deliver_error_json(
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
            "You don\\'t have permission to view this resource.",
            "Contact an administrator if you believe this is a mistake.",
            "Go Home",
            "/",
        );
        return Response::builder()
            .status(StatusCode::FOUND)
            .header(hyper::header::LOCATION, location)
            .header(hyper::header::CACHE_CONTROL, "no-store")
            .body(http_body_util::Empty::new().map_err(|e| match e {}).boxed())
            .context("Failed to build 403 redirect response");
    }

    json_response::deliver_error_json(
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
            "The page you\'re looking for doesn\'t exist.",
            "It may have been moved, renamed, or deleted.",
            "Go Home",
            "/",
        );
        return Response::builder()
            .status(StatusCode::FOUND)
            .header(hyper::header::LOCATION, location)
            .header(hyper::header::CACHE_CONTROL, "no-store")
            .body(http_body_util::Empty::new().map_err(|e| match e {}).boxed())
            .context("Failed to build 404 redirect response");
    }

    json_response::deliver_error_json("NOT_FOUND", "Endpoint not found", StatusCode::NOT_FOUND)
        .context("Failed to deliver 404 response")
}

// ---------------------------------------------------------------------------
// Shared API routes
//
// Registered on BOTH the user server and the admin server.  The auth tier for
// each route is chosen by the router method:
//
//   .get(...)          → Open     — handler gets (req, state)
//   .get_light(...)    → Light    — handler gets (req, state, claims)
//   .post(...)         → Open     — login / register only
//   .post_hard(...)    → Hard     — handler gets (req, state, user_id, claims)
//   .put_hard(...)     → Hard     — same
//   .patch_hard(...)   → Hard     — same
//   .delete_hard(...)  → Hard     — same
// ---------------------------------------------------------------------------

pub fn build_api_router_with_config(web_dir: Option<String>, icons_dir: Option<String>) -> Router {
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
        // ── Light auth: JWT decode only, zero DB reads ────────────────────────
        //
        // Safe for GET requests because reading stale data carries no real risk
        // and avoids a DB round-trip on every page load.  The JWT is still
        // cryptographically verified (signature + expiry).
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
        // GET /api/groups/:id/members — list members of a group
        .get_light("/api/groups/:id/members", |req, state, claims| async move {
            let chat_id = req
                .uri()
                .path()
                .split('/')
                .nth(3)
                .and_then(|s| s.parse::<i64>().ok());
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
        // ── Hard auth: JWT + DB session lookup + IP binding ───────────────────
        //
        // Every route that mutates state lives here.  Auth is performed by the
        // router before the handler is invoked; handlers receive the verified
        // user_id directly and must NOT repeat the auth call.
        // ── Messaging ────────────────────────────────────────────────────────
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
                    .uri()
                    .path()
                    .split('/')
                    .nth(3)
                    .and_then(|s| s.parse::<i64>().ok());
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
        // POST /api/typing — fire-and-forget typing indicator over SSE
        .post_hard("/api/typing", |req, state, user_id, _claims| async move {
            messaging::handle_typing(req, state, user_id)
                .await
                .context("Typing indicator failed")
        })
        // DELETE /api/messages/:id — delete own message (sender only)
        .delete_hard(
            "/api/messages/:id",
            |req, state, user_id, _claims| async move {
                let message_id = req
                    .uri()
                    .path()
                    .split('/')
                    .nth(3)
                    .and_then(|s| s.parse::<i64>().ok());
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
        // GET /api/unread — unread message counts (optional ?chat_id=N)
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
                    .uri()
                    .path()
                    .split('/')
                    .nth(3)
                    .and_then(|s| s.parse::<i64>().ok());
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
                    .uri()
                    .path()
                    .split('/')
                    .nth(3)
                    .and_then(|s| s.parse::<i64>().ok());
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
        // PATCH /api/groups/:id — rename a group
        .patch_hard(
            "/api/groups/:id",
            |req, state, user_id, _claims| async move {
                let chat_id = req
                    .uri()
                    .path()
                    .split('/')
                    .nth(3)
                    .and_then(|s| s.parse::<i64>().ok());
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
        // DELETE /api/groups/:id — delete a group
        .delete_hard(
            "/api/groups/:id",
            |req, state, user_id, _claims| async move {
                let chat_id = req
                    .uri()
                    .path()
                    .split('/')
                    .nth(3)
                    .and_then(|s| s.parse::<i64>().ok());
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
        // GET /api/users/search?q= — search users by username
        .get_light("/api/users/search", |req, state, claims| async move {
            messaging::handle_search_users(req, state, claims)
                .await
                .context("User search failed")
        })
        // ── Profile ──────────────────────────────────────────────────────────
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
        // ── Settings ─────────────────────────────────────────────────────────
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
        // Logout: hard auth so we have the verified session_id to revoke.
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
        // ── Password reset ───────────────────────────────────────────────────
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
