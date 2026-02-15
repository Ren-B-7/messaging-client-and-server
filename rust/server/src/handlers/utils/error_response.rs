use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Response, StatusCode, header};
use serde::{Serialize, Deserialize};
use serde_json::json;
use tracing::error;

/// Standard error response structure
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: String,
    pub code: String,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            status: "error".to_string(),
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

/// Delivers a JSON error response with the specified error code, message, and status
pub fn deliver_error_json(
    error_code: &str,
    message: &str,
    status: StatusCode,
) -> Result<Response<Full<Bytes>>> {
    error!(
        "Delivering error JSON: {} - {} ({})",
        status.as_u16(),
        error_code,
        message
    );

    let error_json = json!({
        "status": "error",
        "code": error_code,
        "message": message
    });

    let json_string: String = error_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(json_bytes))
        .map_err(|e: http::Error| {
            error!("Failed to build error JSON response: {}", e);
            anyhow::anyhow!("Failed to build error JSON response: {}", e)
        })?;

    Ok(response)
}

/// Delivers a success JSON response with optional data
pub fn deliver_success_json<T: serde::Serialize>(
    data: Option<T>,
) -> Result<Response<Full<Bytes>>> {
    let response_body = match data {
        Some(d) => json!({
            "status": "success",
            "data": d
        }),
        None => json!({
            "status": "success"
        }),
    };

    let json_string: String = response_body.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(json_bytes))
        .map_err(|e: http::Error| {
            error!("Failed to build success JSON response: {}", e);
            anyhow::anyhow!("Failed to build success JSON response: {}", e)
        })?;

    Ok(response)
}
