use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use tracing::info;

use crate::AppState;

/// Handle logout
pub async fn handle_logout(
    _req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    info!("User logged out");

    // Delete auth cookie
    let cookie = "auth_token=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0";

    let response_json = serde_json::json!({
        "status": "success",
        "message": "Logged out successfully"
    });

    let json_string: String = response_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("set-cookie", cookie)
        .body(Full::new(json_bytes))
        .context("Failed to build logout response")?;

    Ok(response)
}
