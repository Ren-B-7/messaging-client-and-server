use serde::Serialize;

use crate::types::server_config::AppConfig;

/// Point-in-time snapshot of runtime server statistics.
/// Serialized and returned by `GET /admin/api/stats`.
#[derive(Debug, Clone, Serialize)]
pub struct ServerStats {
    pub server: ServerInfo,
    pub auth: AuthInfo,
    pub database: DatabaseInfo,
    pub runtime: RuntimeInfo,
}

/// Static server configuration values shown in the stats response
#[derive(Debug, Clone, Serialize)]
pub struct ServerInfo {
    pub bind: String,
    pub port_client: u16,
    pub port_admin: u16,
    pub max_connections: usize,
}

/// Auth-related config values
#[derive(Debug, Clone, Serialize)]
pub struct AuthInfo {
    pub token_expiry_minutes: u64,
    pub email_required: bool,
}

/// Live database counts (populated at query time)
#[derive(Debug, Clone, Serialize)]
pub struct DatabaseInfo {
    pub path: String,
    pub total_users: i64,
    pub active_sessions: i64,
    pub banned_users: i64,
    pub total_messages: i64,
    pub total_groups: i64,
}

/// Runtime process info
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeInfo {
    /// Unix timestamp of when the server process started
    pub started_at: i64,
    /// Seconds elapsed since startup
    pub uptime_secs: i64,
}

impl ServerStats {
    /// Build a stats snapshot from config + live database counts.
    ///
    /// `config` is typically a short-lived read guard from `LiveConfig`:
    /// ```rust,no_run
    /// // let cfg = state.config.read().await;
    /// // let stats = ServerStats::build(&cfg, db_info, started_at);
    /// // guard drops here
    /// ```
    ///
    /// `started_at` should be captured once at process startup and passed in.
    pub fn build(config: &AppConfig, db_info: DatabaseInfo, started_at: i64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            server: ServerInfo {
                bind: config.server.bind.clone(),
                port_client: config.server.port_client.unwrap_or(1337),
                port_admin: config.server.port_admin.unwrap_or(1338),
                max_connections: config.server.max_connections,
            },
            auth: AuthInfo {
                token_expiry_minutes: config.auth.token_expiry_minutes,
                email_required: config.auth.email_required,
            },
            database: db_info,
            runtime: RuntimeInfo {
                started_at,
                uptime_secs: now - started_at,
            },
        }
    }
}

impl DatabaseInfo {
    /// Convenience constructor for before DB queries are wired in
    pub fn empty(path: &str) -> Self {
        Self {
            path: path.to_string(),
            total_users: 0,
            active_sessions: 0,
            banned_users: 0,
            total_messages: 0,
            total_groups: 0,
        }
    }
}
