use serde::{Deserialize, Serialize};

/// Claims embedded in every JWT issued by the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    /// Numeric user ID (matches `users.id`).
    pub user_id: i64,
    /// UUID stored in `sessions.session_id`.
    pub session_id: String,
    pub user_agent: String,
    pub is_admin: bool,
    /// Standard JWT expiry (Unix timestamp, seconds).
    pub exp: usize,
    /// Issued-at (Unix timestamp, seconds).
    pub iat: usize,
}
