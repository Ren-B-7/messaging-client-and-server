use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use tracing::info;

use crate::AppState;

/// Serve server and auth configuration stats
pub async fn handle_stats(
    _req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Serving admin stats");

    let stats_json = serde_json::json!({
        "status": "success",
        "data": {
            "server": {
                "max_connections": state.config.server.max_connections,
                "bind":            state.config.server.bind,
                "port_client":     state.config.server.port_client,
                "port_admin":      state.config.server.port_admin,
            },
            "auth": {
                "token_expiry_minutes": state.config.auth.token_expiry_minutes,
                "email_required":       state.config.auth.email_required,
            }
        }
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(stats_json.to_string())).boxed())
        .context("Failed to build stats response")?;

    Ok(response)
}
