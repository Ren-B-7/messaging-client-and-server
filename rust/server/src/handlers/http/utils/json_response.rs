use anyhow::{Context, Result, anyhow};
use bytes::Bytes;
use http::HeaderValue;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::{Response, StatusCode, header};
use serde::Serialize;
use serde_json::json;
use std::convert::Infallible;
use tracing::{debug, error};

use crate::handlers::http::utils::deliver_page::full;

/// Serialize any `Serialize` type and deliver it as a JSON response.
/// This is the primary helper all handlers should use instead of
/// writing their own one-off serialization + response-building blocks.
pub fn deliver_serialized_json<T: Serialize>(
    data: &T,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let json = serde_json::to_string(data).context("Failed to serialize response")?;

    debug!(
        "Delivering serialized JSON response, size: {} bytes",
        json.len()
    );
    Ok(Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(json)).boxed())
        .map_err(|e| anyhow!("Failed to build JSON response: {}", e))?)
}

pub fn deliver_serialized_json_with_cookie<T: Serialize>(
    data: &T,
    status: StatusCode,
    cookie: HeaderValue,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let json = serde_json::to_string(data).context("Failed to serialize response")?;

    debug!(
        "Delivering serialized JSON response, size: {} bytes",
        json.len()
    );
    Ok(Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::SET_COOKIE, cookie)
        .body(Full::new(Bytes::from(json)).boxed())
        .map_err(|e| anyhow!("Failed to build JSON response: {}", e))?)
}

/// Delivers a JSON error response with the specified error code, message, and status.
pub fn deliver_error_json(
    error_code: &str,
    message: &str,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    error!(
        "Delivering error JSON: {} - {} ({})",
        status.as_u16(),
        error_code,
        message
    );

    let error_json = json!({
        "status": "error",
        "code": error_code,
        "message": message
    });

    let json_string = error_json.to_string();

    let response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(json_string)).boxed())
        .map_err(|e: http::Error| {
            error!("Failed to build error JSON response: {}", e);
            anyhow!("Failed to build error JSON response: {}", e)
        })?;

    Ok(response)
}

/// Delivers a success JSON response with optional data, message, and status code.
pub fn deliver_success_json<T: Serialize>(
    data: Option<T>,
    message: Option<&str>,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let response_body = match (data, message) {
        (Some(d), Some(m)) => json!({ "status": "success", "message": m, "data": d }),
        (Some(d), None) => json!({ "status": "success", "data": d }),
        (None, Some(m)) => json!({ "status": "success", "message": m }),
        (None, None) => json!({ "status": "success" }),
    };

    let json_string = response_body.to_string();

    let response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(json_string)).boxed())
        .map_err(|e: http::Error| {
            error!("Failed to build success JSON response: {}", e);
            anyhow!("Failed to build success JSON response: {}", e)
        })?;

    Ok(response)
}

/// Delivers a JSON response from raw pre-serialized bytes.
/// Prefer `deliver_serialized_json` when you have a typed value.
pub fn deliver_json<T: Into<Bytes>>(
    json: T,
    status: StatusCode,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let bytes: Bytes = json.into();

    debug!("Delivering raw JSON response, size: {} bytes", bytes.len());

    let response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(full(bytes))
        .map_err(|e: http::Error| {
            error!("Failed to build JSON response: {}", e);
            anyhow!("Failed to build JSON response: {}", e)
        })?;

    Ok(response)
}

// handlers/http/utils/json_response.rs  — append inside the file
// ─────────────────────────────────────────────────────────────────────────────
// Place this block at the bottom of json_response.rs
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;
    use http_body_util::BodyExt;
    use serde::Serialize;

    // ── helpers ──────────────────────────────────────────────────────────────

    async fn body_string(res: Response<BoxBody<Bytes, Infallible>>) -> String {
        let bytes = res.collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[derive(Serialize)]
    struct Dummy {
        id: u32,
        name: String,
    }

    // ── deliver_serialized_json ───────────────────────────────────────────────

    #[tokio::test]
    async fn serialized_json_sets_content_type() {
        let d = Dummy {
            id: 1,
            name: "alice".into(),
        };
        let res = deliver_serialized_json(&d, StatusCode::OK).unwrap();
        assert_eq!(res.headers()["content-type"], "application/json");
    }

    #[tokio::test]
    async fn serialized_json_correct_status() {
        let d = Dummy {
            id: 1,
            name: "alice".into(),
        };
        let res = deliver_serialized_json(&d, StatusCode::CREATED).unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn serialized_json_body_is_valid_json() {
        let d = Dummy {
            id: 42,
            name: "bob".into(),
        };
        let res = deliver_serialized_json(&d, StatusCode::OK).unwrap();
        let body = body_string(res).await;
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["id"], 42);
        assert_eq!(v["name"], "bob");
    }

    // ── deliver_serialized_json_with_cookie ───────────────────────────────────

    #[tokio::test]
    async fn json_with_cookie_sets_set_cookie_header() {
        use http::HeaderValue;
        let d = Dummy {
            id: 1,
            name: "x".into(),
        };
        let cookie = HeaderValue::from_static("auth_id=abc; Path=/; HttpOnly");
        let res = deliver_serialized_json_with_cookie(&d, StatusCode::OK, cookie).unwrap();
        assert!(res.headers().contains_key("set-cookie"));
    }

    // ── deliver_error_json ────────────────────────────────────────────────────

    #[tokio::test]
    async fn error_json_correct_status() {
        let res = deliver_error_json("NOT_FOUND", "missing", StatusCode::NOT_FOUND).unwrap();
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

    // ── deliver_success_json ──────────────────────────────────────────────────

    #[tokio::test]
    async fn success_json_data_and_message() {
        let d = Dummy {
            id: 7,
            name: "seven".into(),
        };
        let res = deliver_success_json(Some(d), Some("ok"), StatusCode::OK).unwrap();
        let body = body_string(res).await;
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["status"], "success");
        assert_eq!(v["message"], "ok");
        assert_eq!(v["data"]["id"], 7);
    }

    #[tokio::test]
    async fn success_json_no_data_no_message() {
        let res = deliver_success_json::<Dummy>(None, None, StatusCode::OK).unwrap();
        let body = body_string(res).await;
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["status"], "success");
        assert!(v.get("data").is_none());
        assert!(v.get("message").is_none());
    }

    #[tokio::test]
    async fn success_json_only_message() {
        let res = deliver_success_json::<Dummy>(None, Some("done"), StatusCode::OK).unwrap();
        let body = body_string(res).await;
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["message"], "done");
        assert!(v.get("data").is_none());
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
}
