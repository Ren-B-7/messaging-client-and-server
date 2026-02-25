use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use bytes::Bytes;
use http::HeaderValue;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{Response, StatusCode, header};
use std::convert::Infallible;
use tracing::{debug, error, info};

use crate::handlers::http::utils::headers;
use shared::types::cache::CacheStrategy;

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
    // Just delegate everything to the core function
    deliver_page_with_status(file_path, StatusCode::OK, CacheStrategy::Explicit)
}

/// Deliver a static page from a file path with caching headers
/// This is the core function that handles all file-based deliveries
pub fn deliver_page_with_status<P: AsRef<Path>>(
    file_path: P,
    status: StatusCode,
    cache: CacheStrategy,
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
        .map_err(|e| anyhow!("Failed to build response: {}", e))?;

    // Apply specific caching logic
    let response_with_cache = match cache {
        CacheStrategy::Yes => headers::add_cache_headers_with_max_age(response, None),
        CacheStrategy::No => headers::add_cache_headers_with_max_age(response, Some(3600)),
        CacheStrategy::Explicit => headers::add_no_cache_headers(response),
    };
    Ok(response_with_cache)
}
/// Deliver a static page with ETag support for efficient caching
pub fn deliver_page_with_etag<P: AsRef<Path>>(
    file_path: P,
    status: StatusCode,
    cache: CacheStrategy,
    etag: &str,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let expanded_path: PathBuf = expand_tilde(&file_path);

    debug!(
        "Reading static file with ETag from: {} (etag: {})",
        expanded_path.display(),
        etag
    );
    let response = deliver_page_with_status(file_path, status, cache).unwrap();
    let response_with_etag = headers::add_etag_header(response, etag);

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

/// Delivers a redirect response
pub fn deliver_redirect(location: &str) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Delivering redirect to: {}", location);

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

/// Delivers a redirect response
pub fn deliver_redirect_with_cookie(
    location: &str,
    cookie: Option<HeaderValue>,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Delivering redirect to: {}", location);

    let empty_bytes: Bytes = Bytes::from("");
    let mut builder = Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, location);
    if let Some(c) = cookie {
        info!("Setting cookie {}", c.to_str().unwrap_or_default());
        builder = builder.header(header::SET_COOKIE, c);
    }
    let response = builder.body(full(empty_bytes)).map_err(|e: http::Error| {
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

// handlers/http/utils/deliver_page.rs  — append at the bottom
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ── get_mime_type ─────────────────────────────────────────────────────────

    #[test]
    fn mime_html() {
        assert_eq!(
            get_mime_type(std::path::Path::new("page.html")),
            "text/html; charset=utf-8"
        );
    }

    #[test]
    fn mime_htm_alias() {
        assert_eq!(
            get_mime_type(std::path::Path::new("page.htm")),
            "text/html; charset=utf-8"
        );
    }

    #[test]
    fn mime_css() {
        assert_eq!(
            get_mime_type(std::path::Path::new("style.css")),
            "text/css; charset=utf-8"
        );
    }

    #[test]
    fn mime_js() {
        assert_eq!(
            get_mime_type(std::path::Path::new("app.js")),
            "application/javascript; charset=utf-8"
        );
    }

    #[test]
    fn mime_json() {
        assert_eq!(
            get_mime_type(std::path::Path::new("data.json")),
            "application/json"
        );
    }

    #[test]
    fn mime_png() {
        assert_eq!(get_mime_type(std::path::Path::new("img.png")), "image/png");
    }

    #[test]
    fn mime_svg() {
        assert_eq!(
            get_mime_type(std::path::Path::new("icon.svg")),
            "image/svg+xml"
        );
    }

    #[test]
    fn mime_woff2() {
        assert_eq!(
            get_mime_type(std::path::Path::new("font.woff2")),
            "font/woff2"
        );
    }

    #[test]
    fn mime_pdf() {
        assert_eq!(
            get_mime_type(std::path::Path::new("doc.pdf")),
            "application/pdf"
        );
    }

    #[test]
    fn mime_unknown_extension_is_octet_stream() {
        assert_eq!(
            get_mime_type(std::path::Path::new("file.xyz")),
            "application/octet-stream"
        );
    }

    #[test]
    fn mime_no_extension_is_octet_stream() {
        assert_eq!(
            get_mime_type(std::path::Path::new("Makefile")),
            "application/octet-stream"
        );
    }

    // ── full() helper ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn full_helper_wraps_bytes() {
        use bytes::Bytes;
        use http_body_util::BodyExt;
        let body = full(Bytes::from("hello"));
        let collected = body.collect().await.unwrap().to_bytes();
        assert_eq!(&collected[..], b"hello");
    }

    // ── deliver_page_with_status — happy path ─────────────────────────────────

    #[tokio::test]
    async fn deliver_page_reads_file_and_sets_content_type() {
        use http::StatusCode;
        use http_body_util::BodyExt;
        use shared::types::cache::CacheStrategy;

        let mut f = NamedTempFile::with_suffix(".html").unwrap();
        write!(f, "<html><body>test</body></html>").unwrap();

        let res =
            deliver_page_with_status(f.path(), StatusCode::OK, CacheStrategy::Explicit).unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["content-type"], "text/html; charset=utf-8");
        let body = res.collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"<html><body>test</body></html>");
    }

    #[test]
    fn deliver_page_missing_file_returns_error() {
        use http::StatusCode;
        use shared::types::cache::CacheStrategy;
        let result = deliver_page_with_status(
            "/nonexistent/path/file.html",
            StatusCode::OK,
            CacheStrategy::Explicit,
        );
        assert!(result.is_err());
    }

    // ── deliver_redirect ──────────────────────────────────────────────────────

    #[test]
    fn deliver_redirect_302_and_location() {
        use http::StatusCode;
        let res = deliver_redirect("/new-location").unwrap();
        assert_eq!(res.status(), StatusCode::FOUND);
        assert_eq!(res.headers()["location"], "/new-location");
    }

    // ── deliver_text ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn deliver_text_body_and_content_type() {
        use http_body_util::BodyExt;
        let res = deliver_text(b"hello world".to_vec()).unwrap();
        assert_eq!(res.headers()["content-type"], "text/plain; charset=utf-8");
        let body = res.collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"hello world");
    }

    // ── expand_tilde ──────────────────────────────────────────────────────────

    #[test]
    fn expand_tilde_without_home_env_returns_original() {
        // Without HOME set this returns the path unchanged; with HOME it expands.
        // Just check the function doesn't panic with a non-tilde path.
        let p = expand_tilde("/absolute/path/file.html");
        assert_eq!(p.to_str().unwrap(), "/absolute/path/file.html");
    }
}
