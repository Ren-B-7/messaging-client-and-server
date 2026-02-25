use anyhow::{Result, anyhow};
use hyper::Request;
use hyper::header::{HeaderMap, HeaderValue};
use std::time::Duration;
use tracing::{debug, warn};

use shared::types::jwt::JwtClaims;

// ---------------------------------------------------------------------------
// Generic header helpers
// ---------------------------------------------------------------------------

/// Extract a header value as a string.
pub fn get_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(name).and_then(|v| v.to_str().ok()).map(|s| {
        debug!("Retrieved header: {}", name);
        s.to_string()
    })
}

/// Check if a header exists and matches a value (case-insensitive ASCII).
pub fn header_matches(headers: &HeaderMap, name: &str, value: &str) -> bool {
    get_header_value(headers, name)
        .map(|v| v.eq_ignore_ascii_case(value))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Cookie helpers
// ---------------------------------------------------------------------------

/// Extract a single cookie value by name.
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

/// Build a `Set-Cookie` header value with the supplied options.
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
    cookie.push_str("; Partitioned");

    debug!("Setting cookie: {}", name);

    HeaderValue::from_str(&cookie).map_err(|e| {
        warn!("Failed to create cookie header for {}: {}", name, e);
        anyhow!("Invalid cookie value: {}", e)
    })
}

/// Create a session cookie (expires when browser closes).
pub fn create_session_cookie(name: &str, value: &str, https: bool) -> Result<HeaderValue> {
    debug!("Creating session cookie: {}", name);
    set_cookie(name, value, None, Some("/"), true, https)
}

/// Create a persistent cookie with an explicit max-age.
pub fn create_persistent_cookie(
    name: &str,
    value: &str,
    max_age: Duration,
    https: bool,
) -> Result<HeaderValue> {
    debug!(
        "Creating persistent cookie: {} with max_age: {:?}",
        name, max_age
    );
    set_cookie(name, value, Some(max_age), Some("/"), true, https)
}

/// Clear a cookie by setting its max-age to 0.
pub fn delete_cookie(name: &str) -> Result<HeaderValue> {
    debug!("Deleting cookie: {}", name);
    set_cookie(name, "", Some(Duration::from_secs(0)), Some("/"), true, false)
}

// ---------------------------------------------------------------------------
// IP / UA helpers
// ---------------------------------------------------------------------------

/// Extract the client IP from `X-Forwarded-For` → `X-Real-IP` → `None`.
pub fn get_client_ip(req: &Request<hyper::body::Incoming>) -> Option<String> {
    if let Some(forwarded) = get_header_value(req.headers(), "x-forwarded-for") {
        return forwarded.split(',').next().map(|s| s.trim().to_string());
    }
    if let Some(real_ip) = get_header_value(req.headers(), "x-real-ip") {
        return Some(real_ip);
    }
    None
}

/// Extract the `User-Agent` header value.
pub fn get_user_agent(req: &Request<hyper::body::Incoming>) -> Option<String> {
    get_header_value(req.headers(), "user-agent")
}

/// Returns `true` when the `Accept` header includes `content_type`.
pub fn accepts_content_type(req: &Request<hyper::body::Incoming>, content_type: &str) -> bool {
    get_header_value(req.headers(), "accept")
        .map(|accept| accept.contains(content_type))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Bearer / session token extraction
// ---------------------------------------------------------------------------

/// Extract a Bearer token from `Authorization: Bearer <token>`.
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

/// Extract a raw token string from the `Authorization: Bearer` header first,
/// then fall back to the `auth_id` cookie.
///
/// Returns the JWT string as-is without decoding it.  Call
/// `decode_jwt_claims` or `validate_jwt_secure` to get the actual claims.
pub fn extract_session_token(req: &Request<hyper::body::Incoming>) -> Option<String> {
    if let Some(token) = get_bearer_token(req) {
        debug!("Using session token from Bearer header");
        return Some(token);
    }
    if let Some(token) = get_cookie(req.headers(), "auth_id") {
        debug!("Using session token from auth_id cookie");
        return Some(token);
    }
    debug!("No session token found in Bearer header or auth_id cookie");
    None
}

// ---------------------------------------------------------------------------
// JWT helpers
// ---------------------------------------------------------------------------

/// Decode and cryptographically verify a JWT, returning the embedded claims.
///
/// **Fast path** — suitable for GET / read-only requests.  No database is
/// touched; only the HMAC signature is checked along with the `exp` field.
///
/// Returns `Err(String)` with a human-readable reason on any failure.
pub fn decode_jwt_claims(
    req: &Request<hyper::body::Incoming>,
    jwt_secret: &str,
) -> std::result::Result<JwtClaims, String> {
    use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};

    let token = extract_session_token(req).ok_or("No authentication token")?;

    let key = DecodingKey::from_secret(jwt_secret.as_bytes());
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    let token_data = decode::<JwtClaims>(&token, &key, &validation)
        .map_err(|e| format!("JWT decode failed: {}", e))?;

    debug!(
        "JWT fast-path OK: user_id={} session_id={}",
        token_data.claims.user_id,
        token_data.claims.session_id
    );

    Ok(token_data.claims)
}

/// Decode the JWT **and** perform full security validation against the database.
///
/// **Secure path** — required for every POST / PUT / DELETE handler.
///
/// Steps:
///   1. Extract & decode the JWT (same as `decode_jwt_claims`).
///   2. Look up `session_id` in the DB → confirms the session hasn't been
///      revoked (logout / ban).
///   3. Compare `sessions.ip_address` against the current request IP.
///      Mismatches are **rejected** to block stolen-token replay attacks.
///   4. Compare the JWT `user_agent` claim against the current `User-Agent`
///      prefix.  Changes are **warned** but not blocked (browser updates).
///
/// Returns `(user_id, claims)` on success so callers don't need to re-decode
/// the token. Returns `Err(String)` with the rejection reason on any failure.
pub async fn validate_jwt_secure(
    req: &Request<hyper::body::Incoming>,
    state: &crate::AppState,
) -> std::result::Result<(i64, JwtClaims), String> {
    use crate::database::login as db_login;

    // Step 1 — cryptographic JWT verification (no DB).
    let claims = decode_jwt_claims(req, &state.jwt_secret)?;

    // Step 2 — confirm the session_id still exists and hasn't expired.
    let session = db_login::validate_session_id(&state.db, claims.session_id.clone())
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or("Session not found or expired — please log in again")?;

    let current_ip = get_client_ip(req).unwrap_or_else(|| "unknown".to_string());
    let current_ua = get_user_agent(req).unwrap_or_else(|| "unknown".to_string());

    // Step 3 — IP binding check (CRITICAL — reject on mismatch).
    if let Some(ref stored_ip) = session.ip_address {
        if stored_ip != &current_ip {
            warn!(
                "SECURITY: session IP mismatch. user_id={}, original={}, current={}",
                claims.user_id, stored_ip, current_ip
            );
            return Err("Session IP mismatch — possible token theft".to_string());
        }
    }

    // Step 4 — UA prefix check (warn-only — minor variations are common).
    let stored_ua_prefix = &claims.user_agent[..30_usize.min(claims.user_agent.len())];
    let current_ua_prefix = &current_ua[..30_usize.min(current_ua.len())];
    if stored_ua_prefix != current_ua_prefix {
        warn!(
            "Session device changed. user_id={}, original_ua_prefix={}, current_ua_prefix={}",
            claims.user_id, stored_ua_prefix, current_ua_prefix
        );
    }

    debug!(
        "Secure JWT OK: user_id={}, ip={}, session_id={}",
        claims.user_id, current_ip, claims.session_id
    );

    let user_id = claims.user_id;
    Ok((user_id, claims))
}

/// Build a signed JWT string from the supplied claims.
///
/// `jwt_secret` must be at least 32 bytes; shorter keys are rejected by the
/// `jsonwebtoken` crate.  Use `AppState.jwt_secret` in handlers.
pub fn encode_jwt(claims: &JwtClaims, jwt_secret: &str) -> Result<String> {
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

    encode(
        &Header::new(Algorithm::HS256),
        claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| anyhow!("JWT encode failed: {}", e))
}

// ---------------------------------------------------------------------------
// Cache / ETag helpers  (unchanged from original)
// ---------------------------------------------------------------------------

/// Replace all cache-related headers with `no-cache, no-store, must-revalidate`.
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

/// Set `Cache-Control: public, max-age=<seconds>`.
///
/// Pass `None` to use the default (1 year / 31 536 000 s).
pub fn add_cache_headers_with_max_age<T>(
    mut res: hyper::Response<T>,
    max_age_seconds: Option<u64>,
) -> hyper::Response<T> {
    let headers = res.headers_mut();
    let time = max_age_seconds.unwrap_or(31_536_000);
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

/// Attach an `ETag` header to a response.
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

/// Returns `true` when `If-None-Match` matches `etag` (→ send 304).
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

/// Attach a `Last-Modified` header to a response.
pub fn add_last_modified_header<T>(
    mut res: hyper::Response<T>,
    last_modified: &str,
) -> hyper::Response<T> {
    let headers = res.headers_mut();
    if let Ok(v) = HeaderValue::from_str(last_modified) {
        headers.insert("last-modified", v);
        debug!("Added Last-Modified header: {}", last_modified);
    } else {
        warn!("Failed to add invalid Last-Modified: {}", last_modified);
    }
    res
}

/// Always returns `true` (always serve).  Proper HTTP-date parsing is a
/// future enhancement.
pub fn check_if_modified_since(
    _req: &Request<hyper::body::Incoming>,
    _last_modified_timestamp: u64,
) -> bool {
    true
}

/// Extract basic auth credentials (base64 decoding not yet implemented).
pub fn get_basic_auth(req: &Request<hyper::body::Incoming>) -> Option<(String, String)> {
    get_header_value(req.headers(), "authorization").and_then(|auth| {
        if auth.starts_with("Basic ") {
            debug!("Basic auth credentials extracted");
            // TODO: Implement base64 decoding
            None
        } else {
            warn!("Invalid or missing Basic auth");
            None
        }
    })
}
