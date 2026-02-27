use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{Method, Request, Response, StatusCode};
use tracing::{info, warn};

use crate::AppState;
use crate::handlers::http::utils::headers::{decode_jwt_claims, validate_jwt_secure};
use crate::handlers::http::{admin, messaging, profile, utils::*};

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
    //
    // The router decodes and verifies the JWT before the handler is called.
    // Handlers receive `JwtClaims` and must NOT call `decode_jwt_claims`
    // themselves — the work is already done.
    //
    // Use for: GET / HEAD requests that only read data.

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
    //
    // The router runs the full `validate_jwt_secure` pipeline before the
    // handler is called.  Handlers receive the verified `user_id` (i64) and
    // `JwtClaims` and must NOT call any auth function themselves.
    //
    // Use for: POST / PUT / DELETE — anything that mutates state.

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
                        unauthorized()
                    }
                },

                // ── Hard: JWT + DB session lookup + IP binding ────────────────
                RouteKind::Hard(h) => match validate_jwt_secure(&req, &state).await {
                    Ok((user_id, claims)) => h(req, state, user_id, claims).await,
                    Err(reason) => {
                        warn!("Hard-auth rejected {} {}: {}", method, path, reason);
                        unauthorized()
                    }
                },
            };
        }

        // No registered route matched — try static file fallback for GET.
        if method == Method::GET {
            if let Some(static_response) = self.try_serve_static(&path, &state).await? {
                return Ok(static_response);
            }
        }

        json_response::deliver_error_json("NOT_FOUND", "Endpoint not found", StatusCode::NOT_FOUND)
            .context("Failed to deliver 404 response")
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
                info!("icons: {}", icons);
                let file_path = format!("{}/{}{}", web_dir, icons, path);
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
                Ok(Some(
                    deliver_html_page(&file_path).context("Failed to deliver HTML file")?,
                ))
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

fn forbidden() -> Result<Response<BoxBody<Bytes, Infallible>>> {
    json_response::deliver_error_json(
        "FORBIDDEN",
        "Insufficient privileges",
        StatusCode::FORBIDDEN,
    )
    .context("Failed to deliver 403 response")
}

// ---------------------------------------------------------------------------
// Shared base API router
//
// Auth tier is enforced here at the routing level — handlers MUST NOT repeat
// the auth call.  The contract is:
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
        //
        // These are the only routes where auth is intentionally absent.
        // /api/register and /api/login are handled by the specific sub-routers
        // (user.rs / admin.rs) so they are NOT registered here.
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
                    http_body_util::Full::new(Bytes::from(r#"{"status":"success","health":"ok"}"#))
                        .boxed(),
                )
                .unwrap())
        })
        // ── Light auth: JWT decode only, zero DB reads ────────────────────────
        //
        // Safe for GET requests because reading stale data carries no real
        // risk and avoids a DB round-trip on every page load.  The JWT is
        // still cryptographically verified (signature + expiry).
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
        // ── Hard auth: JWT + DB session lookup + IP binding ───────────────────
        //
        // Every route that mutates state lives here.  Auth is performed by
        // the router before the handler is invoked; handlers receive the
        // verified user_id directly and must NOT repeat the auth call.
        // Messaging
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
        // Chats / groups
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
                let group_id = req
                    .uri()
                    .path()
                    .split('/')
                    .nth(3)
                    .and_then(|s| s.parse::<i64>().ok());
                match group_id {
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
                let group_id = req
                    .uri()
                    .path()
                    .split('/')
                    .nth(3)
                    .and_then(|s| s.parse::<i64>().ok());
                match group_id {
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
        // Profile
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
        // Settings
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
            |req, state, user_id, _claims| async move {
                profile::handle_logout_all(req, state, user_id)
                    .await
                    .context("Logout-all failed")
            },
        )
        // Logout: uses hard auth so we have the verified session_id to revoke.
        .post_hard("/api/logout", |req, state, user_id, claims| async move {
            profile::handle_logout(req, state, user_id, claims)
                .await
                .context("Logout failed")
        })
}

// ---------------------------------------------------------------------------
// Admin-specific base router
//
// Admin routes use the Hard tier everywhere — there are no read-only admin
// operations that are safe with JWT-only auth.  The is_admin flag is also
// checked inside `require_admin` after Hard auth succeeds.
// ---------------------------------------------------------------------------

pub fn build_admin_api_routes(router: Router) -> Router {
    router
        // Stats — hard auth + admin flag check inside handler.
        .post_hard(
            "/admin/api/stats",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return forbidden();
                }
                admin::handle_server_config(req, state, user_id)
                    .await
                    .context("Stats failed")
            },
        )
        .post_hard(
            "/admin/api/users",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return forbidden();
                }
                admin::handle_get_users(req, state, user_id)
                    .await
                    .context("Get users failed")
            },
        )
        // Ban / unban
        .post_hard(
            "/admin/api/users/ban",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return forbidden();
                }
                admin::handle_ban_user(req, state, user_id)
                    .await
                    .context("Ban user failed")
            },
        )
        .post_hard(
            "/admin/api/users/unban",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return forbidden();
                }
                admin::handle_unban_user(req, state, user_id)
                    .await
                    .context("Unban user failed")
            },
        )
        // Promote / demote
        .post_hard(
            "/admin/api/users/promote",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return forbidden();
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
                    return forbidden();
                }
                admin::handle_demote_user(req, state, user_id)
                    .await
                    .context("Demote failed")
            },
        )
        // Delete user
        .delete_hard(
            "/admin/api/users/:id",
            |req, state, user_id, claims| async move {
                if !claims.is_admin {
                    return forbidden();
                }
                admin::handle_delete_user(req, state, user_id)
                    .await
                    .context("Delete user failed")
            },
        )
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
    fn trailing_slash_does_not_match_without_slash() {
        assert!(!Router::path_matches("/api/profile", "/api/profile/"));
    }

    #[test]
    fn root_path_matches_self() {
        assert!(Router::path_matches("/", "/"));
    }

    #[test]
    fn wildcard_segment_matches_numeric_id() {
        assert!(Router::path_matches("/admin/users/:id", "/admin/users/42"));
    }

    #[test]
    fn wildcard_segment_matches_string_id() {
        assert!(Router::path_matches(
            "/api/groups/:id/members",
            "/api/groups/99/members"
        ));
    }

    #[test]
    fn wildcard_does_not_match_extra_segments() {
        assert!(!Router::path_matches(
            "/api/groups/:id",
            "/api/groups/99/members"
        ));
    }

    #[test]
    fn query_string_stripped_before_match() {
        assert!(Router::path_matches(
            "/api/messages",
            "/api/messages?limit=50&offset=0"
        ));
    }

    #[test]
    fn static_prefix_detection() {
        assert!("/static/app.js".starts_with("/static/"));
        assert!(!"/app.js".starts_with("/static/"));
    }

    #[test]
    fn html_suffix_detection() {
        assert!("/about.html".ends_with(".html"));
        assert!(!"/about.css".ends_with(".html"));
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
        let r = Router::new().post_hard("/api/test", |_req, _state, _uid, _claims| async move {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(http_body_util::Full::new(Bytes::from("ok")).boxed())
                .unwrap())
        });
        assert_eq!(r.routes.len(), 1);
        assert!(matches!(r.routes[0].kind, RouteKind::Hard(_)));
    }

    #[tokio::test]
    async fn router_put_hard_adds_hard_route() {
        let r = Router::new().put_hard("/api/test", |_req, _state, _uid, _claims| async move {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(http_body_util::Full::new(Bytes::from("ok")).boxed())
                .unwrap())
        });
        assert_eq!(r.routes.len(), 1);
        assert!(matches!(r.routes[0].kind, RouteKind::Hard(_)));
    }

    #[tokio::test]
    async fn router_delete_hard_adds_hard_route() {
        let r = Router::new().delete_hard("/api/test", |_req, _state, _uid, _claims| async move {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(http_body_util::Full::new(Bytes::from("ok")).boxed())
                .unwrap())
        });
        assert_eq!(r.routes.len(), 1);
        assert!(matches!(r.routes[0].kind, RouteKind::Hard(_)));
    }
}
