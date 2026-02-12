use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::{header, Response, StatusCode};
use std::convert::Infallible;

/// Delivers an HTML page with proper headers
pub fn deliver_html_page(html: &str) -> Response<BoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .body(full(html))
        .expect("valid response")
}

/// Delivers a page with a custom status code
pub fn deliver_html_page_with_status(
    html: &str,
    status: StatusCode,
) -> Response<BoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .body(full(html))
        .expect("valid response")
}

/// Delivers a JSON response
pub fn deliver_json(json: &str) -> Response<BoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(full(json))
        .expect("valid response")
}

/// Delivers a redirect response
pub fn deliver_redirect(location: &str) -> Response<BoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, location)
        .body(full(""))
        .expect("valid response")
}

/// Delivers a plain text response
pub fn deliver_text(text: &str) -> Response<BoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(full(text))
        .expect("valid response")
}

/// Delivers an error page with appropriate status code
pub fn deliver_error_page(
    status: StatusCode,
    message: &str,
) -> Response<BoxBody<Bytes, Infallible>> {
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
