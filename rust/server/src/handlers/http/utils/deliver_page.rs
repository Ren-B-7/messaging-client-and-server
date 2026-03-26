use std::path::{Path, PathBuf};

use bytes::Bytes;
use http::HeaderValue;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{Response, StatusCode, header};
use std::convert::Infallible;
use tracing::{debug, info};

use crate::handlers::http::utils::headers;
use shared::types::cache::CacheStrategy;
use shared::types::page_error::PageError;

/// Expand tilde (~) in path to home directory.
pub fn expand_tilde<P: AsRef<Path>>(path: P) -> Result<PathBuf, PageError> {
    let path_ref: &Path = path.as_ref();
    let path_str: &str = path_ref
        .to_str()
        .ok_or_else(|| PageError::InvalidUtf8(path_ref.to_path_buf()))?;

    if let Some(s) = path_str.strip_prefix("~/") {
        let home = std::env::var("HOME").map_err(|_| PageError::HomeMissing)?;
        let mut home_path: PathBuf = PathBuf::from(home);
        home_path.push(s);
        return Ok(home_path);
    }

    Ok(path_ref.to_path_buf())
}

/// Read an HTML file from disk and deliver it with no-cache headers.
///
/// HTML pages are always served with `NoCache` because they contain auth
/// state and must never be served stale from a proxy or browser cache.
pub fn deliver_html_page<P: AsRef<Path>>(
    file_path: P,
) -> Result<Response<BoxBody<Bytes, Infallible>>, PageError> {
    deliver_page_with_status(file_path, StatusCode::OK, CacheStrategy::NoCache)
}

/// Deliver a static page from a file path with the specified caching policy.
///
/// This is the core function that all file-based delivery helpers delegate to.
/// Cache headers are applied according to [`CacheStrategy`]:
///
/// - [`CacheStrategy::LongTerm`]  → `public, max-age=31536000` (1 year)
/// - [`CacheStrategy::ShortTerm`] → `public, max-age=3600` (1 hour)
/// - [`CacheStrategy::NoCache`]   → `no-cache, no-store, must-revalidate`
pub fn deliver_page_with_status<P: AsRef<Path>>(
    file_path: P,
    status: StatusCode,
    cache: CacheStrategy,
) -> Result<Response<BoxBody<Bytes, Infallible>>, PageError> {
    let expanded_path: PathBuf = expand_tilde(file_path)?;

    debug!(
        "Reading static file from: {} (cache: {})",
        expanded_path.display(),
        cache
    );

    let content: Vec<u8> = std::fs::read(&expanded_path)
        .map_err(|e| PageError::ReadFailed(expanded_path.clone(), e))?;

    let content_bytes: Bytes = Bytes::from(content);
    let mime_type: &str = get_mime_type(&expanded_path);

    debug!(
        "Delivering static page — status: {}, size: {} bytes, mime: {}, cache: {}",
        status,
        content_bytes.len(),
        mime_type,
        cache
    );

    let response: Response<BoxBody<Bytes, Infallible>> = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime_type)
        .body(full(content_bytes))
        .map_err(PageError::ResponseBuildFailed)?;

    let response_with_cache = match cache {
        CacheStrategy::LongTerm => headers::add_cache_headers_with_max_age(response, None),
        CacheStrategy::ShortTerm => headers::add_cache_headers_with_max_age(response, Some(3600)),
        CacheStrategy::NoCache => headers::add_no_cache_headers(response),
    };

    Ok(response_with_cache)
}

/// Deliver a static page with an ETag for conditional-GET support.
pub fn deliver_page_with_etag<P: AsRef<Path>>(
    file_path: P,
    status: StatusCode,
    cache: CacheStrategy,
    etag: &str,
) -> Result<Response<BoxBody<Bytes, Infallible>>, PageError> {
    let expanded_path: PathBuf = expand_tilde(&file_path)?;

    debug!(
        "Reading static file with ETag from: {} (etag: {})",
        expanded_path.display(),
        etag
    );

    let response = deliver_page_with_status(&expanded_path, status, cache)?;
    let response_with_etag = headers::add_etag_header(response, etag);

    Ok(response_with_etag)
}

/// Determine the MIME type from the file extension.
pub fn get_mime_type(path: &Path) -> &'static str {
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

        _ => "application/octet-stream",
    }
}

/// Deliver a redirect response.
pub fn deliver_redirect(location: &str) -> Result<Response<BoxBody<Bytes, Infallible>>, PageError> {
    info!("Delivering redirect to: {}", location);

    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, location)
        .body(full(Bytes::from("")))
        .map_err(PageError::ResponseBuildFailed)
}

/// Deliver a redirect response with a cookie header.
pub fn deliver_redirect_with_cookie(
    location: &str,
    cookie: HeaderValue,
) -> Result<Response<BoxBody<Bytes, Infallible>>, PageError> {
    info!("Delivering redirect to: {}", location);

    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, location)
        .header(header::SET_COOKIE, cookie)
        .body(full(Bytes::from("")))
        .map_err(PageError::ResponseBuildFailed)
}

/// Deliver a plain text response.
pub fn deliver_text<T: Into<Bytes>>(
    text: T,
) -> Result<Response<BoxBody<Bytes, Infallible>>, PageError> {
    let bytes: Bytes = text.into();
    debug!("Delivering text response, size: {} bytes", bytes.len());

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(full(bytes))
        .map_err(PageError::ResponseBuildFailed)
}

/// Build an empty boxed body.
pub fn empty() -> BoxBody<Bytes, Infallible> {
    Empty::<Bytes>::new().boxed()
}

/// Build a full (single-chunk) boxed body from any `Into<Bytes>` value.
pub fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    Full::new(chunk.into()).boxed()
}
