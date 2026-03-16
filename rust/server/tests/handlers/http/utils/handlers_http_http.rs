/// Tests for HTTP protocol detection
use http_body_util::Empty;
use hyper::Request;
use hyper::body::Bytes;
use server::handlers::http::utils::*;

// Helper to create a mock request with headers
fn create_request_with_headers(headers: Vec<(&str, &str)>) -> Request<Empty<Bytes>> {
    let mut builder = Request::builder();

    for (name, value) in headers {
        builder = builder.header(name, value);
    }

    // Empty::<Bytes>::new() implements the Body trait
    builder.body(Empty::<Bytes>::new()).unwrap()
} // ── is_https detection ─────────────────────────────────────────────────────

#[test]
fn is_https_with_x_forwarded_proto_https() {
    let req = create_request_with_headers(vec![("x-forwarded-proto", "https")]);
    assert!(is_https(&req));
}

#[test]
fn is_https_with_x_forwarded_proto_http() {
    let req = create_request_with_headers(vec![("x-forwarded-proto", "http")]);
    assert!(!is_https(&req));
}

#[test]
fn is_https_with_x_forwarded_proto_case_insensitive() {
    let req = create_request_with_headers(vec![("x-forwarded-proto", "HTTPS")]);
    assert!(is_https(&req));

    let req2 = create_request_with_headers(vec![("x-forwarded-proto", "HtTpS")]);
    assert!(is_https(&req2));
}

#[test]
fn is_https_with_x_forwarded_ssl_on() {
    let req = create_request_with_headers(vec![("x-forwarded-ssl", "on")]);
    assert!(is_https(&req));
}

#[test]
fn is_https_with_x_forwarded_ssl_off() {
    let req = create_request_with_headers(vec![("x-forwarded-ssl", "off")]);
    assert!(!is_https(&req));
}

#[test]
fn is_https_with_x_forwarded_ssl_case_insensitive() {
    let req = create_request_with_headers(vec![("x-forwarded-ssl", "ON")]);
    assert!(is_https(&req));

    let req2 = create_request_with_headers(vec![("x-forwarded-ssl", "oN")]);
    assert!(is_https(&req2));
}

#[test]
fn is_https_no_headers_returns_false() {
    let req: Request<Empty<Bytes>> = Request::builder().body(Empty::new()).unwrap();
    assert!(!is_https(&req));
}

#[test]
fn is_https_x_forwarded_proto_takes_precedence() {
    let req = create_request_with_headers(vec![
        ("x-forwarded-proto", "https"),
        ("x-forwarded-ssl", "off"),
    ]);
    // x-forwarded-proto should be checked first
    assert!(is_https(&req));
}

#[test]
fn is_https_falls_back_to_x_forwarded_ssl() {
    let req = create_request_with_headers(vec![("x-forwarded-ssl", "on")]);
    assert!(is_https(&req));
}

#[test]
fn is_https_invalid_x_forwarded_proto_value() {
    let req = create_request_with_headers(vec![("x-forwarded-proto", "ftp")]);
    assert!(!is_https(&req));
}

#[test]
fn is_https_invalid_x_forwarded_ssl_value() {
    let req = create_request_with_headers(vec![("x-forwarded-ssl", "yes")]);
    assert!(!is_https(&req));
}

#[test]
fn is_https_multiple_headers_none_https() {
    let req = create_request_with_headers(vec![
        ("content-type", "application/json"),
        ("user-agent", "test"),
        ("accept", "application/json"),
    ]);
    assert!(!is_https(&req));
}

#[test]
fn is_https_empty_x_forwarded_proto_value() {
    let req = create_request_with_headers(vec![("x-forwarded-proto", "")]);
    assert!(!is_https(&req));
}
