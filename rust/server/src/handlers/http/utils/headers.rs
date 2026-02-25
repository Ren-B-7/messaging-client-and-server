use anyhow::{Result, anyhow};
use hyper::Request;
use hyper::header::{HeaderMap, HeaderValue};
use std::time::Duration;
use tracing::{debug, warn};

/// Extract a header value as a string
pub fn get_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(name).and_then(|v| v.to_str().ok()).map(|s| {
        debug!("Retrieved header: {}", name);
        s.to_string()
    })
}

/// Check if a header exists and matches a value
pub fn header_matches(headers: &HeaderMap, name: &str, value: &str) -> bool {
    get_header_value(headers, name)
        .map(|v| v.eq_ignore_ascii_case(value))
        .unwrap_or(false)
}

/// Extract cookie value by name
pub fn get_cookie(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let mut parts = cookie.trim().splitn(2, '=');
                let name = parts.next()?.trim();
                let value = parts.next()?.trim();
                if name == cookie_name {
                    debug!("Cookie found: {}", cookie_name);
                    Some(value.to_string())
                } else {
                    None
                }
            })
        })
        .or_else(|| {
            warn!("Cookie not found: {}", cookie_name);
            None
        })
}

/// Set a cookie with options
pub fn set_cookie(
    name: &str,
    value: &str,
    max_age: Option<Duration>,
    path: Option<&str>,
    http_only: bool,
    secure: bool,
) -> Result<HeaderValue> {
    let mut cookie = format!("{}={}", name, value);

    if let Some(age) = max_age {
        cookie.push_str(&format!("; Max-Age={}", age.as_secs()));
    }

    if let Some(p) = path {
        cookie.push_str(&format!("; Path={}", p));
    }

    if http_only {
        cookie.push_str("; HttpOnly");
    }

    if secure {
        cookie.push_str("; Secure");
    }

    cookie.push_str("; SameSite=Strict");

    debug!("Setting cookie: {}", name);

    HeaderValue::from_str(&cookie).map_err(|e| {
        warn!("Failed to create cookie header for {}: {}", name, e);
        anyhow!("Invalid cookie value: {}", e)
    })
}

/// Create a session cookie (expires when browser closes)
pub fn create_session_cookie(name: &str, value: &str, secure: bool) -> Result<HeaderValue> {
    debug!("Creating session cookie: {}", name);
    set_cookie(name, value, None, Some("/"), true, secure)
}

/// Create a persistent cookie with expiration
pub fn create_persistent_cookie(
    name: &str,
    value: &str,
    max_age: Duration,
    secure: bool,
) -> Result<HeaderValue> {
    debug!(
        "Creating persistent cookie: {} with max_age: {:?}",
        name, max_age
    );
    set_cookie(name, value, Some(max_age), Some("/"), true, secure)
}

/// Delete a cookie by setting it to expire
pub fn delete_cookie(name: &str) -> Result<HeaderValue> {
    debug!("Deleting cookie: {}", name);
    set_cookie(
        name,
        "",
        Some(Duration::from_secs(0)),
        Some("/"),
        true,
        false,
    )
}

/// Extract the client IP address from the request
pub fn get_client_ip(req: &Request<hyper::body::Incoming>) -> Option<String> {
    // Check X-Forwarded-For header first (for proxied requests)
    if let Some(forwarded) = get_header_value(req.headers(), "x-forwarded-for") {
        return forwarded.split(',').next().map(|s| s.trim().to_string());
    }

    // Check X-Real-IP header
    if let Some(real_ip) = get_header_value(req.headers(), "x-real-ip") {
        return Some(real_ip);
    }

    None
}

/// Extract the user agent string
pub fn get_user_agent(req: &Request<hyper::body::Incoming>) -> Option<String> {
    get_header_value(req.headers(), "user-agent")
}

/// Check if the request accepts a specific content type
pub fn accepts_content_type(req: &Request<hyper::body::Incoming>, content_type: &str) -> bool {
    get_header_value(req.headers(), "accept")
        .map(|accept| accept.contains(content_type))
        .unwrap_or(false)
}

/// Add no-cache headers for non-static files
pub fn add_no_cache_headers<T>(mut res: hyper::Response<T>) -> hyper::Response<T> {
    let headers = res.headers_mut();

    headers.insert(
        "cache-control",
        HeaderValue::from_static("no-cache, no-store, must-revalidate"),
    );
    headers.insert("pragma", HeaderValue::from_static("no-cache"));
    headers.insert("expires", HeaderValue::from_static("0"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );

    debug!("Added no-cache headers");
    res
}

/// Add custom cache headers with specified max-age
pub fn add_cache_headers_with_max_age<T>(
    mut res: hyper::Response<T>,
    max_age_seconds: Option<u64>,
) -> hyper::Response<T> {
    let headers = res.headers_mut();
    let time = max_age_seconds.unwrap_or(31536000);

    let cache_control = format!("public, max-age={}", time);
    headers.insert(
        "cache-control",
        HeaderValue::from_str(&cache_control)
            .unwrap_or_else(|_| HeaderValue::from_static("public, max-age=3600")),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );

    debug!("Added cache headers with max-age: {} seconds", time);
    res
}

/// Add ETag header for cache validation
pub fn add_etag_header<T>(mut res: hyper::Response<T>, etag: &str) -> hyper::Response<T> {
    let headers = res.headers_mut();

    if let Ok(etag_value) = HeaderValue::from_str(etag) {
        headers.insert("etag", etag_value);
        debug!("Added ETag header: {}", etag);
    } else {
        warn!("Failed to add invalid ETag: {}", etag);
    }

    res
}

/// Check if request has matching ETag (for 304 Not Modified responses)
pub fn check_etag_match(req: &Request<hyper::body::Incoming>, etag: &str) -> bool {
    if let Some(if_none_match) = get_header_value(req.headers(), "if-none-match") {
        let matches = if_none_match == etag;
        if matches {
            debug!("ETag match found: {}", etag);
        }
        return matches;
    }
    false
}

/// Add Last-Modified header
pub fn add_last_modified_header<T>(
    mut res: hyper::Response<T>,
    last_modified: &str,
) -> hyper::Response<T> {
    let headers = res.headers_mut();

    if let Ok(last_modified_value) = HeaderValue::from_str(last_modified) {
        headers.insert("last-modified", last_modified_value);
        debug!("Added Last-Modified header: {}", last_modified);
    } else {
        warn!("Failed to add invalid Last-Modified: {}", last_modified);
    }

    res
}

/// Check if request has If-Modified-Since header (for 304 responses)
pub fn check_if_modified_since(
    req: &Request<hyper::body::Incoming>,
    last_modified_timestamp: u64,
) -> bool {
    if let Some(if_modified_since) = get_header_value(req.headers(), "if-modified-since") {
        // Parse the If-Modified-Since header and compare with last_modified_timestamp
        // This is a simplified version - proper implementation would parse HTTP date format
        debug!("Checking If-Modified-Since: {}", if_modified_since);
        // For now, return true to indicate modification (always serve)
        return true;
    }
    true
}

/// Extract bearer token from Authorization header
/// Format: "Authorization: Bearer <token>"
pub fn get_bearer_token(req: &Request<hyper::body::Incoming>) -> Option<String> {
    get_header_value(req.headers(), "authorization").and_then(|auth| {
        if auth.starts_with("Bearer ") {
            debug!("Bearer token extracted");
            Some(auth[7..].to_string())
        } else {
            warn!("Invalid or missing Bearer token");
            None
        }
    })
}

/// Extract session token from either Bearer header OR auth_id cookie
/// Checks Authorization header with Bearer scheme first, then falls back to auth_id cookie
/// This unified approach allows both authentication methods and should be used by all handlers
pub fn extract_session_token(req: &Request<hyper::body::Incoming>) -> Option<String> {
    // Try Bearer token first
    if let Some(token) = get_bearer_token(req) {
        debug!("Using session token from Bearer header");
        return Some(token);
    }

    // Fall back to auth_id cookie
    if let Some(token) = get_cookie(req.headers(), "auth_id") {
        debug!("Using session token from auth_id cookie");
        return Some(token);
    }

    debug!("No session token found in Bearer header or auth_id cookie");
    None
}

/// Validate token with full security checks (for POST/PUT/DELETE state-changing requests).
///
/// This is the SECURE path — queries the database and verifies IP/UA match.
///
/// Verifies:
/// - Token exists in the database and hasn't expired
/// - Request IP matches the IP the session was created from (prevents stolen-token replay)
/// - User-agent prefix is similar (warns on device change, does not block)
pub async fn validate_token_secure(
    req: &Request<hyper::body::Incoming>,
    state: &crate::AppState,
) -> std::result::Result<i64, String> {
    use crate::database::login as db_login;

    let token = extract_session_token(req).ok_or("No authentication token")?;

    // validate_session now returns Option<Session>, which carries ip_address / user_agent
    let session = db_login::validate_session(&state.db, token)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or("Invalid or expired session")?;

    let current_ip = get_client_ip(req).unwrap_or_else(|| "unknown".to_string());
    let current_ua = get_user_agent(req).unwrap_or_else(|| "unknown".to_string());

    // CRITICAL: Reject requests where the IP has changed — likely a stolen token
    if let Some(ref stored_ip) = session.ip_address {
        if stored_ip != &current_ip {
            warn!(
                "SECURITY: session IP mismatch. user_id={}, original={}, current={}",
                session.user_id, stored_ip, current_ip
            );
            return Err("Session IP mismatch - possible token theft".to_string());
        }
    }

    // OPTIONAL: Warn when the user-agent prefix changed (browser update, device swap, etc.)
    if let Some(ref stored_ua) = session.user_agent {
        let stored_prefix  = &stored_ua[..30_usize.min(stored_ua.len())];
        let current_prefix = &current_ua[..30_usize.min(current_ua.len())];
        if stored_prefix != current_prefix {
            warn!(
                "Session device changed. user_id={}, original={}, current={}",
                session.user_id, stored_prefix, current_prefix
            );
            // Do not block — minor UA variations are common (browser updates etc.)
        }
    }

    debug!(
        "Secure session OK: user_id={}, ip={}",
        session.user_id, current_ip
    );

    Ok(session.user_id)
}

/// Extract basic auth credentials from Authorization header
pub fn get_basic_auth(req: &Request<hyper::body::Incoming>) -> Option<(String, String)> {
    get_header_value(req.headers(), "authorization").and_then(|auth| {
        if auth.starts_with("Basic ") {
            debug!("Basic auth credentials extracted");
            // TODO: Implement base64 decoding
            // Requires base64 crate
            None
        } else {
            warn!("Invalid or missing Basic auth");
            None
        }
    })
}
