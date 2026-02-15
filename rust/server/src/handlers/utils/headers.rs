use anyhow::{Result, anyhow};
use hyper::header::{HeaderMap, HeaderValue};
use hyper::Request;
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

/// Add CORS headers to a response
pub fn add_cors_headers<T>(mut res: hyper::Response<T>, origin: &str) -> hyper::Response<T> {
    let headers = res.headers_mut();

    headers.insert(
        "access-control-allow-origin",
        HeaderValue::from_str(origin).unwrap(),
    );
    headers.insert(
        "access-control-allow-methods",
        HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"),
    );
    headers.insert(
        "access-control-allow-headers",
        HeaderValue::from_static("Content-Type, Authorization"),
    );
    headers.insert("access-control-max-age", HeaderValue::from_static("86400"));

    res
}

/// Add security headers to a response
pub fn add_security_headers<T>(mut res: hyper::Response<T>) -> hyper::Response<T> {
    let headers = res.headers_mut();

    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert(
        "x-xss-protection",
        HeaderValue::from_static("1; mode=block"),
    );
    headers.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );

    res
}

/// Extract bearer token from Authorization header
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
