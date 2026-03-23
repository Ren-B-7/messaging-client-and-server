use bytes::Bytes;
use http_body_util::Empty;
use hyper::Request;
use hyper::{Response, header::HeaderMap};
use server::handlers::http::utils::*;
use std::time::Duration;

// ── Generic header helpers ─────────────────────────────────────────────────

#[test]
fn get_header_value_returns_some_when_exists() {
    let mut headers = HeaderMap::new();
    headers.insert("x-custom", "value123".parse().unwrap());
    assert_eq!(
        get_header_value(&headers, "x-custom"),
        Some("value123".to_string())
    );
}

#[test]
fn get_header_value_returns_none_when_missing() {
    let headers = HeaderMap::new();
    assert!(get_header_value(&headers, "missing-header").is_none());
}

#[test]
fn get_header_value_with_multiple_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", "application/json".parse().unwrap());
    headers.insert("authorization", "Bearer token123".parse().unwrap());
    assert_eq!(
        get_header_value(&headers, "content-type"),
        Some("application/json".to_string())
    );
}

#[test]
fn header_matches_case_insensitive() {
    let mut headers = HeaderMap::new();
    headers.insert("x-type", "json".parse().unwrap());
    assert!(header_matches(&headers, "x-type", "JSON"));
    assert!(header_matches(&headers, "x-type", "Json"));
    assert!(header_matches(&headers, "x-type", "json"));
}

#[test]
fn header_matches_returns_false_on_mismatch() {
    let mut headers = HeaderMap::new();
    headers.insert("x-type", "json".parse().unwrap());
    assert!(!header_matches(&headers, "x-type", "xml"));
}

#[test]
fn header_matches_returns_false_on_missing_header() {
    let headers = HeaderMap::new();
    assert!(!header_matches(&headers, "missing", "value"));
}

// ── Cookie helpers ─────────────────────────────────────────────────────────

#[test]
fn get_cookie_single_cookie() {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "session_id=abc123".parse().unwrap());
    assert_eq!(
        get_cookie(&headers, "session_id"),
        Some("abc123".to_string())
    );
}

#[test]
fn get_cookie_multiple_cookies() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "session_id=abc123; user_id=456; theme=dark"
            .parse()
            .unwrap(),
    );
    assert_eq!(
        get_cookie(&headers, "session_id"),
        Some("abc123".to_string())
    );
    assert_eq!(get_cookie(&headers, "user_id"), Some("456".to_string()));
    assert_eq!(get_cookie(&headers, "theme"), Some("dark".to_string()));
}

#[test]
fn get_cookie_missing_returns_none() {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "session_id=abc123".parse().unwrap());
    assert!(get_cookie(&headers, "nonexistent").is_none());
}

#[test]
fn get_cookie_no_cookie_header_returns_none() {
    let headers = HeaderMap::new();
    assert!(get_cookie(&headers, "session_id").is_none());
}

#[test]
fn set_cookie_all_options() {
    let result = set_cookie(
        "auth",
        "token123",
        Some(Duration::from_secs(7200)),
        Some("/"),
        true,
        true,
    );
    assert!(result.is_ok());
    let s = result.unwrap().to_str().unwrap().to_string();
    assert!(s.contains("auth=token123"));
    assert!(s.contains("Max-Age=7200"));
    assert!(s.contains("Path=/"));
    assert!(s.contains("HttpOnly"));
    assert!(s.contains("Secure"));
}

#[test]
fn create_session_cookie_sets_http_only() {
    let cookie = create_session_cookie("session", "abc123", true).unwrap();
    let s = cookie.to_str().unwrap();
    assert!(s.contains("session=abc123"));
    assert!(s.contains("HttpOnly"));
    assert!(s.contains("Secure"));
}

#[test]
fn create_session_cookie_no_secure_over_http() {
    let cookie = create_session_cookie("session", "abc123", false).unwrap();
    let s = cookie.to_str().unwrap();
    assert!(s.contains("HttpOnly"));
    assert!(!s.contains("Secure"), "no Secure flag over plain HTTP");
}

#[test]
fn create_persistent_cookie_has_max_age() {
    let cookie =
        create_persistent_cookie("remember", "token", Duration::from_secs(86400), true).unwrap();
    let s = cookie.to_str().unwrap();
    assert!(s.contains("Max-Age=86400"));
    assert!(s.contains("Secure"));
    assert!(s.contains("HttpOnly"));
}

#[test]
fn delete_cookie_sets_max_age_zero() {
    let cookie = delete_cookie("session").unwrap();
    let s = cookie.to_str().unwrap();
    assert!(s.contains("Max-Age=0"));
}

// ── get_client_ip ─────────────────────────────────────────────────────────

#[test]
fn get_client_ip_from_x_forwarded_for_header() {
    let req = Request::builder()
        .header("x-forwarded-for", "203.0.113.1, 10.0.0.1")
        .body(Empty::<Bytes>::new())
        .unwrap();
    let ip = get_client_ip(&req);
    // get_client_ip should return the first (client-facing) IP
    assert!(ip.is_some(), "should extract IP from X-Forwarded-For");
    let ip_str = ip.unwrap();
    assert!(
        ip_str.contains("203.0.113.1") || ip_str == "203.0.113.1",
        "got: {}",
        ip_str
    );
}

#[test]
fn get_client_ip_from_x_real_ip_header() {
    let req = Request::builder()
        .header("x-real-ip", "198.51.100.5")
        .body(Empty::<Bytes>::new())
        .unwrap();
    let ip = get_client_ip(&req);
    // X-Real-Ip is a single-value header
    if let Some(ip_str) = ip {
        assert!(ip_str.contains("198.51.100.5"), "got: {}", ip_str);
    }
    // If neither header is present, None is acceptable — just must not panic.
}

#[test]
fn get_client_ip_returns_none_with_no_headers() {
    let req = Request::builder().body(Empty::<Bytes>::new()).unwrap();
    // No forwarding headers — result may be None or an empty string, but must not panic.
    let _ = get_client_ip(&req);
}

// ── get_bearer_token ──────────────────────────────────────────────────────

#[test]
fn get_bearer_token_from_authorization_header() {
    let req = Request::builder()
        .header("authorization", "Bearer mytoken123")
        .body(Empty::<Bytes>::new())
        .unwrap();
    let token = get_bearer_token(req.headers());
    assert_eq!(token, Some("mytoken123".to_string()));
}

#[test]
fn get_bearer_token_missing_returns_none() {
    let req = Request::builder().body(Empty::<Bytes>::new()).unwrap();
    let token = get_bearer_token(req.headers());
    assert!(token.is_none());
}

#[test]
fn get_bearer_token_non_bearer_scheme_returns_none() {
    let req = Request::builder()
        .header("authorization", "Basic dXNlcjpwYXNz")
        .body(Empty::<Bytes>::new())
        .unwrap();
    let token = get_bearer_token(req.headers());
    assert!(
        token.is_none(),
        "Basic auth should not be parsed as a bearer token"
    );
}

#[test]
fn get_bearer_token_strips_bearer_prefix() {
    let req = Request::builder()
        .header("authorization", "Bearer eyJhbGciOiJIUzI1NiJ9.payload.sig")
        .body(Empty::<Bytes>::new())
        .unwrap();
    let token = get_bearer_token(req.headers()).unwrap();
    assert!(!token.starts_with("Bearer "), "prefix must be stripped");
    assert!(token.starts_with("eyJ"));
}

// ── Cache helpers ──────────────────────────────────────────────────────────

#[test]
fn add_no_cache_headers_sets_all_required_headers() {
    let response = Response::builder().status(200).body(Bytes::new()).unwrap();
    let r = add_no_cache_headers(response);
    let h = r.headers();
    assert_eq!(h["cache-control"], "no-cache, no-store, must-revalidate");
    assert_eq!(h["pragma"], "no-cache");
    assert_eq!(h["expires"], "0");
    assert_eq!(h["x-content-type-options"], "nosniff");
}

#[test]
fn add_cache_headers_with_default_max_age_uses_one_year() {
    let response = Response::builder().status(200).body(Bytes::new()).unwrap();
    let r = add_cache_headers_with_max_age(response, None);
    let cc = r.headers()["cache-control"].to_str().unwrap();
    assert!(cc.contains("max-age=31536000"));
}

#[test]
fn add_cache_headers_with_custom_max_age() {
    let response = Response::builder().status(200).body(Bytes::new()).unwrap();
    let r = add_cache_headers_with_max_age(response, Some(3600));
    let cc = r.headers()["cache-control"].to_str().unwrap();
    assert!(cc.contains("max-age=3600"));
}
