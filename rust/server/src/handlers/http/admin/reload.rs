// This is not entirely needed but i do it to know where it is used
use libc;

use std::convert::Infallible;

use anyhow::Result;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use tracing::{error, info};

use crate::handlers::http::utils::json_response::*;

/// POST /admin/api/reload — send SIGHUP to the current process.
///
/// This triggers the SIGHUP handler installed in `main.rs`, which re-reads
/// the config file from disk and atomically replaces the live config via
/// `LiveConfig::reload`.  The server keeps running without dropping any
/// in-flight connections.
///
/// Note: ports and `jwt_secret` are **not** re-read on a reload — those
/// require a full restart.  See the SIGHUP handler in `main.rs` for details.
///
/// Hard-auth + is_admin guard applied by the router before this is called.
pub async fn handle_reload_config(
    _req: Request<IncomingBody>,
    _admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Admin {} triggering config reload via SIGHUP", _admin_id);

    // SAFETY: kill(2) with the process's own PID and a valid signal number is
    // always safe.  We send SIGHUP to ourselves, which unblocks the signal
    // future installed in main.rs and causes it to reload the config file.
    let rc = unsafe { libc::kill(std::process::id() as libc::pid_t, libc::SIGHUP) };

    if rc != 0 {
        let errno = std::io::Error::last_os_error();
        error!("Failed to send SIGHUP to self: {}", errno);

        return deliver_serialized_json(
            &serde_json::json!({
                "status":  "error",
                "code":    "SIGNAL_FAILED",
                "message": format!("Could not send SIGHUP: {}", errno),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }

    info!("SIGHUP sent — config reload initiated");

    deliver_success_json(
        Some(serde_json::json!({
            "reloaded": true,
        })),
        Some("Config reload initiated"),
        StatusCode::OK,
    )
}
