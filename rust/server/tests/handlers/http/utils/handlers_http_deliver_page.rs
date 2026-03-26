/// Tests for static page delivery and caching
use http_body_util::BodyExt;
use hyper::http::{HeaderValue, StatusCode};
use server::handlers::http::utils::deliver_page::*;
use shared::types::cache::CacheStrategy;
use std::io::Write;
use tempfile::NamedTempFile;

// ── get_mime_type ─────────────────────────────────────────────────────────

#[test]
fn mime_html() {
    assert_eq!(
        get_mime_type(std::path::Path::new("page.html")),
        "text/html; charset=utf-8"
    );
}

#[test]
fn mime_htm_alias() {
    assert_eq!(
        get_mime_type(std::path::Path::new("page.htm")),
        "text/html; charset=utf-8"
    );
}

#[test]
fn mime_css() {
    assert_eq!(
        get_mime_type(std::path::Path::new("style.css")),
        "text/css; charset=utf-8"
    );
}

#[test]
fn mime_js() {
    assert_eq!(
        get_mime_type(std::path::Path::new("app.js")),
        "application/javascript; charset=utf-8"
    );
}

#[test]
fn mime_mjs() {
    assert_eq!(
        get_mime_type(std::path::Path::new("module.mjs")),
        "application/javascript; charset=utf-8"
    );
}

#[test]
fn mime_json() {
    assert_eq!(
        get_mime_type(std::path::Path::new("data.json")),
        "application/json"
    );
}

#[test]
fn mime_xml() {
    assert_eq!(
        get_mime_type(std::path::Path::new("config.xml")),
        "application/xml"
    );
}

#[test]
fn mime_png() {
    assert_eq!(get_mime_type(std::path::Path::new("img.png")), "image/png");
}

#[test]
fn mime_jpg() {
    assert_eq!(
        get_mime_type(std::path::Path::new("photo.jpg")),
        "image/jpeg"
    );
}

#[test]
fn mime_jpeg() {
    assert_eq!(
        get_mime_type(std::path::Path::new("photo.jpeg")),
        "image/jpeg"
    );
}

#[test]
fn mime_gif() {
    assert_eq!(get_mime_type(std::path::Path::new("anim.gif")), "image/gif");
}

#[test]
fn mime_svg() {
    assert_eq!(
        get_mime_type(std::path::Path::new("icon.svg")),
        "image/svg+xml"
    );
}

#[test]
fn mime_ico() {
    assert_eq!(
        get_mime_type(std::path::Path::new("favicon.ico")),
        "image/x-icon"
    );
}

#[test]
fn mime_webp() {
    assert_eq!(
        get_mime_type(std::path::Path::new("image.webp")),
        "image/webp"
    );
}

#[test]
fn mime_bmp() {
    assert_eq!(
        get_mime_type(std::path::Path::new("bitmap.bmp")),
        "image/bmp"
    );
}

#[test]
fn mime_avif() {
    assert_eq!(
        get_mime_type(std::path::Path::new("image.avif")),
        "image/avif"
    );
}

#[test]
fn mime_woff() {
    assert_eq!(
        get_mime_type(std::path::Path::new("font.woff")),
        "font/woff"
    );
}

#[test]
fn mime_woff2() {
    assert_eq!(
        get_mime_type(std::path::Path::new("font.woff2")),
        "font/woff2"
    );
}

#[test]
fn mime_ttf() {
    assert_eq!(get_mime_type(std::path::Path::new("font.ttf")), "font/ttf");
}

#[test]
fn mime_otf() {
    assert_eq!(get_mime_type(std::path::Path::new("font.otf")), "font/otf");
}

#[test]
fn mime_eot() {
    assert_eq!(
        get_mime_type(std::path::Path::new("font.eot")),
        "application/vnd.ms-fontobject"
    );
}

#[test]
fn mime_mp3() {
    assert_eq!(
        get_mime_type(std::path::Path::new("audio.mp3")),
        "audio/mpeg"
    );
}

#[test]
fn mime_mp4() {
    assert_eq!(
        get_mime_type(std::path::Path::new("video.mp4")),
        "video/mp4"
    );
}

#[test]
fn mime_webm() {
    assert_eq!(
        get_mime_type(std::path::Path::new("video.webm")),
        "video/webm"
    );
}

#[test]
fn mime_ogg() {
    assert_eq!(
        get_mime_type(std::path::Path::new("audio.ogg")),
        "audio/ogg"
    );
}

#[test]
fn mime_wav() {
    assert_eq!(
        get_mime_type(std::path::Path::new("sound.wav")),
        "audio/wav"
    );
}

#[test]
fn mime_pdf() {
    assert_eq!(
        get_mime_type(std::path::Path::new("doc.pdf")),
        "application/pdf"
    );
}

#[test]
fn mime_txt() {
    assert_eq!(
        get_mime_type(std::path::Path::new("readme.txt")),
        "text/plain; charset=utf-8"
    );
}

#[test]
fn mime_md() {
    assert_eq!(
        get_mime_type(std::path::Path::new("README.md")),
        "text/markdown; charset=utf-8"
    );
}

#[test]
fn mime_zip() {
    assert_eq!(
        get_mime_type(std::path::Path::new("archive.zip")),
        "application/zip"
    );
}

#[test]
fn mime_gz() {
    assert_eq!(
        get_mime_type(std::path::Path::new("archive.gz")),
        "application/gzip"
    );
}

#[test]
fn mime_tar() {
    assert_eq!(
        get_mime_type(std::path::Path::new("archive.tar")),
        "application/x-tar"
    );
}

#[test]
fn mime_unknown_extension_is_octet_stream() {
    assert_eq!(
        get_mime_type(std::path::Path::new("file.xyz")),
        "application/octet-stream"
    );
}

#[test]
fn mime_no_extension_is_octet_stream() {
    assert_eq!(
        get_mime_type(std::path::Path::new("Makefile")),
        "application/octet-stream"
    );
}

// ── full() helper ─────────────────────────────────────────────────────────

#[tokio::test]
async fn full_helper_wraps_bytes() {
    use bytes::Bytes;
    use http_body_util::BodyExt;
    let body = full(Bytes::from("hello"));
    let collected = body.collect().await.unwrap().to_bytes();
    assert_eq!(&collected[..], b"hello");
}

#[tokio::test]
async fn empty_helper_returns_empty_body() {
    use http_body_util::BodyExt;
    let body = empty();
    let collected = body.collect().await.unwrap().to_bytes();
    assert!(collected.is_empty());
}

#[tokio::test]
async fn full_helper_with_string() {
    use http_body_util::BodyExt;
    let body = full("world");
    let collected = body.collect().await.unwrap().to_bytes();
    assert_eq!(&collected[..], b"world");
}

#[tokio::test]
async fn full_helper_with_vec() {
    use http_body_util::BodyExt;
    let body = full(vec![1, 2, 3, 4, 5]);
    let collected = body.collect().await.unwrap().to_bytes();
    assert_eq!(&collected[..], &[1, 2, 3, 4, 5]);
}

// ── deliver_page_with_status — happy path ─────────────────────────────────

#[tokio::test]
async fn deliver_page_reads_file_and_sets_content_type() {
    let mut f = NamedTempFile::with_suffix(".html").unwrap();
    write!(f, "<html><body>test</body></html>").unwrap();

    let res = deliver_page_with_status(f.path(), StatusCode::OK, CacheStrategy::LongTerm).unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "text/html; charset=utf-8");
    let body = res.collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<html><body>test</body></html>");
}

#[tokio::test]
async fn deliver_page_with_json_status() {
    let mut f = NamedTempFile::with_suffix(".json").unwrap();
    write!(f, r#"{{"test": true}}"#).unwrap();

    let res =
        deliver_page_with_status(f.path(), StatusCode::CREATED, CacheStrategy::ShortTerm).unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);
    assert_eq!(res.headers()["content-type"], "application/json");
}

#[test]
fn deliver_page_missing_file_returns_error() {
    let result = deliver_page_with_status(
        "/nonexistent/path/file.html",
        StatusCode::OK,
        CacheStrategy::NoCache,
    );
    assert!(result.is_err());
}

#[test]
fn deliver_html_page_delegates_to_deliver_page_with_status() {
    // This would require mocking or integration testing to fully verify
    // For now, we verify it doesn't panic with a nonexistent path
    let result = deliver_html_page("/nonexistent/file.html");
    assert!(result.is_err());
}

// ── deliver_redirect ──────────────────────────────────────────────────────

#[test]
fn deliver_redirect_302_and_location() {
    let res = deliver_redirect("/new-location").unwrap();
    assert_eq!(res.status(), StatusCode::FOUND);
    assert_eq!(res.headers()["location"], "/new-location");
}

#[test]
fn deliver_redirect_with_relative_path() {
    let res = deliver_redirect("../parent").unwrap();
    assert_eq!(res.status(), StatusCode::FOUND);
    assert_eq!(res.headers()["location"], "../parent");
}

#[test]
fn deliver_redirect_with_absolute_url() {
    let res = deliver_redirect("https://example.com/path").unwrap();
    assert_eq!(res.status(), StatusCode::FOUND);
    assert_eq!(res.headers()["location"], "https://example.com/path");
}

#[tokio::test]
async fn deliver_redirect_empty_body() {
    let res = deliver_redirect("/new-location").unwrap();
    let body = res.collect().await.unwrap().to_bytes();
    assert!(body.is_empty());
}

#[test]
fn deliver_redirect_with_cookies() {
    let cookie = HeaderValue::from_static("session_id=abc123; Path=/");
    let res = deliver_redirect_with_cookie("/new-location", cookie).unwrap();
    assert_eq!(res.status(), StatusCode::FOUND);
    assert_eq!(res.headers()["location"], "/new-location");
    assert!(res.headers().contains_key("set-cookie"));
}

// ── deliver_text ──────────────────────────────────────────────────────────

#[tokio::test]
async fn deliver_text_body_and_content_type() {
    use http_body_util::BodyExt;
    let res = deliver_text(b"hello world".to_vec()).unwrap();
    assert_eq!(res.headers()["content-type"], "text/plain; charset=utf-8");
    let body = res.collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"hello world");
}

#[tokio::test]
async fn deliver_text_with_string() {
    use http_body_util::BodyExt;
    let res = deliver_text("plain text message").unwrap();
    assert_eq!(res.headers()["content-type"], "text/plain; charset=utf-8");
    let body = res.collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"plain text message");
}

#[tokio::test]
async fn deliver_text_empty_string() {
    use http_body_util::BodyExt;
    let res = deliver_text("").unwrap();
    let body = res.collect().await.unwrap().to_bytes();
    assert!(body.is_empty());
}

#[tokio::test]
async fn deliver_text_with_special_characters() {
    use http_body_util::BodyExt;
    let text = "Special chars: !@#$%^&*()_+-=[]{}|;':\",./<>?";
    let res = deliver_text(text).unwrap();
    let body = res.collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], text.as_bytes());
}

#[tokio::test]
async fn deliver_text_returns_ok_status() {
    let res = deliver_text("test").unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

// ── expand_tilde ──────────────────────────────────────────────────────────

#[test]
fn expand_tilde_without_home_env_returns_original() {
    // Without HOME set this returns the path unchanged; with HOME it expands.
    // Just check the function doesn't panic with a non-tilde path.
    let p = expand_tilde("/absolute/path/file.html").unwrap();
    assert_eq!(p.to_str().unwrap(), "/absolute/path/file.html");
}

#[test]
fn expand_tilde_non_tilde_path_unchanged() {
    let p = expand_tilde("/home/user/documents").unwrap();
    assert_eq!(p.to_str().unwrap(), "/home/user/documents");
}

#[test]
fn expand_tilde_relative_path_unchanged() {
    let p = expand_tilde("./relative/path").unwrap();
    assert_eq!(p.to_str().unwrap(), "./relative/path");
}

// ── deliver_page_with_etag ────────────────────────────────────────────────

#[tokio::test]
async fn deliver_page_with_etag_includes_etag_header() {
    let mut f = NamedTempFile::with_suffix(".html").unwrap();
    write!(f, "<html>test</html>").unwrap();

    let res = deliver_page_with_etag(
        f.path(),
        StatusCode::OK,
        CacheStrategy::LongTerm,
        "\"abc123\"",
    )
    .unwrap();

    assert!(res.headers().contains_key("etag"));
    let body = res.collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<html>test</html>");
}
