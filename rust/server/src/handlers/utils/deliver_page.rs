use anyhow::{Result, anyhow};
use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{Response, StatusCode, header};
use std::{convert::Infallible, vec};
use tracing::{debug, error};

fn apply_security_headers(builder: http::response::Builder) -> http::response::Builder {
    builder
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::X_FRAME_OPTIONS, "DENY")
        .header(
            header::STRICT_TRANSPORT_SECURITY,
            "max-age=31536000; includeSubDomains",
        )
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'self'; script-src 'self'",
        )
        .header(header::REFERRER_POLICY, "no-referrer")
}

/// Delivers an HTML page with proper headers (Default OK status)
pub fn deliver_html_page(html: &str) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    deliver_html_page_with_status(html, StatusCode::OK)
}

/// Delivers a page with a custom status code
pub fn deliver_html_page_with_status<T: AsRef<[u8]>>(
    html: T,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let bytes_string = Bytes::copy_from_slice(html.as_ref());
    let bytes_vec: Vec<Bytes> = vec![bytes_string.clone()];
    debug!(
        "Delivering HTML page with status: {}, size: {} bytes",
        status,
        bytes_vec.len()
    );

    apply_security_headers(Response::builder())
        .status(status)
        .body(full(bytes_string))
        .map_err(|e| {
            error!("Failed to build HTML response: {}", e);
            anyhow!("Failed to build HTML response: {}", e)
        })
}

/// Delivers a JSON response
pub fn deliver_json<T: Into<Bytes>>(json: T) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let bytes_string = json.into();
    let bytes_vec: Vec<Bytes> = vec![bytes_string.clone()];
    debug!("Delivering JSON response, size: {} bytes", bytes_vec.len());

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(full(bytes_string))
        .map_err(|e| {
            error!("Failed to build JSON response: {}", e);
            anyhow!("Failed to build JSON response: {}", e)
        })
}

/// Delivers a redirect response
pub fn deliver_redirect(location: &str) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    debug!("Delivering redirect to: {}", location);

    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, location)
        .body(full(""))
        .map_err(|e| {
            error!("Failed to build redirect response to {}: {}", location, e);
            anyhow!("Failed to build redirect response: {}", e)
        })
}

/// Delivers a plain text response
pub fn deliver_text<T: Into<Bytes>>(text: T) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let bytes_string = text.into();
    let bytes_vec: Vec<Bytes> = vec![bytes_string.clone()];
    debug!("Delivering text response, size: {} bytes", bytes_vec.len());

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(full(bytes_string))
        .map_err(|e| {
            error!("Failed to build text response: {}", e);
            anyhow!("Failed to build text response: {}", e)
        })
}

/// Delivers an error page with appropriate status code
pub fn deliver_error_page(
    status: StatusCode,
    message: &str,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    error!("Delivering error page: {} - {}", status, message);

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>{} - Error</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background-color: #f5f5f5;
        }}
        .error-container {{
            text-align: center;
            padding: 2rem;
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }}
        h1 {{ color: #d32f2f; }}
        p {{ color: #666; }}
    </style>
</head>
<body>
    <div class="error-container">
        <h1>{}</h1>
        <p>{}</p>
        <a href="/">Go Home</a>
    </div>
</body>
</html>"#,
        status.as_u16(),
        status.as_u16(),
        message
    );

    deliver_html_page_with_status(&html, status)
}

/// Helper function to create an empty body
pub fn empty() -> BoxBody<Bytes, Infallible> {
    Empty::<Bytes>::new().boxed()
}

/// Helper function to create a full body from various types
fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    Full::new(chunk.into()).boxed()
}
