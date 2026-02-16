use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::{Method, Request, Response, StatusCode};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use tracing::info;

use crate::AppState;
use crate::handlers::http::utils;
use crate::handlers::http::{auth, messaging, profile, utils::*};

/// Type alias for route handler functions
type RouteHandler = Box<
    dyn Fn(
            Request<hyper::body::Incoming>,
            AppState,
        )
            -> Pin<Box<dyn Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send>>
        + Send
        + Sync,
>;

/// Route definition
struct Route {
    method: Method,
    path: String,
    handler: RouteHandler,
}

/// HTTP Router with builder pattern for route registration
pub struct Router {
    routes: Vec<Route>,
    web_dir: Option<String>,
    icons_dir: Option<String>,
}

// Manual Debug implementation since RouteHandler contains function pointers
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
    /// Create a new Router instance
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            web_dir: None,
            icons_dir: None,
        }
    }

    /// Set web directory for static file serving
    pub fn with_web_dir(mut self, web_dir: String) -> Self {
        self.web_dir = Some(web_dir);
        self
    }

    /// Set icons directory for icon serving
    pub fn with_icons_dir(mut self, icons_dir: String) -> Self {
        self.icons_dir = Some(icons_dir);
        self
    }

    /// Register a GET route
    pub fn get<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::GET,
            path: path.to_string(),
            handler: Box::new(move |req, state| Box::pin(handler(req, state))),
        });
        self
    }

    /// Register a POST route
    pub fn post<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::POST,
            path: path.to_string(),
            handler: Box::new(move |req, state| Box::pin(handler(req, state))),
        });
        self
    }

    /// Register a DELETE route
    pub fn delete<F, Fut>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(Request<hyper::body::Incoming>, AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response<BoxBody<Bytes, Infallible>>>> + Send + 'static,
    {
        self.routes.push(Route {
            method: Method::DELETE,
            path: path.to_string(),
            handler: Box::new(move |req, state| Box::pin(handler(req, state))),
        });
        self
    }

    /// Route a request to the appropriate handler
    pub async fn route(
        &self,
        req: Request<hyper::body::Incoming>,
        state: AppState,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>> {
        let method = req.method().clone();
        let path = req.uri().path().to_string();

        // Try to match against registered routes
        for route in &self.routes {
            if route.method == method && Self::path_matches(&route.path, &path) {
                let response = (route.handler)(req, state).await?;
                return Ok(response); // No conversion needed - already BoxBody
            }
        }

        // If no route matched, try static file serving for GET requests
        if method == Method::GET {
            if let Some(static_response) = self.try_serve_static(&path, &state)? {
                return Ok(static_response);
            }
        }

        // No route or static file matched - return 404
        convert_result_body(utils::error_response::deliver_error_json(
            "NOT_FOUND",
            "Endpoint not found",
            StatusCode::NOT_FOUND,
        ))
        .context("Failed to deliver 404 response")
    }

    /// Check if a route path matches the request path
    /// Supports exact matches and path parameters (e.g., "/users/:id")
    fn path_matches(route_path: &str, request_path: &str) -> bool {
        // For now, just do exact matching
        // Can be extended to support path parameters later
        route_path == request_path
    }

    /// Try to serve static files for GET requests
    fn try_serve_static(
        &self,
        path: &str,
        state: &AppState,
    ) -> Result<Option<Response<BoxBody<Bytes, Infallible>>>> {
        let web_dir = self.web_dir.as_ref().unwrap_or(&state.config.paths.web_dir);
        let icons = self.icons_dir.as_ref().unwrap_or(&state.config.paths.icons);

        match path {
            // Home page
            "/" | "/index.html" => {
                let file_path = format!("{}/index.html", web_dir);
                Ok(Some(
                    crate::handlers::http::utils::deliver_page::deliver_html_page(&file_path)
                        .context("Failed to deliver HTML page")?,
                ))
            }

            // Static files - cached (1 year)
            path if path.starts_with("/static/") => {
                let file_path = format!("{}{}", web_dir, path);
                Ok(Some(
                    deliver_page_with_status(&file_path, StatusCode::OK, true)
                        .context("Failed to deliver static file")?,
                ))
            }

            // Favicon and browser icons
            "/favicon.ico"
            | "/favicon.png"
            | "/favicon.svg"
            | "/apple-touch-icon.png"
            | "/apple-touch-icon-precomposed.png"
            | "/android-chrome-192x192.png"
            | "/android-chrome-512x512.png"
            | "/browserconfig.xml"
            | "/site.webmanifest" => {
                let filename = path.trim_start_matches('/');
                let file_path = format!("{}{}{}", web_dir, icons, filename);
                Ok(Some(
                    deliver_page_with_status(&file_path, StatusCode::OK, true)
                        .context("Failed to deliver browser icon")?,
                ))
            }

            // Non-static files - not cached
            path if path.starts_with("/non-static/") => {
                let file_path = format!("{}{}", web_dir, path);
                Ok(Some(
                    deliver_page_with_status(&file_path, StatusCode::OK, false)
                        .context("Failed to deliver non-static file")?,
                ))
            }

            // Any .html file from the frontend directory
            path if path.ends_with(".html") => {
                let file_path = format!("{}{}", web_dir, path);
                Ok(Some(
                    crate::handlers::http::utils::deliver_page::deliver_html_page(&file_path)
                        .context("Failed to deliver HTML file")?,
                ))
            }

            _ => Ok(None),
        }
    }
}

/// Build the user-facing API router
pub fn build_user_router() -> Router {
    build_user_router_with_config(None, None)
}

/// Build the user-facing API router with custom web_dir and icons_dir
pub fn build_user_router_with_config(web_dir: Option<String>, icons_dir: Option<String>) -> Router {
    let mut router = Router::new();

    // Set directories if provided
    if let Some(dir) = web_dir {
        router = router.with_web_dir(dir);
    }
    if let Some(dir) = icons_dir {
        router = router.with_icons_dir(dir);
    }

    router
        // Auth endpoints
        .get("/login", |_req, state| async move {
            let file_path = format!("{}index.html", state.config.paths.web_dir);
            crate::handlers::http::utils::deliver_page::deliver_html_page(&file_path)
                .context("Failed to deliver login page")
        })
        .get("/", |_req, state| async move {
            let file_path = format!("{}index.html", state.config.paths.web_dir);
            crate::handlers::http::utils::deliver_page::deliver_html_page(&file_path)
                .context("Failed to deliver login page")
        })
        .get("/index", |_req, state| async move {
            let file_path = format!("{}index.html", state.config.paths.web_dir);
            crate::handlers::http::utils::deliver_page::deliver_html_page(&file_path)
                .context("Failed to deliver login page")
        })
        .get("/register", |_req, state| async move {
            let file_path = format!("{}register.html", state.config.paths.web_dir);
            crate::handlers::http::utils::deliver_page::deliver_html_page(&file_path)
                .context("Failed to deliver register page")
        })
        .get("/settings", |_req, state| async move {
            let file_path = format!("{}settings.html", state.config.paths.web_dir);
            crate::handlers::http::utils::deliver_page::deliver_html_page(&file_path)
                .context("Failed to deliver register page")
        })
        .get("/chat", |_req, state| async move {
            let file_path = format!("{}chat.html", state.config.paths.web_dir);
            crate::handlers::http::utils::deliver_page::deliver_html_page(&file_path)
                .context("Failed to deliver register page")
        })
        .post("/api/register", |req, state| async move {
            convert_result_body(auth::handle_register(req, state).await).context("Register failed")
        })
        .post("/api/login", |req, state| async move {
            convert_result_body(auth::handle_login(req, state).await)
                .context("Login attempt failed")
        })
        .post("/api/logout", |req, state| async move {
            convert_result_body(auth::handle_logout(req, state).await).context("Logout failed")
        })
        // Profile & Settings
        .post("/api/profile/update", |req, state| async move {
            convert_result_body(profile::handle_update_profile(req, state).await)
                .context("Profile update failed")
        })
        .post("/api/settings/password", |req, state| async move {
            convert_result_body(profile::handle_change_password(req, state).await)
                .context("Password change failed")
        })
        .post("/api/settings/logout-all", |req, state| async move {
            convert_result_body(profile::handle_logout_all(req, state).await)
                .context("Logout attempt failed")
        })
        .get("/api/profile", |req, state| async move {
            convert_result_body(profile::handle_get_profile(req, state).await)
                .context("Profile get failed")
        })
        // Messaging & Chats
        .post("/api/messages/send", |req, state| async move {
            convert_result_body(messaging::handle_send_message(req, state).await)
                .context("Message send failed")
        })
        .get("/api/messages", |req, state| async move {
            convert_result_body(messaging::handle_get_messages(req, state).await)
                .context("Message get attempt failed")
        })
        .post("/api/chats", |req, state| async move {
            convert_result_body(messaging::handle_create_chat(req, state).await)
                .context("Create chat failed")
        })
        .get("/api/chats", |req, state| async move {
            convert_result_body(messaging::handle_get_chats(req, state).await)
                .context("Chat get attempt failed")
        })
        .post("/api/groups", |req, state| async move {
            convert_result_body(messaging::handle_create_group(req, state).await)
                .context("Create group failed")
        })
        .get("/api/groups", |req, state| async move {
            convert_result_body(messaging::handle_get_groups(req, state).await)
                .context("Group chat get attempt failed")
        })
        // Config & Health
        .get("/api/config", |_req, state| async move {
            let config_json = serde_json::json!({
                "status": "success",
                "data": {
                    "email_required": state.config.auth.email_required,
                    "token_expiry_minutes": state.config.auth.token_expiry_minutes
                }
            });
            let response = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(http_body_util::Full::new(Bytes::from(
                    config_json.to_string(),
                )))
                .context("Failed to build config response")?;
            Ok(convert_response_body(response))
        })
        .get("/health", |_req, _state| async move {
            let response = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(http_body_util::Full::new(Bytes::from(
                    r#"{"status":"success","health":"ok"}"#,
                )))
                .unwrap();
            Ok(convert_response_body(response))
        })
}
