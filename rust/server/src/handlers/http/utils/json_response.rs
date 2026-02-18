use anyhow::{Context, Result, anyhow};
use bytes::Bytes;
use http::HeaderValue;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::{Response, StatusCode, header};
use serde::Serialize;
use serde_json::json;
use std::convert::Infallible;
use tracing::{debug, error};

/// Serialize any `Serialize` type and deliver it as a JSON response.
/// This is the primary helper all handlers should use instead of
/// writing their own one-off serialization + response-building blocks.
pub fn deliver_serialized_json<T: Serialize>(
    data: &T,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let json = serde_json::to_string(data).context("Failed to serialize response")?;

    debug!(
        "Delivering serialized JSON response, size: {} bytes",
        json.len()
    );
    let response: Response<BoxBody<Bytes, Infallible>> = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(json)).boxed())
        .map_err(|e| anyhow!("Failed to build JSON response: {}", e))?;

    Ok(response)
}

pub fn deliver_serialized_json_with_cookie<T: Serialize>(
    data: &T,
    status: StatusCode,
    cookie: Option<HeaderValue>,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let json = serde_json::to_string(data).context("Failed to serialize response")?;

    debug!(
        "Delivering serialized JSON response, size: {} bytes",
        json.len()
    );
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(c) = cookie {
        builder = builder.header(header::SET_COOKIE, c);
    }
    let response = builder
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
