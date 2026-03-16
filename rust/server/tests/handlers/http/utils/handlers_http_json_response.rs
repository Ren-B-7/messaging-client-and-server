/// Tests for JSON response delivery
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::Response;
use hyper::http::{HeaderValue, StatusCode};
use serde::Serialize;
use std::convert::Infallible;

use server::handlers::http::utils::*;

// ── helpers ──────────────────────────────────────────────────────────────

async fn body_string(res: Response<BoxBody<Bytes, Infallible>>) -> String {
    let bytes = res.collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[derive(Serialize)]
struct TestUser {
    id: u32,
    name: String,
    email: String,
}

#[derive(Serialize)]
struct TestData {
    count: u32,
    items: Vec<String>,
}

// ── deliver_serialized_json ───────────────────────────────────────────────

#[tokio::test]
async fn serialized_json_sets_content_type() {
    let user = TestUser {
        id: 1,
        name: "alice".into(),
        email: "alice@example.com".into(),
    };
    let res = deliver_serialized_json(&user, StatusCode::OK).unwrap();
    assert_eq!(res.headers()["content-type"], "application/json");
}

#[tokio::test]
async fn serialized_json_correct_status() {
    let user = TestUser {
        id: 1,
        name: "alice".into(),
        email: "alice@example.com".into(),
    };
    let res = deliver_serialized_json(&user, StatusCode::CREATED).unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn serialized_json_body_is_valid_json() {
    let user = TestUser {
        id: 42,
        name: "bob".into(),
        email: "bob@example.com".into(),
    };
    let res = deliver_serialized_json(&user, StatusCode::OK).unwrap();
    let body = body_string(res).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["id"], 42);
    assert_eq!(v["name"], "bob");
    assert_eq!(v["email"], "bob@example.com");
}

#[tokio::test]
async fn serialized_json_with_complex_data() {
    let data = TestData {
        count: 3,
        items: vec!["a".into(), "b".into(), "c".into()],
    };
    let res = deliver_serialized_json(&data, StatusCode::OK).unwrap();
    let body = body_string(res).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["count"], 3);
    assert_eq!(v["items"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn serialized_json_with_different_status_codes() {
    let user = TestUser {
        id: 1,
        name: "alice".into(),
        email: "alice@example.com".into(),
    };

    let res_accepted = deliver_serialized_json(&user, StatusCode::ACCEPTED).unwrap();
    assert_eq!(res_accepted.status(), StatusCode::ACCEPTED);

    let res_no_content = deliver_serialized_json(&user, StatusCode::OK).unwrap();
    assert_eq!(res_no_content.status(), StatusCode::OK);
}

// ── deliver_serialized_json_with_cookie ───────────────────────────────────

#[tokio::test]
async fn json_with_cookie_sets_set_cookie_header() {
    let user = TestUser {
        id: 1,
        name: "x".into(),
        email: "x@example.com".into(),
    };
    let cookie = HeaderValue::from_static("auth_id=abc; Path=/; HttpOnly");
    let res = deliver_serialized_json_with_cookie(&user, StatusCode::OK, cookie).unwrap();
    assert!(res.headers().contains_key("set-cookie"));
}

#[tokio::test]
async fn json_with_cookie_includes_data() {
    let user = TestUser {
        id: 99,
        name: "test_user".into(),
        email: "test@example.com".into(),
    };
    let cookie = HeaderValue::from_static("auth_id=token123");
    let res = deliver_serialized_json_with_cookie(&user, StatusCode::OK, cookie).unwrap();
    let body = body_string(res).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["id"], 99);
}

#[tokio::test]
async fn json_with_cookie_set_cookie_value() {
    let user = TestUser {
        id: 1,
        name: "x".into(),
        email: "x@example.com".into(),
    };
    let cookie = HeaderValue::from_static("session=xyz789; Max-Age=3600; Secure");
    let res = deliver_serialized_json_with_cookie(&user, StatusCode::OK, cookie).unwrap();
    let set_cookie = res.headers().get("set-cookie").unwrap();
    assert!(set_cookie.to_str().unwrap().contains("session=xyz789"));
}

// ── deliver_error_json ────────────────────────────────────────────────────

#[tokio::test]
async fn error_json_correct_status() {
    let res = deliver_error_json("NOT_FOUND", "Resource not found", StatusCode::NOT_FOUND).unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn error_json_body_structure() {
    let res = deliver_error_json("MY_CODE", "my message", StatusCode::BAD_REQUEST).unwrap();
    let body = body_string(res).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["status"], "error");
    assert_eq!(v["code"], "MY_CODE");
    assert_eq!(v["message"], "my message");
}

#[tokio::test]
async fn error_json_different_status_codes() {
    let status_codes = vec![
        (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
        (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
        (StatusCode::FORBIDDEN, "FORBIDDEN"),
        (StatusCode::NOT_FOUND, "NOT_FOUND"),
        (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
    ];

    for (status, code) in status_codes {
        let res = deliver_error_json(code, "error message", status).unwrap();
        assert_eq!(res.status(), status);
        let body = body_string(res).await;
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["code"], code);
    }
}

#[tokio::test]
async fn error_json_special_characters_in_message() {
    let message = r#"Error: "quoted" and \backslash\ chars"#;
    let res = deliver_error_json("ERROR", message, StatusCode::BAD_REQUEST).unwrap();
    let body = body_string(res).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["message"], message);
}

// ── deliver_success_json ──────────────────────────────────────────────────

#[tokio::test]
async fn success_json_data_and_message() {
    let user = TestUser {
        id: 7,
        name: "seven".into(),
        email: "seven@example.com".into(),
    };
    let res = deliver_success_json(Some(user), Some("User created"), StatusCode::CREATED).unwrap();
    let body = body_string(res).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["status"], "success");
    assert_eq!(v["message"], "User created");
    assert_eq!(v["data"]["id"], 7);
}

#[tokio::test]
async fn success_json_no_data_no_message() {
    let res = deliver_success_json::<TestUser>(None, None, StatusCode::OK).unwrap();
    let body = body_string(res).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["status"], "success");
    assert!(v.get("data").is_none());
    assert!(v.get("message").is_none());
}

#[tokio::test]
async fn success_json_only_message() {
    let res = deliver_success_json::<TestUser>(None, Some("Operation completed"), StatusCode::OK)
        .unwrap();
    let body = body_string(res).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["message"], "Operation completed");
    assert!(v.get("data").is_none());
}

#[tokio::test]
async fn success_json_only_data() {
    let user = TestUser {
        id: 5,
        name: "five".into(),
        email: "five@example.com".into(),
    };
    let res = deliver_success_json(Some(user), None, StatusCode::OK).unwrap();
    let body = body_string(res).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["status"], "success");
    assert_eq!(v["data"]["id"], 5);
    assert!(v.get("message").is_none());
}

#[tokio::test]
async fn success_json_with_different_status_codes() {
    let user = TestUser {
        id: 1,
        name: "test".into(),
        email: "test@example.com".into(),
    };
    let res = deliver_success_json(Some(user), Some("ok"), StatusCode::CREATED).unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

// ── deliver_json (raw bytes) ──────────────────────────────────────────────

#[tokio::test]
async fn raw_json_roundtrip() {
    let raw = r#"{"raw":true}"#;
    let res = deliver_json(raw.as_bytes().to_vec(), StatusCode::OK).unwrap();
    assert_eq!(res.headers()["content-type"], "application/json");
    let body = body_string(res).await;
    assert_eq!(body, raw);
}

#[tokio::test]
async fn raw_json_with_string() {
    let raw = r#"{"status":"ok","data":{"id":123}}"#;
    let res = deliver_json(raw, StatusCode::OK).unwrap();
    let body = body_string(res).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["status"], "ok");
}

#[tokio::test]
async fn raw_json_with_custom_status() {
    let raw = r#"{"error":"not found"}"#;
    let res = deliver_json(raw, StatusCode::NOT_FOUND).unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn raw_json_empty_object() {
    let raw = "{}";
    let res = deliver_json(raw, StatusCode::OK).unwrap();
    let body = body_string(res).await;
    let _v: serde_json::Value = serde_json::from_str(&body).unwrap();
    // Should parse as valid JSON
}

#[tokio::test]
async fn raw_json_empty_array() {
    let raw = "[]";
    let res = deliver_json(raw, StatusCode::OK).unwrap();
    let body = body_string(res).await;
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn raw_json_complex_structure() {
    let raw = r#"{"users":[{"id":1,"name":"Alice"},{"id":2,"name":"Bob"}],"total":2}"#;
    let res = deliver_json(raw, StatusCode::OK).unwrap();
    let body = body_string(res).await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["total"], 2);
    assert_eq!(v["users"][0]["name"], "Alice");
}

// ── Content-Type verification ──────────────────────────────────────────────

#[tokio::test]
async fn all_json_responses_have_correct_content_type() {
    let content_type = "application/json";

    let user = TestUser {
        id: 1,
        name: "test".into(),
        email: "test@example.com".into(),
    };

    let res1 = deliver_serialized_json(&user, StatusCode::OK).unwrap();
    assert_eq!(res1.headers()["content-type"], content_type);

    let res2 = deliver_error_json("ERROR", "message", StatusCode::BAD_REQUEST).unwrap();
    assert_eq!(res2.headers()["content-type"], content_type);

    let res3 = deliver_success_json(Some(user), None, StatusCode::OK).unwrap();
    assert_eq!(res3.headers()["content-type"], content_type);

    let res4 = deliver_json(r#"{"test":true}"#, StatusCode::OK).unwrap();
    assert_eq!(res4.headers()["content-type"], content_type);
}
