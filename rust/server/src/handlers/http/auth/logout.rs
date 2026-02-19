use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use tracing::info;

use crate::AppState;
use crate::handlers::http::utils::{create_session_cookie, deliver_serialized_json_with_cookie};

/// Handle logout
pub async fn handle_logout(
    _req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("User logged out");

    // Clear the instance_id cookie. The session token is held by the frontend
    // in memory and discarded when the user logs out — no cookie to clear for it.
    let clear_cookie = create_session_cookie("instance_id", "", true)
        .context("Failed to create session instance cookie")?;

    let response_json = serde_json::json!({
        "status": "success",
        "message": "Logged out successfully"
    });

    deliver_serialized_json_with_cookie(&response_json, StatusCode::OK, clear_cookie)
}
