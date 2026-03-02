//! Handlers for the presence (online status) endpoints.
//!
//! Routes (register these in `build_api_routes`):
//!
//!   POST /api/presence         — heartbeat; hard auth
//!   POST /api/presence/offline — explicit offline signal; hard auth
//!
//! The client calls POST /api/presence every 60 s.
//! The server marks the user offline if no heartbeat is seen for 120 s.

use std::convert::Infallible;

use anyhow::Result;
use bytes::Bytes;
use http::StatusCode;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response};
use tracing::{error, info};

use crate::AppState;
use crate::database::presence;
use crate::handlers::http::utils::{deliver_error_json, deliver_success_json};

/// `POST /api/presence`
///
/// Heartbeat endpoint.  The client calls this every 60 seconds while the
/// chat or profile page is open.  Requires hard auth so we have a verified
/// `user_id`.
///
/// Returns 200 OK with an empty body — the client ignores the response.
pub async fn handle_heartbeat(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    match presence::touch_presence(&state.db, user_id).await {
        Ok(_) => return deliver_success_json(Some(serde_json::json!({})), None, StatusCode::OK),
        Err(e) => {
            error!("[presence] heartbeat DB error for user {}: {}", user_id, e);
            deliver_error_json(
                "INTERNAL_ERROR",
                "Internal server error",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

/// `POST /api/presence/offline`
///
/// Explicit offline signal.  The client fires this via `navigator.sendBeacon`
/// on `beforeunload` so other users see the status change immediately rather
/// than waiting for the 2-minute timeout.
///
/// Also called internally by the logout handler.
///
/// Returns 200 OK — the client never reads this response (sendBeacon is
/// fire-and-forget).
pub async fn handle_set_offline(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    match presence::set_offline(&state.db, user_id).await {
        Ok(_) => return deliver_success_json(Some(serde_json::json!({})), None, StatusCode::OK),
        Err(e) => {
            error!(
                "[presence] set_offline DB error for user {}: {}",
                user_id, e
            );
            deliver_error_json(
                "INTERNAL_ERROR",
                "Internal server error",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}
