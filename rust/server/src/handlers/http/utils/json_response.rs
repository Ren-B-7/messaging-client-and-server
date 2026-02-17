use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::{Response, StatusCode, header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::Infallible;
use tracing::{debug, error};

use crate::handlers::http::utils::deliver_page::full;

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

/// Serialize any `Serialize` type and deliver it as a JSON response.
/// This is the primary helper all handlers should use instead of
/// writing their own one-off serialization + response-building blocks.
pub fn deliver_serialized_json<T: Serialize>(
    data: &T,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let json = serde_json::to_string(data).context("Failed to serialize response")?;

    debug!("Delivering serialized JSON response, size: {} bytes", json.len());

    let response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(json)).boxed())
        .map_err(|e| anyhow!("Failed to build JSON response: {}", e))?;

    Ok(response)
}

/// Delivers a JSON error response with the specified error code, message, and status.
pub fn deliver_error_json(
    error_code: &str,
    message: &str,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
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

    let json_string = error_json.to_string();

    let response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(json_string)).boxed())
        .map_err(|e: http::Error| {
            error!("Failed to build error JSON response: {}", e);
            anyhow!("Failed to build error JSON response: {}", e)
        })?;

    Ok(response)
}

/// Delivers a success JSON response with optional data.
pub fn deliver_success_json<T: Serialize>(
    data: Option<T>,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let response_body = match data {
        Some(d) => json!({
            "status": "success",
            "data": d
        }),
        None => json!({
            "status": "success"
        }),
    };

    let json_string = response_body.to_string();

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(json_string)).boxed())
        .map_err(|e: http::Error| {
            error!("Failed to build success JSON response: {}", e);
            anyhow!("Failed to build success JSON response: {}", e)
        })?;

    Ok(response)
}

/// Delivers a JSON response from raw pre-serialized bytes.
/// Prefer `deliver_serialized_json` when you have a typed value.
pub fn deliver_json<T: Into<Bytes>>(
    json: T,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let bytes: Bytes = json.into();

    debug!("Delivering raw JSON response, size: {} bytes", bytes.len());

    let response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(full(bytes))
        .map_err(|e: http::Error| {
            error!("Failed to build JSON response: {}", e);
            anyhow!("Failed to build JSON response: {}", e)
        })?;

    Ok(response)
}
