use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use tracing::info;

use crate::AppState;
use crate::handlers::utils::error_response::ErrorResponse;

/// Handle get profile (requires authentication)
pub async fn handle_get_profile(
    req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    // Check for auth token
    let token = crate::handlers::utils::get_bearer_token(&req).or_else(|| {
        // Fallback to cookie
        crate::handlers::utils::get_cookie(req.headers(), "auth_token")
    });

    if token.is_none() {
        return deliver_error_json(
            "UNAUTHORIZED",
            "Authentication required",
            StatusCode::UNAUTHORIZED,
        );
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

    let json_string: String = profile_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build profile response")?;

    Ok(response)
}

/// Handle logout
pub async fn handle_logout(
    _req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    info!("User logged out");

    // Delete auth cookie
    let cookie_header: hyper::header::HeaderValue = crate::handlers::utils::delete_cookie("auth_token")
        .context("Failed to create delete cookie")?;

    let cookie: String = cookie_header.to_str()
        .map_err(|e| anyhow::anyhow!("Invalid cookie header value: {}", e))?
        .to_string();

    let response_json = serde_json::json!({
        "status": "success",
        "message": "Logged out successfully"
    });

    let json_string: String = response_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("set-cookie", cookie)
        .body(Full::new(json_bytes))
        .context("Failed to build logout response")?;

    Ok(response)
}

/// Deliver JSON error response
fn deliver_error_json(
    code: &str,
    message: &str,
    status: StatusCode,
) -> Result<Response<Full<Bytes>>> {
    let error: ErrorResponse = ErrorResponse::new(code, message);
    let json: String = serde_json::to_string(&error).unwrap_or_else(|_| {
        r#"{"status":"error","code":"INTERNAL_ERROR","message":"Failed to serialize error"}"#
            .to_string()
    });

    let json_bytes: Bytes = Bytes::from(json);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build error response")?;

    Ok(response)
}
