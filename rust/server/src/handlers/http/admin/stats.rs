use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use tokio_rusqlite::rusqlite;
use tracing::info;

use crate::AppState;
use crate::handlers::http::utils::deliver_serialized_json;
use shared::types::server_stats::{DatabaseInfo, ServerStats};

/// Serve server and auth configuration stats
pub async fn handle_server_config(
    _req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Serving admin stats");

    // Gather live DB counts.
    // The Ok::<_, rusqlite::Error> turbofish satisfies the E: Send + 'static
    // bound that tokio_rusqlite::Connection::call requires on its closure.
    let db_info = state
        .db
        .call(|conn| {
            let total_users: i64 = conn
                .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
                .unwrap_or(0);

            let active_sessions: i64 = conn
                .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
                .unwrap_or(0);

            let banned_users: i64 = conn
                .query_row("SELECT COUNT(*) FROM users WHERE banned = 1", [], |r| {
                    r.get(0)
                })
                .unwrap_or(0);

            let total_messages: i64 = conn
                .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
                .unwrap_or(0);

            let total_groups: i64 = conn
                .query_row("SELECT COUNT(*) FROM groups", [], |r| r.get(0))
                .unwrap_or(0);

            Ok::<_, rusqlite::Error>(DatabaseInfo {
                path: "messaging.db".to_string(),
                total_users,
                active_sessions,
                banned_users,
                total_messages,
                total_groups,
            })
        })
        .await
        .context("Failed to query database stats")?;

    // Read config — guard is dropped before the response is built
    let stats = {
        let cfg = state.config.read().await;
        ServerStats::build(&cfg, db_info, 0)
    };

    deliver_serialized_json(&stats, StatusCode::OK)
}
