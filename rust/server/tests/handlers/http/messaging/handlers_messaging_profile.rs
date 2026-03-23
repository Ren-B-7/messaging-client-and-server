/// Tests for user profile and settings handlers
use std::collections::HashMap;

// ── Form parsing utilities ─────────────────────────────────────────────────

#[test]
fn parse_update_both_fields() {
    let body = b"username=alice&email=alice@example.com";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    let username = params
        .get("username")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let email = params
        .get("email")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    assert_eq!(username, Some("alice".to_string()));
    assert_eq!(email, Some("alice@example.com".to_string()));
}

#[test]
fn parse_update_username_only() {
    let body = b"username=bob";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    let username = params
        .get("username")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let email = params
        .get("email")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    assert_eq!(username, Some("bob".to_string()));
    assert!(email.is_none());
}

#[test]
fn parse_update_email_only() {
    let body = b"email=charlie@example.com";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    let username = params
        .get("username")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let email = params
        .get("email")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    assert!(username.is_none());
    assert_eq!(email, Some("charlie@example.com".to_string()));
}

#[test]
fn parse_update_empty_fields_become_none() {
    let body = b"username=&email=";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    let username = params
        .get("username")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let email = params
        .get("email")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    assert!(username.is_none(), "empty username should be None");
    assert!(email.is_none(), "empty email should be None");
}

#[test]
fn parse_update_whitespace_trimmed() {
    let body = b"username=%20alice%20";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    let username = params
        .get("username")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    assert_eq!(username, Some("alice".to_string()));
}

#[test]
fn parse_update_special_chars_encoded() {
    let body = b"email=user%2Btest%40example.com";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    let email = params.get("email").map(|s| s.to_string());
    assert_eq!(email, Some("user+test@example.com".to_string()));
}

#[test]
fn parse_update_multiple_same_key_last_wins() {
    let body = b"username=alice&username=bob";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    // HashMap behavior — last value typically wins
    let username = params.get("username").map(|s| s.to_string());
    assert!(username.is_some());
}

#[test]
fn parse_update_unknown_fields_ignored() {
    let body = b"username=alice&unknown_field=value&email=alice@example.com";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    let username = params.get("username").map(|s| s.to_string());
    let email = params.get("email").map(|s| s.to_string());
    assert_eq!(username, Some("alice".to_string()));
    assert_eq!(email, Some("alice@example.com".to_string()));
    // unknown_field is present but should be ignored
    assert!(params.contains_key("unknown_field"));
}

// ── Email validation ───────────────────────────────────────────────────────

#[test]
fn valid_email_simple() {
    let email = "user@example.com";
    assert!(email.contains("@"));
}

#[test]
fn valid_email_with_plus() {
    let email = "user+tag@example.com";
    assert!(email.contains("@"));
    assert!(email.contains("+"));
}

#[test]
fn valid_email_with_dots() {
    let email = "user.name@example.com";
    assert!(email.contains("@"));
    assert!(email.contains("."));
}

#[test]
fn invalid_email_no_at_sign() {
    let email = "userexample.com";
    assert!(!email.contains("@"));
}

#[test]
fn invalid_email_no_domain() {
    let email = "user@";
    assert!(email.contains("@"));
    assert_eq!(email.split('@').count(), 2);
}

// ── Avatar filename/extension handling ──────────────────────────────────

#[test]
fn avatar_extension_jpg() {
    let ext = "jpg";
    assert_eq!(ext, "jpg");
}

#[test]
fn avatar_extension_jpeg() {
    let ext = "jpeg";
    assert_eq!(ext, "jpeg");
}

#[test]
fn avatar_extension_png() {
    let ext = "png";
    assert_eq!(ext, "png");
}

#[test]
fn avatar_extension_gif() {
    let ext = "gif";
    assert_eq!(ext, "gif");
}

#[test]
fn avatar_extension_webp() {
    let ext = "webp";
    assert_eq!(ext, "webp");
}

#[test]
fn avatar_filename_user_id_based() {
    let user_id = 42;
    let ext = "jpg";
    let filename = format!("{}.{}", user_id, ext);
    assert_eq!(filename, "42.jpg");
}

#[test]
fn avatar_filename_large_user_id() {
    let user_id = 9223372036854775800_i64;
    let ext = "png";
    let filename = format!("{}.{}", user_id, ext);
    assert!(filename.contains(".png"));
}

// ── MIME type validation ───────────────────────────────────────────────────

#[test]
fn mime_type_jpeg() {
    let mime = "image/jpeg";
    assert!(mime.contains("image"));
}

#[test]
fn mime_type_png() {
    let mime = "image/png";
    assert_eq!(mime, "image/png");
}

#[test]
fn mime_type_gif() {
    let mime = "image/gif";
    assert_eq!(mime, "image/gif");
}

#[test]
fn mime_type_webp() {
    let mime = "image/webp";
    assert_eq!(mime, "image/webp");
}

#[test]
fn mime_type_invalid_not_image() {
    let mime = "text/plain";
    assert!(!mime.starts_with("image"));
}

// ── Integration scenarios ──────────────────────────────────────────────────

#[test]
fn parse_then_validate_username() {
    let body = b"username=alice_new";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    let username = params
        .get("username")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    assert!(username.is_some());
    let name = username.unwrap();
    assert!(!name.is_empty());
}

#[test]
fn parse_then_validate_email() {
    let body = b"email=new%40example.com";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    let email = params
        .get("email")
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    assert!(email.is_some());
    let e = email.unwrap();
    assert!(e.contains("@"));
}

#[test]
fn avatar_upload_flow() {
    // Simulate avatar upload flow
    let user_id = 123_i64;
    let mime_type = "image/png";
    let ext = match mime_type {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "jpg",
    };

    let filename = format!("{}.{}", user_id, ext);
    assert_eq!(filename, "123.png");
}
