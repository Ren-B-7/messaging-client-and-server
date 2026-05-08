use serde::Deserialize;
use std::collections::HashSet;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
}

// ---------------------------------------------------------------------------
// Config structs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub bind: String,
    #[serde(default = "default_admin_port")]
    pub port_admin: Option<u16>,
    #[serde(default = "default_client_port")]
    pub port_client: Option<u16>,
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PathsConfig {
    pub icons: String,
    pub web_dir: String,
    #[serde(default)]
    pub blocked_paths: HashSet<String>,
    pub uploads_dir: String,
    #[serde(default = "default_db_path")]
    pub db_path: String,
}

pub fn default_db_path() -> String {
    "messaging.db".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    #[serde(default = "default_token_expiry")]
    pub token_expiry_minutes: u64,
    #[serde(default)]
    pub email_required: bool,
    /// HMAC key used to sign and verify JWTs.
    ///
    /// Prefer loading this via the `JWT_SECRET` environment variable.  This
    /// config field is the fallback for deployments that cannot inject env
    /// vars at runtime (e.g. certain container setups).
    ///
    /// **Minimum length:** 32 characters.
    /// **Hot-reload safe:** NO — the server reads this once at startup and
    /// stores it in `AppState.jwt_secret`.  Changing it via SIGHUP requires
    /// a restart because rotating the secret immediately invalidates every
    /// active session.
    pub jwt_secret: Option<String>,
    /// Whether to strictly reject requests whose IP does not match the IP
    /// stored in the session row (`sessions.ip_address`).
    ///
    /// | Value | Behaviour |
    /// |-------|-----------|
    /// | `true` (default) | Mismatch → **403 Forbidden**.  Strongest protection against stolen tokens, but will break mobile users who switch between WiFi and cellular, corporate users behind rotating NAT, and VPN users. |
    /// | `false` | Mismatch → **warn log only**.  The warning is still emitted so you can observe suspicious behaviour in logs without locking users out. |
    ///
    /// Set to `false` in environments where users frequently change IPs.
    /// The JWT signature and session-ID DB check still protect against replay
    /// attacks from a completely different device or network.
    #[serde(default = "default_strict_ip_binding")]
    pub strict_ip_binding: bool,
    /// Allowed CORS origins for the production CORS layer.
    ///
    /// In debug builds the CORS layer is permissive regardless of this list.
    /// In release builds, only origins listed here are allowed.
    ///
    /// Example:
    /// ```toml
    /// [auth]
    /// cors_origins = ["https://app.example.com", "https://admin.example.com"]
    /// ```
    ///
    /// Defaults to `["http://127.0.0.1:1337", "http://127.0.0.1:1338"]` so
    /// the server works out of the box on localhost without configuration.
    #[serde(default = "default_cors_origins")]
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub paths: PathsConfig,
    pub auth: AuthConfig,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

impl ServerConfig {
    /// Full bind address for the user-facing server, e.g. `"0.0.0.0:1337"`
    pub fn client_addr(&self) -> String {
        format!("{}:{}", self.bind, self.port_client.unwrap_or(1337))
    }

    /// Full bind address for the admin server, e.g. `"0.0.0.0:1338"`
    pub fn admin_addr(&self) -> String {
        format!("{}:{}", self.bind, self.port_admin.unwrap_or(1338))
    }
}

impl AuthConfig {
    /// Token expiry converted to seconds — convenience for cookie `Max-Age`.
    pub fn token_expiry_secs(&self) -> u64 {
        self.token_expiry_minutes * 60
    }

    pub fn email_required(&self) -> bool {
        self.email_required
    }

    /// Resolve the JWT secret with `JWT_SECRET` env-var taking priority over
    /// the config file field.
    ///
    /// Returns `None` when neither source is set (the server startup code
    /// treats this as a hard error).
    pub fn resolved_jwt_secret(&self) -> Option<String> {
        std::env::var("JWT_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| self.jwt_secret.clone())
            .filter(|s| !s.is_empty())
    }
}

// ---------------------------------------------------------------------------
// Serde defaults
// ---------------------------------------------------------------------------

pub fn default_admin_port() -> Option<u16> {
    Some(1338)
}

pub fn default_client_port() -> Option<u16> {
    Some(1337)
}

pub fn default_max_connections() -> usize {
    1000
}

pub fn default_timeout() -> u64 {
    10
}

pub fn default_token_expiry() -> u64 {
    60
}

/// Default to strict IP binding — safest setting for new deployments.
/// Operators running mobile-heavy user bases should set this to `false`.
pub fn default_strict_ip_binding() -> bool {
    true
}

/// Default CORS origins — localhost only, safe for development.
pub fn default_cors_origins() -> Vec<String> {
    vec![
        "http://127.0.0.1:1337".to_string(),
        "http://127.0.0.1:1338".to_string(),
    ]
}
