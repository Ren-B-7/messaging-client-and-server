use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use tokio_rusqlite::rusqlite;
use tracing::info;

use crate::AppState;
use crate::handlers::http::utils::json_response::*;
use shared::types::server_config::AppConfig;
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

    // `state.started_at` is captured once at AppState::new() and never
    // changes, so this gives real elapsed time since the server started.
    // Previously this was passed as `0`, which caused uptime to display
    // the number of seconds since the Unix epoch (i.e. ~55 years).
    let stats = {
        let cfg = state.config.read().await;
        ServerStats::build(&cfg, db_info, state.started_at)
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

// ---------------------------------------------------------------------------
// Serialisable mirror of AppConfig (GET response)
// ---------------------------------------------------------------------------

/// Sent to the browser on GET /admin/api/config.
///
/// Mirrors `AppConfig` field-for-field with two intentional differences:
///   - `jwt_secret` is always `null` — it is write-only and must never be
///     echoed back in a response.
///   - `blocked_paths` is a sorted `Vec` rather than a `HashSet` so the UI
///     renders fields in a stable order.
#[derive(Debug, Serialize)]
struct ConfigView {
    server: ServerView,
    auth: AuthView,
    paths: PathsView,
}

#[derive(Debug, Serialize)]
struct ServerView {
    bind: String,
    port_admin: Option<u16>,
    port_client: Option<u16>,
    max_connections: usize,
}

#[derive(Debug, Serialize)]
struct AuthView {
    token_expiry_minutes: u64,
    email_required: bool,
    strict_ip_binding: bool,
    /// Always `null` in responses — the secret is write-only.
    jwt_secret: Option<String>,
    cors_origins: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PathsView {
    icons: String,
    web_dir: String,
    uploads_dir: String,
    /// Sorted for a stable rendering order in the UI.
    blocked_paths: Vec<String>,
}

impl From<&AppConfig> for ConfigView {
    fn from(cfg: &AppConfig) -> Self {
        let mut blocked: Vec<String> = cfg.paths.blocked_paths.iter().cloned().collect();
        blocked.sort();

        ConfigView {
            server: ServerView {
                bind: cfg.server.bind.clone(),
                port_admin: cfg.server.port_admin,
                port_client: cfg.server.port_client,
                max_connections: cfg.server.max_connections,
            },
            auth: AuthView {
                token_expiry_minutes: cfg.auth.token_expiry_minutes,
                email_required: cfg.auth.email_required,
                strict_ip_binding: cfg.auth.strict_ip_binding,
                jwt_secret: None, // redacted — never echoed
                cors_origins: cfg.auth.cors_origins.clone(),
            },
            paths: PathsView {
                icons: cfg.paths.icons.clone(),
                web_dir: cfg.paths.web_dir.clone(),
                uploads_dir: cfg.paths.uploads_dir.clone(),
                blocked_paths: blocked,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Patch body (PATCH request)
// ---------------------------------------------------------------------------

/// Partial update sent by the browser.  Every sub-object is optional so the
/// client only needs to include sections that contain modified fields.
#[derive(Debug, Deserialize)]
struct ConfigPatch {
    server: Option<ServerPatch>,
    auth: Option<AuthPatch>,
    paths: Option<PathsPatch>,
}

#[derive(Debug, Deserialize)]
struct ServerPatch {
    bind: Option<String>,
    port_admin: Option<u16>,
    port_client: Option<u16>,
    max_connections: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AuthPatch {
    token_expiry_minutes: Option<u64>,
    email_required: Option<bool>,
    strict_ip_binding: Option<bool>,
    /// Replaces the active JWT secret when supplied and non-empty.
    /// Rejected with 422 if shorter than 32 characters.
    jwt_secret: Option<String>,
    cors_origins: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct PathsPatch {
    icons: Option<String>,
    web_dir: Option<String>,
    uploads_dir: Option<String>,
    blocked_paths: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /admin/api/config — return the live config as `ConfigView` JSON.
///
/// `jwt_secret` is always redacted (null) in the response.
/// Hard-auth + is_admin guard applied by the router before this is called.
pub async fn handle_get_config(
    _req: Request<IncomingBody>,
    state: AppState,
    _admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Serving admin config");

    let cfg = state.config.read().await;
    let view = ConfigView::from(&*cfg);

    deliver_serialized_json(&view, StatusCode::OK)
}

/// PATCH /admin/api/config — apply a partial config update.
///
/// Reads the JSON body, validates it, clones the live config, applies the
/// patch field-by-field, then hot-reloads via `LiveConfig::reload`.
///
/// Returns 422 if `jwt_secret` is present but shorter than 32 characters.
/// Hard-auth + is_admin guard applied by the router before this is called.
pub async fn handle_patch_config(
    req: Request<IncomingBody>,
    state: AppState,
    _admin_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Patching admin config");

    // ── Consume + parse body ─────────────────────────────────────────────────
    let body_bytes = req
        .into_body()
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let patch: ConfigPatch =
        serde_json::from_slice(&body_bytes).context("Invalid JSON in config patch body")?;

    // ── Validate before touching the lock ────────────────────────────────────
    if let Some(auth) = &patch.auth {
        if let Some(secret) = &auth.jwt_secret {
            if !secret.is_empty() && secret.len() < 32 {
                return deliver_error_json(
                    "500",
                    "jwt_secret must be at least 32 characters",
                    StatusCode::UNPROCESSABLE_ENTITY,
                );
            }
        }
    }

    // ── Clone → patch → hot-reload ───────────────────────────────────────────
    // Read guard is dropped before reload to avoid a deadlock on the write lock.
    let new_config = {
        let current = state.config.read().await;
        apply_patch(current.clone(), patch)
    };

    state.config.reload(new_config).await;

    deliver_success_json(None::<()>, None, StatusCode::OK)
}

/// Pure function — clones the current config, applies every `Some` field from
/// the patch, and returns the updated value.  Nothing is mutated in-place.
fn apply_patch(mut cfg: AppConfig, patch: ConfigPatch) -> AppConfig {
    if let Some(sp) = patch.server {
        if let Some(v) = sp.bind {
            cfg.server.bind = v;
        }
        if let Some(v) = sp.port_admin {
            cfg.server.port_admin = Some(v);
        }
        if let Some(v) = sp.port_client {
            cfg.server.port_client = Some(v);
        }
        if let Some(v) = sp.max_connections {
            cfg.server.max_connections = v;
        }
    }

    if let Some(ap) = patch.auth {
        if let Some(v) = ap.token_expiry_minutes {
            cfg.auth.token_expiry_minutes = v;
        }
        if let Some(v) = ap.email_required {
            cfg.auth.email_required = v;
        }
        if let Some(v) = ap.strict_ip_binding {
            cfg.auth.strict_ip_binding = v;
        }
        if let Some(v) = ap.cors_origins {
            cfg.auth.cors_origins = v;
        }
        // Only replace the secret when the caller supplies a non-empty string.
        // An empty string means "leave unchanged" — the UI never sends a value
        // for the password field unless the admin explicitly types a new one.
        if let Some(v) = ap.jwt_secret {
            if !v.is_empty() {
                cfg.auth.jwt_secret = Some(v);
            }
        }
    }

    if let Some(pp) = patch.paths {
        if let Some(v) = pp.icons {
            cfg.paths.icons = v;
        }
        if let Some(v) = pp.web_dir {
            cfg.paths.web_dir = v;
        }
        if let Some(v) = pp.uploads_dir {
            cfg.paths.uploads_dir = v;
        }
        if let Some(v) = pp.blocked_paths {
            cfg.paths.blocked_paths = v.into_iter().collect();
        }
    }

    cfg
}
