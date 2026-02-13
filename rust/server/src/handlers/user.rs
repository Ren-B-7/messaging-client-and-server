use std::net::SocketAddr;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Method, Request, Response, StatusCode, body::Incoming};
use serde::Serialize;
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::FormHandlers;
use crate::handlers::utils;

/// Standard error response
#[derive(Serialize)]
struct ErrorResponse {
    status: String,
    code: String,
    message: String,
}

impl ErrorResponse {
    fn new(code: &str, message: &str) -> Self {
        Self {
            status: "error".to_string(),
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

/// Main user connection handler
pub async fn user_conn(
    req: Request<Incoming>,
    addr: SocketAddr,
    state: AppState,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    info!("User request from {}: {} {}", addr, req.method(), req.uri());

    // Access config values
    let blocked_paths = &state.config.paths.blocked_paths;
    let path = req.uri().path().to_string();

    // Check if path is blocked
    if blocked_paths.contains(&path) {
        warn!("Blocked path access attempt from {}: {}", addr, path);
        return deliver_error_json("FORBIDDEN", "Access Denied", StatusCode::FORBIDDEN);
    }

    // Route requests - wrap in error handler
    let result = route_request(req, addr, state, &path).await;

    match result {
        Ok(response) => Ok(response),
        Err(e) => {
            error!("Request handler error for {}: {:?}", path, e);
            deliver_error_json(
                "INTERNAL_ERROR",
                "Internal Server Error",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

/// Route requests to appropriate handlers
async fn route_request(
    req: Request<Incoming>,
    addr: SocketAddr,
    state: AppState,
    path: &str,
) -> Result<Response<Full<Bytes>>> {
    match (req.method(), path) {
        // Static pages
        (&Method::GET, "/") => {
            let web_dir = &state.config.paths.web_dir;
            info!("Serving home page from web_dir: {}", web_dir);
            Ok(Response::new(Full::new(Bytes::from(
                r#"{"status":"success","message":"Welcome to the API"}"#,
            ))))
        }

        (&Method::GET, "/health") => Ok(Response::new(Full::new(Bytes::from(
            r#"{"status":"success","health":"ok"}"#,
        )))),

        // Authentication endpoints
        (&Method::POST, "/api/register") | (&Method::POST, "/register") => {
            info!("Processing registration from {}", addr);
            FormHandlers::register::handle_register(req, state)
                .await
                .map_err(|e| {
                    error!("Registration error: {:?}", e);
                    anyhow::anyhow!("Registration failed")
                })
        }

        (&Method::POST, "/api/login") | (&Method::POST, "/login") => {
            info!("Processing login from {}", addr);
            FormHandlers::login::handle_login(req, state)
                .await
                .map_err(|e| {
                    error!("Login error: {:?}", e);
                    anyhow::anyhow!("Login failed")
                })
        }

        (&Method::POST, "/api/logout") | (&Method::POST, "/logout") => {
            info!("Processing logout from {}", addr);
            handle_logout(req, state).await
        }

        // API endpoints
        (&Method::GET, "/api/config") => {
            let config_json = serde_json::json!({
                "status": "success",
                "data": {
                    "email_required": state.config.auth.email_required,
                    "token_expiry_minutes": state.config.auth.token_expiry_minutes
                }
            });

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Full::new(Bytes::from(config_json.to_string())))
                .context("Failed to build response")?)
        }

        (&Method::GET, "/api/profile") => {
            info!("Fetching profile for authenticated user");
            handle_get_profile(req, state).await
        }

        // 404 for all other routes
        _ => {
            warn!("404 Not Found: {} from {}", path, addr);
            deliver_error_json("NOT_FOUND", "Endpoint not found", StatusCode::NOT_FOUND)
                .map_err(|e| anyhow::anyhow!("Failed to deliver 404: {:?}", e))
        }
    }
}

/// Handle logout
async fn handle_logout(_req: Request<Incoming>, _state: AppState) -> Result<Response<Full<Bytes>>> {
    info!("User logged out");

    // Delete auth cookie
    let cookie = utils::delete_cookie("auth_token").context("Failed to create delete cookie")?;

    let response_json = serde_json::json!({
        "status": "success",
        "message": "Logged out successfully"
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("set-cookie", cookie)
        .body(Full::new(Bytes::from(response_json.to_string())))
        .context("Failed to build logout response")?;

    Ok(response)
}

/// Handle get profile (requires authentication)
async fn handle_get_profile(
    req: Request<Incoming>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    // Check for auth token
    let token = utils::get_bearer_token(&req).or_else(|| {
        // Fallback to cookie
        utils::get_cookie(req.headers(), "auth_token")
    });

    if token.is_none() {
        return deliver_error_json(
            "UNAUTHORIZED",
            "Authentication required",
            StatusCode::UNAUTHORIZED,
        )
        .map_err(|e| anyhow::anyhow!("Failed to deliver error: {:?}", e));
    }

    // TODO: Verify token and fetch user profile from database
    let profile_json = serde_json::json!({
        "status": "success",
        "data": {
            "user_id": 12345,
            "username": "demo",
            "email": "demo@example.com",
            "created_at": "2024-01-01T00:00:00Z"
        }
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(profile_json.to_string())))
        .context("Failed to build profile response")?)
}

/// Deliver JSON error response
fn deliver_error_json(
    code: &str,
    message: &str,
    status: StatusCode,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let error = ErrorResponse::new(code, message);
    let json = serde_json::to_string(&error).unwrap_or_else(|_| {
        r#"{"status":"error","code":"INTERNAL_ERROR","message":"Failed to serialize error"}"#
            .to_string()
    });

    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(json)))
        .unwrap())
}
