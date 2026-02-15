use std::collections::HashSet;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::body::Incoming as IncomingBody;
use hyper::service::Service;
use hyper::{Method, Request, Response, StatusCode};
use std::convert::Infallible;
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::end_points;
use crate::handlers::form_handlers;
use crate::handlers::utils::*;

/// Helper function to convert Full<Bytes> response to BoxBody<Bytes, Infallible>
fn convert_response_body(
    response: Response<http_body_util::Full<Bytes>>,
) -> Response<BoxBody<Bytes, Infallible>> {
    let (parts, body) = response.into_parts();
    let boxed_body: BoxBody<Bytes, Infallible> = body.boxed();
    Response::from_parts(parts, boxed_body)
}

/// Helper to convert Result with Full body to Result with BoxBody
/// Accepts Result with anyhow::Error which all handlers should use
fn convert_result_body(
    result: Result<Response<http_body_util::Full<Bytes>>>,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    result.map(convert_response_body)
}

/// User service implementation
#[derive(Clone, Debug)]
pub struct UserService {
    state: AppState,
    addr: SocketAddr,
}

impl UserService {
    pub fn new(state: AppState, addr: SocketAddr) -> Self {
        Self { state, addr }
    }
}

impl Service<Request<IncomingBody>> for UserService {
    type Response = Response<BoxBody<Bytes, Infallible>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<IncomingBody>) -> Self::Future {
        let state = self.state.clone();
        let addr = self.addr;

        Box::pin(async move {
            match user_conn(req, addr, state).await {
                Ok(response) => Ok(response),
                Err(e) => {
                    error!("User handler error: {:?}", e);
                    // Return a proper error response with BoxBody type
                    match deliver_error_json(
                        "INTERNAL_ERROR",
                        "Internal Server Error",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    ) {
                        Ok(err_response) => Ok(convert_response_body(err_response)),
                        Err(delivery_err) => {
                            error!("Failed to deliver error response: {:?}", delivery_err);
                            // Last resort: manually construct response
                            let fallback_body = crate::handlers::utils::deliver_page::full(
                                Bytes::from(
                                    r#"{"status":"error","code":"INTERNAL_ERROR","message":"Internal Server Error"}"#,
                                ),
                            );
                            Ok(Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .header("content-type", "application/json")
                                .body(fallback_body)
                                .expect("Failed to build fallback error response"))
                        }
                    }
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
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("User request from {}: {} {}", addr, req.method(), req.uri());

    // Access config values
    let blocked_paths: &HashSet<String> = &state.config.paths.blocked_paths;
    let path: String = req.uri().path().to_string();

    // Check if path is blocked
    if blocked_paths.contains(&path) {
        warn!("Blocked path access attempt from {}: {}", addr, path);
        return convert_result_body(deliver_error_json(
            "FORBIDDEN",
            "Access Denied",
            StatusCode::FORBIDDEN,
        ))
        .context("Failed to deliver FORBIDDEN error response");
    }

    // Route requests - wrap in error handler
    let result: Result<Response<BoxBody<Bytes, Infallible>>> =
        route_request(req, addr, state, &path).await;

    match result {
        Ok(response) => Ok(response),
        Err(e) => {
            error!("Request handler error for {}: {:?}", path, e);
            convert_result_body(deliver_error_json(
                "INTERNAL_ERROR",
                "Internal Server Error",
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
            .context("Failed to deliver INTERNAL_ERROR response")
        }
    }
}

/// Serve a static file with appropriate caching headers
fn deliver_static_file(
    file_path: &str,
    cache: bool,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    crate::handlers::utils::deliver_static_page_with_status(file_path, StatusCode::OK, cache)
}

/// Route requests to appropriate handlers
async fn route_request(
    req: Request<IncomingBody>,
    addr: SocketAddr,
    state: AppState,
    path: &str,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let web_dir: &String = &state.config.paths.web_dir;
    let page_path: String = format!("{}{}", web_dir, path);

    match (req.method(), path) {
        // Static pages
        (&Method::GET, "/") | (&Method::GET, "/index.html") => {
            let file_path: String = format!("{}/index.html", web_dir);
            crate::handlers::utils::deliver_page::deliver_html_file(&file_path)
                .context("Failed to deliver HTML page")
        }

        (&Method::GET, "/health") => {
            deliver_json(Bytes::from(r#"{"status":"success","health":"ok"}"#))
                .context("Failed to deliver health check response")
        }

        // Serve any .html file from the frontend directory
        (&Method::GET, path) if path.ends_with(".html") => {
            let file_path: String = format!("{}{}", web_dir, path);
            info!("Serving HTML file: {}", file_path);
            crate::handlers::utils::deliver_page::deliver_html_file(&file_path)
                .context("Failed to deliver HTML file")
        }

        // Static files - cached (1 year)
        (&Method::GET, path) if path.starts_with("/static/") => {
            let file_path: String = format!("{}{}", web_dir, path);
            info!("Serving static file (cached): {}", file_path);
            deliver_static_file(&file_path, true).context("Failed to deliver static file")
        }

        // Non-static files - not cached
        (&Method::GET, path) if path.starts_with("/non-static/") => {
            let file_path: String = format!("{}{}", web_dir, path);
            info!("Serving non-static file (no cache): {}", file_path);
            deliver_static_file(&file_path, false).context("Failed to deliver non-static file")
        }

        // Authentication endpoints
        (&Method::POST, "/api/register") | (&Method::POST, "/register") => {
            info!("Processing registration from {}", addr);
            convert_result_body(form_handlers::register::handle_register(req, state).await)
                .context("Registration failed")
        }

        (&Method::POST, "/api/login") | (&Method::POST, "/login") => {
            info!("Processing login from {}", addr);
            convert_result_body(form_handlers::login::handle_login(req, state).await)
                .context("Login failed")
        }

        (&Method::POST, "/api/logout") | (&Method::POST, "/logout") => {
            info!("Processing logout from {}", addr);
            convert_result_body(end_points::profile::handle_logout(req, state).await)
                .context("Logout failed")
        }

        // API endpoints
        (&Method::GET, "/api/config") => {
            let email_required: bool = state.config.auth.email_required;
            let token_expiry: u64 = state.config.auth.token_expiry_minutes;

            let config_json = serde_json::json!({
                "status": "success",
                "data": {
                    "email_required": email_required,
                    "token_expiry_minutes": token_expiry
                }
            });

            let json_string: String = config_json.to_string();
            deliver_json(Bytes::from(json_string)).context("Failed to build config response")
        }

        (&Method::GET, "/api/profile") => {
            info!("Fetching profile for authenticated user");
            convert_result_body(end_points::profile::handle_get_profile(req, state).await)
                .context("Failed to get profile")
        }

        // 404 for all other routes
        _ => {
            warn!("404 Not Found: {} from {}", path, addr);
            convert_result_body(deliver_error_json(
                "NOT_FOUND",
                "Endpoint not found",
                StatusCode::NOT_FOUND,
            ))
            .context("Failed to deliver 404 response")
        }
    }
}
