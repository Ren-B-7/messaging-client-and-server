use anyhow::{Result, anyhow};
use httpdate;
use hyper::Request;
use hyper::header::{HeaderMap, HeaderValue};
use std::time::Duration;
use tracing::{debug, warn};

use shared::types::jwt::JwtClaims;

use crate::AppState;
use crate::database::login;

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
    set_cookie(
        name,
        "",
        Some(Duration::from_secs(0)),
        Some("/"),
        true,
        false,
    )
}

// ---------------------------------------------------------------------------
// IP / UA helpers
// ---------------------------------------------------------------------------

/// Extract the client IP from `X-Forwarded-For` → `X-Real-IP` → `None`.
pub fn get_client_ip<B>(req: &Request<B>) -> Option<String> {
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
pub fn get_bearer_token(headers: &HeaderMap) -> Option<String> {
    get_header_value(headers, "authorization").and_then(|auth| {
        if let Some(s) = auth.strip_prefix("Bearer ") {
            debug!("Bearer token extracted");
            Some(s.to_string())
        } else {
            warn!("Invalid or missing Bearer token");
            None
        }
    })
}

/// Extract a raw token string from the `Authorization: Bearer` header first,
/// then fall back to the `auth_id` cookie.
pub fn extract_session_token(req: &Request<hyper::body::Incoming>) -> Option<String> {
    if let Some(token) = get_bearer_token(req.headers()) {
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
        token_data.claims.user_id, token_data.claims.session_id
    );

    Ok(token_data.claims)
}

/// Decode the JWT **and** perform full security validation against the database.
///
/// **Secure path** — required for every POST / PUT / DELETE handler.
///
/// Steps:
///   1. Extract & decode the JWT (HMAC verify + expiry).
///   2. Look up `session_id` in the DB → confirms the session hasn't been
///      revoked (logout / ban).
///   3. Compare `sessions.ip_address` against the current request IP.
///      Behaviour is controlled by `AppConfig.auth.strict_ip_binding`:
///      - `true`  → **reject** on mismatch (strongest protection, breaks mobile/VPN)
///      - `false` → **warn** on mismatch (logs suspicious activity, never blocks)
///   4. Compare the JWT `user_agent` claim against the current `User-Agent`
///      prefix.  Changes are warn-only (browser updates are common).
///
/// Returns `(user_id, claims)` on success. Returns `Err(String)` with the
/// rejection reason on any failure.
pub async fn validate_jwt_secure(
    req: &Request<hyper::body::Incoming>,
    state: &AppState,
) -> std::result::Result<(i64, JwtClaims), String> {
    // Step 1 — cryptographic JWT verification (no DB).
    let claims = decode_jwt_claims(req, &state.jwt_secret)?;

    // Step 2 — confirm the session_id still exists and hasn't expired.
    let session = login::validate_session_id(&state.db, claims.session_id.clone())
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or("Session not found or expired — please log in again")?;

    let current_ip = get_client_ip(req).unwrap_or_else(|| "unknown".to_string());
    let current_ua = get_user_agent(req).unwrap_or_else(|| "unknown".to_string());

    // Step 3 — IP binding check.
    //
    // strict_ip_binding=true (default): hard reject on mismatch — strongest
    // stolen-token defence, but breaks mobile users who switch networks.
    //
    // strict_ip_binding=false: warn-only — suspicious IPs are logged so you
    // can see them in Grafana/Loki, but the request is not blocked. The JWT
    // signature and DB session check still prevent replay from a device that
    // never authenticated.
    if let Some(ref stored_ip) = session.ip_address
        && stored_ip != &current_ip
    {
        let strict = state.config.read().await.auth.strict_ip_binding;
        if strict {
            warn!(
                "SECURITY: session IP mismatch — rejecting. user_id={}, original={}, current={}",
                claims.user_id, stored_ip, current_ip
            );
            return Err("Session IP mismatch — possible token theft".to_string());
        } else {
            warn!(
                "SECURITY: session IP mismatch (warn-only, strict_ip_binding=false). \
                     user_id={}, original={}, current={}",
                claims.user_id, stored_ip, current_ip
            );
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
// Cache / ETag helpers
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

/// Check `If-Modified-Since` against a stored timestamp.
///
/// Returns `true` if the resource has been modified (i.e. should be served),
/// `false` if the client's cached copy is still valid (i.e. send 304).
///
/// HTTP-date format: `Thu, 01 Jan 1970 00:00:00 GMT`
pub fn check_if_modified_since(
    req: &Request<hyper::body::Incoming>,
    last_modified_timestamp: u64,
) -> bool {
    let Some(ims_header) = get_header_value(req.headers(), "if-modified-since") else {
        // No header → always serve the resource.
        return true;
    };

    // Parse the HTTP-date with httpdate.  If parsing fails, serve the resource
    // (conservative — avoids accidentally sending stale 304 responses).
    let Ok(ims_time) = httpdate::parse_http_date(&ims_header) else {
        warn!("Failed to parse If-Modified-Since header: '{}'", ims_header);
        return true;
    };

    let ims_secs = ims_time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // The resource has NOT been modified if its timestamp is ≤ the IMS time.
    // Return false (304) in that case; true (serve) otherwise.
    last_modified_timestamp > ims_secs
}

/// Extract basic auth credentials from `Authorization: Basic <base64>`.
pub fn get_basic_auth(req: &Request<hyper::body::Incoming>) -> Option<(String, String)> {
    use base64::prelude::{BASE64_STANDARD, Engine as _};

    let auth = get_header_value(req.headers(), "authorization")?;
    if !auth.starts_with("Basic ") {
        warn!("Invalid or missing Basic auth");
        return None;
    }

    let decoded = BASE64_STANDARD.decode(&auth[6..]).ok()?;
    let credentials = String::from_utf8(decoded).ok()?;
    let mut parts = credentials.splitn(2, ':');
    let username = parts.next()?.to_string();
    let password = parts.next()?.to_string();

    debug!("Basic auth credentials extracted");
    Some((username, password))
}
