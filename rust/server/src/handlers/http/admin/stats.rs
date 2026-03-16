use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use tokio_rusqlite::rusqlite;
use tracing::info;

use crate::AppState;
use crate::handlers::http::utils::json_response::*;
use shared::types::server_stats::{DatabaseInfo, ServerStats};

/// GET /admin/api/stats — serve live server and database statistics.
///
/// Hard-auth + is_admin guard applied by the router before this is called.
pub async fn handle_server_config(
    _req: Request<IncomingBody>,
    state: AppState,
    _admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Serving admin stats");

    let db_info = state
        .db
        .call(|conn| {
            let total_users: i64 = conn
                .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
                .unwrap_or(0);

            // Only count non-expired sessions so the dashboard figure is
            // meaningful — expired rows are cleaned up lazily by the periodic
            // task but may linger between runs.
            let active_sessions: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sessions WHERE expires_at > CAST(strftime('%s','now') AS INTEGER)",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            let banned_users: i64 = conn
                .query_row("SELECT COUNT(*) FROM users WHERE is_banned = 1", [], |r| {
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

    // Guard is dropped before the response is built.
    let stats = {
        let cfg = state.config.read().await;
        ServerStats::build(&cfg, db_info, 0)
    };

    deliver_serialized_json(&stats, StatusCode::OK)
}

/// GET /admin/api/metrics — live Tower middleware metrics snapshot.
///
/// Pure in-memory read — no database queries. Returns counters collected by
/// `MetricsLayer` plus current rate-limiter bucket stats.
///
/// Hard-auth + is_admin guard applied by the router before this is called.
pub async fn handle_metrics(
    _req: Request<IncomingBody>,
    state: AppState,
    _admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Serving admin metrics");

    let snapshot = state.metrics.snapshot().await;
    let rl_stats = state.rate_limiter.stats().await;

    deliver_success_json(
        Some(serde_json::json!({
            "total_requests":      snapshot.total_requests,
            "active_connections":  snapshot.active_connections,
            "error_count":         snapshot.error_count,
            "bytes_sent":          snapshot.bytes_sent,
            "bytes_received":      snapshot.bytes_received,
            "rate_limited":        snapshot.rate_limited,
            "ip_blocked":          snapshot.ip_blocked,
            "uptime_secs":         snapshot.uptime.as_secs_f64(),
            "requests_per_second": snapshot.requests_per_second(),
            "error_rate_pct":      snapshot.error_rate(),
            "latency_avg_ms":      snapshot.latency_avg.map(|d| d.as_secs_f64() * 1_000.0),
            "latency_p50_ms":      snapshot.latency_p50.map(|d| d.as_secs_f64() * 1_000.0),
            "latency_p95_ms":      snapshot.latency_p95.map(|d| d.as_secs_f64() * 1_000.0),
            "latency_p99_ms":      snapshot.latency_p99.map(|d| d.as_secs_f64() * 1_000.0),
            "rate_limiter": {
                "total_ips":    rl_stats.total_ips,
                "rate_limited": rl_stats.rate_limited,
                "capacity":     rl_stats.capacity,
                "refill_rate":  rl_stats.refill_rate,
            },
        })),
        None,
        StatusCode::OK,
    )
}
