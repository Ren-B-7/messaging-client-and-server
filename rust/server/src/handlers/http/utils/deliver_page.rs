use anyhow::{Context, Result, anyhow};
use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{Response, StatusCode, header};
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use tracing::{debug, error};

use crate::handlers::http::utils::headers;

/// Expand tilde (~) in path to home directory
fn expand_tilde<P: AsRef<Path>>(path: P) -> PathBuf {
    let path_ref: &Path = path.as_ref();
    let path_str: &str = path_ref.to_str().unwrap_or("");

    if path_str.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            let mut home_path: PathBuf = PathBuf::from(home);
            home_path.push(&path_str[2..]);
            return home_path;
        }
    }

    path_ref.to_path_buf()
}

/// Read an HTML file from disk and deliver it with security headers
pub fn deliver_html_page<P: AsRef<Path>>(
    file_path: P,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let expanded_path: PathBuf = expand_tilde(file_path);

    debug!("Reading HTML file from: {}", expanded_path.display());

    let html_content: String = std::fs::read_to_string(&expanded_path)
        .with_context(|| format!("Failed to read HTML file: {}", expanded_path.display()))?;

    debug!(
        "Successfully read {} bytes from {}",
        html_content.len(),
        expanded_path.display()
    );

    deliver_html_page_with_status(html_content, StatusCode::OK)
}

/// Delivers HTML content with a custom status code and security headers
/// Uses deliver_page_with_status internally for consistent handling
pub fn deliver_html_page_with_status<T: AsRef<[u8]>>(
    html: T,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let html_bytes: &[u8] = html.as_ref();
    let bytes: Bytes = Bytes::copy_from_slice(html_bytes);

    debug!(
        "Delivering HTML page with status: {}, size: {} bytes",
        status,
        bytes.len()
    );

    // Build HTML response manually since we have content in memory, not a file path
    let response: Response<BoxBody<Bytes, Infallible>> = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(full(bytes))
        .map_err(|e: http::Error| {
            error!("Failed to build HTML response: {}", e);
            anyhow!("Failed to build HTML response: {}", e)
        })?;

    // Apply security headers for HTML content
    let response_with_security = headers::add_security_headers(response);

    Ok(response_with_security)
}

/// Deliver a static page from a file path with caching headers
/// This is the core function that handles all file-based deliveries
pub fn deliver_page_with_status<P: AsRef<Path>>(
    file_path: P,
    status: StatusCode,
    cache: bool,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let expanded_path: PathBuf = expand_tilde(file_path);

    debug!(
        "Reading static file from: {} (cache: {})",
        expanded_path.display(),
        cache
    );

    let content: Vec<u8> = std::fs::read(&expanded_path)
        .with_context(|| format!("Failed to read static file: {}", expanded_path.display()))?;

    let content_bytes: Bytes = Bytes::from(content);

    // Determine MIME type based on file extension
    let mime_type: &str = get_mime_type(&expanded_path);

    debug!(
        "Delivering static page with status: {}, size: {} bytes, mime: {}, cache: {}",
        status,
        content_bytes.len(),
        mime_type,
        cache
    );

    // Build base response
    let response: Response<BoxBody<Bytes, Infallible>> = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime_type)
        .body(full(content_bytes))
        .map_err(|e: http::Error| {
            error!("Failed to build static page response: {}", e);
            anyhow!("Failed to build static page response: {}", e)
        })?;

    // Apply appropriate cache headers using header utilities
    let response_with_cache = if cache {
        headers::add_static_cache_headers(response)
    } else {
        headers::add_no_cache_headers(response)
    };

    // Apply security headers for HTML content
    let is_html = mime_type.starts_with("text/html");
    let final_response = if is_html {
        headers::add_security_headers(response_with_cache)
    } else {
        response_with_cache
    };

    Ok(final_response)
}

/// Deliver a static page with ETag support for efficient caching
pub fn deliver_page_with_etag<P: AsRef<Path>>(
    file_path: P,
    status: StatusCode,
    etag: &str,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let expanded_path: PathBuf = expand_tilde(file_path);

    debug!(
        "Reading static file with ETag from: {} (etag: {})",
        expanded_path.display(),
        etag
    );

    let content: Vec<u8> = std::fs::read(&expanded_path)
        .with_context(|| format!("Failed to read static file: {}", expanded_path.display()))?;

    let content_bytes: Bytes = Bytes::from(content);

    // Determine MIME type based on file extension
    let mime_type: &str = get_mime_type(&expanded_path);

    debug!(
        "Delivering static page with ETag: status: {}, size: {} bytes, mime: {}, etag: {}",
        status,
        content_bytes.len(),
        mime_type,
        etag
    );

    // Build base response with content type
    let response: Response<BoxBody<Bytes, Infallible>> = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime_type)
        .body(full(content_bytes))
        .map_err(|e: http::Error| {
            error!("Failed to build static page response: {}", e);
            anyhow!("Failed to build static page response: {}", e)
        })?;

    // Add static cache headers and ETag using headers.rs functions
    let response_with_cache = headers::add_static_cache_headers(response);
    let response_with_etag = headers::add_etag_header(response_with_cache, etag);

    // Apply security headers for HTML content
    let is_html = mime_type.starts_with("text/html");
    let final_response = if is_html {
        headers::add_security_headers(response_with_etag)
    } else {
        response_with_etag
    };

    Ok(final_response)
}

/// Helper function to determine MIME type from file extension
fn get_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()) {
        // Web documents
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("xml") => "application/xml",

        // Images
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("avif") => "image/avif",

        // Fonts
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("eot") => "application/vnd.ms-fontobject",

        // Media
        Some("mp3") => "audio/mpeg",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",

        // Documents
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain; charset=utf-8",
        Some("md") => "text/markdown; charset=utf-8",

        // Archives
        Some("zip") => "application/zip",
        Some("gz") => "application/gzip",
        Some("tar") => "application/x-tar",

        // Default
        _ => "application/octet-stream",
    }
}

/// Delivers a JSON response
pub fn deliver_json<T: Into<Bytes>>(json: T) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let bytes_string: Bytes = json.into();

    debug!(
        "Delivering JSON response, size: {} bytes",
        bytes_string.len()
    );

    let response: Response<BoxBody<Bytes, Infallible>> = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(full(bytes_string))
        .map_err(|e: http::Error| {
            error!("Failed to build JSON response: {}", e);
            anyhow!("Failed to build JSON response: {}", e)
        })?;

    Ok(response)
}

/// Delivers a redirect response
pub fn deliver_redirect(location: &str) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    debug!("Delivering redirect to: {}", location);

    let empty_bytes: Bytes = Bytes::from("");
    let response: Response<BoxBody<Bytes, Infallible>> = Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, location)
        .body(full(empty_bytes))
        .map_err(|e: http::Error| {
            error!("Failed to build redirect response to {}: {}", location, e);
            anyhow!("Failed to build redirect response: {}", e)
        })?;

    Ok(response)
}

/// Delivers a plain text response
pub fn deliver_text<T: Into<Bytes>>(text: T) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let bytes_string: Bytes = text.into();

    debug!(
        "Delivering text response, size: {} bytes",
        bytes_string.len()
    );

    let response: Response<BoxBody<Bytes, Infallible>> = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(full(bytes_string))
        .map_err(|e: http::Error| {
            error!("Failed to build text response: {}", e);
            anyhow!("Failed to build text response: {}", e)
        })?;

    Ok(response)
}

/// Helper function to create an empty body
pub fn empty() -> BoxBody<Bytes, Infallible> {
    Empty::<Bytes>::new().boxed()
}

/// Helper function to create a full body from various types
/// Made public for use in error handling
pub fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    let bytes: Bytes = chunk.into();
    let full_body: Full<Bytes> = Full::new(bytes);
    full_body.boxed()
}
