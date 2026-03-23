use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Login wire types
// ---------------------------------------------------------------------------

/// Deserialized request body for POST /api/login and POST /login.
///
/// # Field name
///
/// The field is named `username` because authentication is always performed
/// against `users.username`.  Email login is not supported — the database
/// does not have a unique index on `users.email` suitable for auth lookups,
/// and the registration flow allows email to be omitted entirely.
///
/// The previous `#[serde(alias = "email")]` annotation was removed because it
/// silently accepted `{"email": "alice"}` as if it were a username lookup,
/// which would mislead clients into thinking email-based auth was supported.
/// Any client sending `"email"` instead of `"username"` will now receive a
/// 400/401 response, making the contract explicit.
#[derive(Debug, Deserialize)]
pub struct LoginData {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub remember_me: bool,
}

/// Successful / failed login response envelope.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginResponse {
    Success {
        user_id: i64,
        username: String,
        /// Signed JWT string — also set as the `auth_id` cookie.
        token: String,
        expires_in: u64,
        message: String,
    },
    Error {
        code: String,
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Login errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub enum LoginError {
    InvalidCredentials,
    UserBanned,
    UserNotFound,
    MissingField(String),
    DatabaseError,
    InternalError,
}

impl LoginError {
    pub fn to_code(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::UserBanned => "USER_BANNED",
            Self::UserNotFound => "USER_NOT_FOUND",
            Self::MissingField(_) => "MISSING_FIELD",
            Self::DatabaseError => "DATABASE_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }

    pub fn to_message(&self) -> String {
        match self {
            Self::InvalidCredentials => "Invalid username or password".to_string(),
            Self::UserBanned => "This account has been banned".to_string(),
            Self::UserNotFound => "User not found".to_string(),
            Self::MissingField(field) => format!("Missing required field: {}", field),
            Self::DatabaseError => "Database error occurred".to_string(),
            Self::InternalError => "An internal error occurred".to_string(),
        }
    }

    pub fn to_response(&self) -> LoginResponse {
        LoginResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }
}

// ---------------------------------------------------------------------------
// Credential helper
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LoginCredentials {
    pub username: String,
    pub password_hash: String,
}

// ---------------------------------------------------------------------------
// Auth rows returned from the database
// ---------------------------------------------------------------------------

/// Minimal data needed to verify a regular user's credentials.
#[derive(Debug, Clone)]
pub struct UserAuth {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
}

/// Auth record for admin accounts — same `users` table, filtered by `is_admin = 1`.
#[derive(Debug, Clone)]
pub struct AdminAuth {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
}

/// Data required to INSERT a new session row.
#[derive(Debug, Clone)]
pub struct NewSession {
    pub user_id: i64,
    /// UUID revocation handle — must match `JwtClaims.session_id`.
    pub session_id: String,
    pub expires_at: i64,
    /// Client IP captured at login; compared on every secure (mutating) request.
    pub ip_address: Option<String>,
}

/// A full session row read back from the database.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    pub session_id: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub last_activity: i64,
    pub ip_address: Option<String>,
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for NewSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "user_id={}, session_id={}, ip={:?}",
            self.user_id, self.session_id, self.ip_address
        )
    }
}

impl fmt::Display for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "id={}, user_id={}, session_id={}, ip={:?}",
            self.id, self.user_id, self.session_id, self.ip_address
        )
    }
}
