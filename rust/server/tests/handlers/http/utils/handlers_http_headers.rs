/// Tests for HTTP header utilities and cookie management
use bytes::Bytes;
use hyper::{Response, header::HeaderMap};
use server::handlers::http::utils::*;
use std::time::Duration;

// ── Generic header helpers ─────────────────────────────────────────────────

#[test]
fn get_header_value_returns_some_when_exists() {
    let mut headers = HeaderMap::new();
    headers.insert("x-custom", "value123".parse().unwrap());

    let result = get_header_value(&headers, "x-custom");
    assert_eq!(result, Some("value123".to_string()));
}

#[test]
fn get_header_value_returns_none_when_missing() {
    let headers = HeaderMap::new();
    let result = get_header_value(&headers, "missing-header");
    assert!(result.is_none());
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
    assert_eq!(
        get_header_value(&headers, "authorization"),
        Some("Bearer token123".to_string())
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

    let result = get_cookie(&headers, "session_id");
    assert_eq!(result, Some("abc123".to_string()));
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
fn get_cookie_missing_cookie_returns_none() {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "session_id=abc123".parse().unwrap());

    let result = get_cookie(&headers, "nonexistent");
    assert!(result.is_none());
}

#[test]
fn get_cookie_no_cookies_header() {
    let headers = HeaderMap::new();
    let result = get_cookie(&headers, "session_id");
    assert!(result.is_none());
}

#[test]
fn get_cookie_with_spaces() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "session_id = abc123 ; user_id = 456".parse().unwrap(),
    );

    assert_eq!(
        get_cookie(&headers, "session_id"),
        Some("abc123".to_string())
    );
    assert_eq!(get_cookie(&headers, "user_id"), Some("456".to_string()));
}

#[test]
fn set_cookie_basic() {
    let result = set_cookie("test", "value", None, None, false, false);
    assert!(result.is_ok());
    let header_value = result.unwrap();
    let cookie_str = header_value.to_str().unwrap();
    assert!(cookie_str.contains("test=value"));
}

#[test]
fn set_cookie_with_max_age() {
    let max_age = Duration::from_secs(3600);
    let result = set_cookie("test", "value", Some(max_age), None, false, false);
    assert!(result.is_ok());
    let cookie = result.unwrap();
    let cookie_str = cookie.to_str().unwrap();
    assert!(cookie_str.contains("Max-Age=3600"));
}

#[test]
fn set_cookie_with_path() {
    let result = set_cookie("test", "value", None, Some("/api"), false, false);
    assert!(result.is_ok());
    let cookie = result.unwrap();
    let cookie_str = cookie.to_str().unwrap();
    assert!(cookie_str.contains("Path=/api"));
}

#[test]
fn set_cookie_http_only() {
    let result = set_cookie("test", "value", None, None, true, false);
    assert!(result.is_ok());
    let cookie = result.unwrap();
    let cookie_str = cookie.to_str().unwrap();
    assert!(cookie_str.contains("HttpOnly"));
}

#[test]
fn set_cookie_secure() {
    let result = set_cookie("test", "value", None, None, false, true);
    assert!(result.is_ok());
    let cookie = result.unwrap();
    let cookie_str = cookie.to_str().unwrap();
    assert!(cookie_str.contains("Secure"));
}

#[test]
fn set_cookie_all_options() {
    let max_age = Duration::from_secs(7200);
    let result = set_cookie("auth", "token123", Some(max_age), Some("/"), true, true);
    assert!(result.is_ok());
    let cookie = result.unwrap();
    let cookie_str = cookie.to_str().unwrap();
    assert!(cookie_str.contains("auth=token123"));
    assert!(cookie_str.contains("Max-Age=7200"));
    assert!(cookie_str.contains("Path=/"));
    assert!(cookie_str.contains("HttpOnly"));
    assert!(cookie_str.contains("Secure"));
    assert!(cookie_str.contains("SameSite=Strict"));
}

#[test]
fn create_session_cookie_http_only() {
    let result = create_session_cookie("session", "abc123", true);
    assert!(result.is_ok());
    let cookie = result.unwrap();
    let cookie_str = cookie.to_str().unwrap();
    assert!(cookie_str.contains("session=abc123"));
    assert!(cookie_str.contains("HttpOnly"));
    assert!(cookie_str.contains("Path=/"));
    assert!(cookie_str.contains("Secure"));
}

#[test]
fn create_session_cookie_http() {
    let result = create_session_cookie("session", "abc123", false);
    assert!(result.is_ok());
    let cookie = result.unwrap();
    let cookie_str = cookie.to_str().unwrap();
    assert!(cookie_str.contains("session=abc123"));
    assert!(cookie_str.contains("HttpOnly"));
    // Should not have Secure when https=false
    assert!(!cookie_str.contains("Secure"));
}

#[test]
fn create_persistent_cookie_https() {
    let max_age = Duration::from_secs(86400);
    let result = create_persistent_cookie("remember", "token", max_age, true);
    assert!(result.is_ok());
    let cookie = result.unwrap();
    let cookie_str = cookie.to_str().unwrap();
    assert!(cookie_str.contains("remember=token"));
    assert!(cookie_str.contains("Max-Age=86400"));
    assert!(cookie_str.contains("Secure"));
    assert!(cookie_str.contains("HttpOnly"));
}

#[test]
fn delete_cookie_sets_max_age_zero() {
    let result = delete_cookie("session");
    assert!(result.is_ok());
    let cookie = result.unwrap();
    let cookie_str = cookie.to_str().unwrap();
    assert!(cookie_str.contains("session="));
    assert!(cookie_str.contains("Max-Age=0"));
}

// ── IP / UA helpers ────────────────────────────────────────────────────────

#[test]
fn get_client_ip_from_x_forwarded_for() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());

    // Note: this requires Request context, test structure may need adjustment
    // This is a placeholder for the actual implementation
}

// ── Bearer token helpers ───────────────────────────────────────────────────

#[test]
fn get_bearer_token_from_authorization_header() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer mytoken123".parse().unwrap());

    // This requires Request context, placeholder for structure
}

// ── Cache helpers ──────────────────────────────────────────────────────────

#[test]
fn add_no_cache_headers_sets_all_headers() {
    let response = Response::builder().status(200).body(Bytes::new()).unwrap();

    let cached_response = add_no_cache_headers(response);
    let headers = cached_response.headers();

    assert_eq!(
        headers.get("cache-control").unwrap().to_str().unwrap(),
        "no-cache, no-store, must-revalidate"
    );
    assert_eq!(headers.get("pragma").unwrap().to_str().unwrap(), "no-cache");
    assert_eq!(headers.get("expires").unwrap().to_str().unwrap(), "0");
    assert_eq!(
        headers
            .get("x-content-type-options")
            .unwrap()
            .to_str()
            .unwrap(),
        "nosniff"
    );
}

#[test]
fn add_cache_headers_with_default_max_age() {
    use bytes::Bytes;
    use hyper::Response;

    let response = Response::builder().status(200).body(Bytes::new()).unwrap();

    let cached_response = add_cache_headers_with_max_age(response, None);
    let cache_control = cached_response
        .headers()
        .get("cache-control")
        .unwrap()
        .to_str()
        .unwrap();

    assert!(cache_control.contains("public, max-age=31536000"));
}

#[test]
fn add_cache_headers_with_custom_max_age() {
    use bytes::Bytes;
    use hyper::Response;

    let response = Response::builder().status(200).body(Bytes::new()).unwrap();

    let cached_response = add_cache_headers_with_max_age(response, Some(3600));
    let cache_control = cached_response
        .headers()
        .get("cache-control")
        .unwrap()
        .to_str()
        .unwrap();

    assert!(cache_control.contains("public, max-age=3600"));
}
