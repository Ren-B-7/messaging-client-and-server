use anyhow::{Result, anyhow, Context};
use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{Response, StatusCode, header};
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use tracing::{debug, error};

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
pub fn deliver_html_page<S: AsRef<str>>(html: S) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    deliver_html_page_with_status(html.as_ref(), StatusCode::OK)
}

/// Read an HTML file from disk and deliver it
pub fn deliver_html_file<P: AsRef<Path>>(
    file_path: P,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let expanded_path: PathBuf = expand_tilde(file_path);
    
    debug!("Reading HTML file from: {}", expanded_path.display());
    
    let html_content: String = std::fs::read_to_string(&expanded_path)
        .with_context(|| format!("Failed to read HTML file: {}", expanded_path.display()))?;
    
    debug!("Successfully read {} bytes from {}", html_content.len(), expanded_path.display());
    
    deliver_html_page_with_status(html_content, StatusCode::OK)
}

/// Deliver a page from a file path with a specific status code
pub fn deliver_page_with_status<P: AsRef<Path>>(
    file_path: P,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let expanded_path: PathBuf = expand_tilde(file_path);
    
    debug!("Reading file from: {}", expanded_path.display());
    
    let content: Vec<u8> = std::fs::read(&expanded_path)
        .with_context(|| format!("Failed to read file: {}", expanded_path.display()))?;
    
    let content_bytes: Bytes = Bytes::from(content);
    
    // Determine MIME type based on file extension
    let mime_type: &str = get_mime_type(&expanded_path);
    
    debug!(
        "Delivering page with status: {}, size: {} bytes, mime: {}",
        status,
        content_bytes.len(),
        mime_type
    );

    let response: Response<BoxBody<Bytes, Infallible>> = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime_type)
        .body(full(content_bytes))
        .map_err(|e: http::Error| {
            error!("Failed to build page response: {}", e);
            anyhow!("Failed to build page response: {}", e)
        })?;
    
    Ok(response)
}

/// Deliver a static page from a file path with caching headers
pub fn deliver_static_page_with_status<P: AsRef<Path>>(
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
        crate::handlers::utils::headers::add_static_cache_headers(response)
    } else {
        crate::handlers::utils::headers::add_no_cache_headers(response)
    };
    
    Ok(response_with_cache)
}

/// Deliver a static page with ETag support for efficient caching
pub fn deliver_static_page_with_etag<P: AsRef<Path>>(
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

    // Build base response with static cache headers
    let response: Response<BoxBody<Bytes, Infallible>> = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime_type)
        .body(full(content_bytes))
        .map_err(|e: http::Error| {
            error!("Failed to build static page response: {}", e);
            anyhow!("Failed to build static page response: {}", e)
        })?;
    
    // Add static cache headers
    let response_with_cache = crate::handlers::utils::headers::add_static_cache_headers(response);
    
    // Add ETag header
    let response_with_etag = crate::handlers::utils::headers::add_etag_header(response_with_cache, etag);
    
    Ok(response_with_etag)
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

/// Delivers a page with a custom status code
pub fn deliver_html_page_with_status<T: AsRef<[u8]>>(
    html: T,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let html_bytes: &[u8] = html.as_ref();
    let bytes_string: Bytes = Bytes::copy_from_slice(html_bytes);
    
    debug!(
        "Delivering HTML page with status: {}, size: {} bytes",
        status,
        bytes_string.len()
    );

    let response: Response<BoxBody<Bytes, Infallible>> = apply_security_headers(Response::builder())
        .status(status)
        .body(full(bytes_string))
        .map_err(|e: http::Error| {
            error!("Failed to build HTML response: {}", e);
            anyhow!("Failed to build HTML response: {}", e)
        })?;
    
    Ok(response)
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

/// Delivers an error page with appropriate status code
pub fn deliver_error_page(
    status: StatusCode,
    message: &str,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    error!("Delivering error page: {} - {}", status, message);

    let status_code: u16 = status.as_u16();
    let html: String = format!(
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
        status_code,
        status_code,
        message
    );

    deliver_html_page_with_status(&html, status)
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
