use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use tracing::info;

use crate::AppState;

/// Handle logout
pub async fn handle_logout(
    _req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("User logged out");

    // Clear the instance_id cookie. The session token is held by the frontend
    // in memory and discarded when the user logs out — no cookie to clear for it.
    let clear_cookie = "instance_id=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0";

    let response_json = serde_json::json!({
        "status": "success",
        "message": "Logged out successfully"
    });

    let json_bytes = Bytes::from(response_json.to_string());

    let response: Response<BoxBody<Bytes, Infallible>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("set-cookie", clear_cookie)
        .body(Full::new(json_bytes).boxed())
        .context("Failed to build logout response")?;

    Ok(response)
}
