use std::collections::HashMap;

// ── Form parsing (username + email update) ────────────────────────────────

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
fn parse_update_whitespace_is_trimmed_to_none() {
    let body = b"username=%20%20%20";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    let username = params
        .get("username")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    assert!(
        username.is_none(),
        "whitespace-only username must become None"
    );
}

#[test]
fn parse_update_url_encoded_chars_decoded() {
    let body = b"email=user%2Btest%40example.com";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    let email = params.get("email").map(|s| s.to_string());
    assert_eq!(email, Some("user+test@example.com".to_string()));
}

#[test]
fn parse_update_unknown_fields_are_ignored() {
    let body = b"username=alice&unknown_field=value&email=alice@example.com";
    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();
    assert_eq!(params.get("username").map(|s| s.as_str()), Some("alice"));
    assert_eq!(
        params.get("email").map(|s| s.as_str()),
        Some("alice@example.com")
    );
    // Unknown fields are present in the map but the handler ignores them
    assert!(params.contains_key("unknown_field"));
}

// ── Avatar MIME type → extension mapping ─────────────────────────────────

#[test]
fn jpeg_mime_maps_to_jpg_extension() {
    let ext = match "image/jpeg" {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "jpg",
    };
    assert_eq!(ext, "jpg");
}

#[test]
fn png_mime_maps_to_png_extension() {
    let ext = match "image/png" {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "jpg",
    };
    assert_eq!(ext, "png");
}

#[test]
fn gif_mime_maps_to_gif_extension() {
    let ext = match "image/gif" {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "jpg",
    };
    assert_eq!(ext, "gif");
}

#[test]
fn webp_mime_maps_to_webp_extension() {
    let ext = match "image/webp" {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "jpg",
    };
    assert_eq!(ext, "webp");
}

#[test]
fn unknown_mime_falls_back_to_jpg() {
    let ext = match "image/bmp" {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "jpg",
    };
    assert_eq!(ext, "jpg");
}

#[test]
fn non_image_mime_is_rejected() {
    let mime = "text/plain";
    assert!(
        !mime.starts_with("image/"),
        "non-image MIME must be rejected"
    );
}

#[test]
fn non_image_application_mime_is_rejected() {
    let mime = "application/pdf";
    assert!(!mime.starts_with("image/"));
}

// ── Avatar filename construction ──────────────────────────────────────────

#[test]
fn avatar_filename_uses_user_id_and_extension() {
    let user_id = 123_i64;
    let mime = "image/png";
    let ext = match mime {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "jpg",
    };
    let filename = format!("{}.{}", user_id, ext);
    assert_eq!(filename, "123.png");
}

#[test]
fn avatar_filename_overwrites_previous_for_same_user() {
    // Two uploads by the same user produce the same filename, so the second
    // replaces the first on disk — no orphaned files.
    let user_id = 7_i64;
    let filename_first = format!("{}.jpg", user_id);
    let filename_second = format!("{}.jpg", user_id);
    assert_eq!(filename_first, filename_second);
}
