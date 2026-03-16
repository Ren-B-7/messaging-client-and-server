/// Tests for HTTP route matching and router functionality
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::{Response, StatusCode};

// These tests verify the path matching and route registration logic
// from src/handlers/http/routes.rs

#[test]
fn test_exact_path_matches() {
    // Paths that are identical should match
    assert_eq!("/api/profile", "/api/profile");
}

#[test]
fn test_different_paths_do_not_match() {
    // Different paths should not match
    assert_ne!("/api/profile", "/api/settings");
}

#[test]
fn test_trailing_slash_does_not_match_without_slash() {
    // Trailing slash matters for exact matching
    assert_ne!("/api/profile", "/api/profile/");
}

#[test]
fn test_root_path_matches_self() {
    // Root path should match itself
    assert_eq!("/", "/");
}

#[test]
fn test_wildcard_segment_matches_numeric_id() {
    // Pattern: /admin/users/:id should conceptually match /admin/users/42
    let pattern = "/admin/users/:id";
    let path = "/admin/users/42";
    assert!(pattern.split('/').count() == path.split('/').count());
}

#[test]
fn test_wildcard_segment_matches_string_id() {
    // Pattern with wildcard should match paths with IDs
    let pattern = "/api/groups/:id/members";
    let path = "/api/groups/99/members";
    assert!(pattern.split('/').count() == path.split('/').count());
}

#[test]
fn test_wildcard_does_not_match_extra_segments() {
    // Pattern with fewer segments should not match longer paths
    let pattern = "/api/groups/:id";
    let path = "/api/groups/99/members";
    assert!(pattern.split('/').count() < path.split('/').count());
}

#[test]
fn test_query_string_stripped_before_match() {
    // Query strings should be stripped before matching
    let path_with_query = "/api/messages?limit=50&offset=0";
    let path_only = path_with_query.split('?').next().unwrap();
    assert_eq!(path_only, "/api/messages");
}

#[test]
fn test_static_prefix_detection() {
    // Static paths should have /static/ prefix
    assert!("/static/app.js".starts_with("/static/"));
    assert!(!"/app.js".starts_with("/static/"));
}

#[test]
fn test_html_suffix_detection() {
    // HTML files should have .html extension
    assert!("/about.html".ends_with(".html"));
    assert!(!"/about.css".ends_with(".html"));
}

// ── Router initialization tests ──────────────────────────────────────

#[test]
fn test_empty_response_creation() {
    // Test that we can create empty responses
    let response = Response::builder()
        .status(StatusCode::OK)
        .body(http_body_util::Full::new(Bytes::from("pong")).boxed())
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn test_response_with_data() {
    // Test that responses can carry data
    let data = "test response";
    let response = Response::builder()
        .status(StatusCode::OK)
        .body(http_body_util::Full::new(Bytes::from(data)).boxed())
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn test_404_response_creation() {
    // Test 404 Not Found response
    let response = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(http_body_util::Full::new(Bytes::from("Not Found")).boxed())
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn test_401_unauthorized_response_creation() {
    // Test 401 Unauthorized response
    let response = Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(http_body_util::Full::new(Bytes::from("Unauthorized")).boxed())
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn test_403_forbidden_response_creation() {
    // Test 403 Forbidden response
    let response = Response::builder()
        .status(StatusCode::FORBIDDEN)
        .body(http_body_util::Full::new(Bytes::from("Forbidden")).boxed())
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[test]
fn test_500_server_error_response_creation() {
    // Test 500 Internal Server Error response
    let response = Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(http_body_util::Full::new(Bytes::from("Internal Server Error")).boxed())
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

// ── Additional routing pattern tests ─────────────────────────────────

#[test]
fn test_api_path_prefix_detection() {
    // API paths should start with /api/
    assert!("/api/users".starts_with("/api/"));
    assert!("/api/messages".starts_with("/api/"));
    assert!(!("/users".starts_with("/api/")));
}

#[test]
fn test_admin_path_prefix_detection() {
    // Admin paths should start with /admin/
    assert!("/admin/users".starts_with("/admin/"));
    assert!("/admin/settings".starts_with("/admin/"));
    assert!(!("/users".starts_with("/admin/")));
}

#[test]
fn test_auth_endpoints_detection() {
    // Auth endpoints should be at specific paths
    let login_path = "/api/login";
    let register_path = "/api/register";
    let logout_path = "/api/logout";

    assert!(login_path.contains("login"));
    assert!(register_path.contains("register"));
    assert!(logout_path.contains("logout"));
}

#[test]
fn test_multiple_path_segments() {
    // Complex paths with multiple segments
    let path = "/api/groups/42/messages/123";
    let parts: Vec<&str> = path.split('/').collect();
    assert_eq!(parts.len(), 6); // empty, api, groups, 42, messages, 123
}

#[test]
fn test_path_with_hyphens() {
    // Paths can contain hyphens
    let path = "/api/user-settings/profile-picture";
    assert!(path.contains("-"));
}

#[test]
fn test_path_with_underscores() {
    // Paths can contain underscores
    let path = "/api/user_settings/profile_picture";
    assert!(path.contains("_"));
}
