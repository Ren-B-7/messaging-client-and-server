use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// User / Admin auth rows
// ---------------------------------------------------------------------------

/// Minimal user data needed for login credential verification.
#[derive(Debug, Clone)]
pub struct UserAuth {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
}

/// Same shape as UserAuth, but only returned for rows where `is_admin = 1`.
#[derive(Debug, Clone)]
pub struct AdminAuth {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

/// Data required to INSERT a new session row.
///
/// # JWT migration notes
/// - `session_id` is a UUID v4 generated at login time.
///   It is embedded in the JWT `session_id` claim and stored here so the
///   secure-path validator can confirm the session hasn't been revoked.
/// - `user_agent` has been **removed** — it now lives exclusively in the
///   JWT claims, so there is no need to persist it in the DB.
#[derive(Debug, Clone)]
pub struct NewSession {
    pub user_id: i64,
    /// UUID that acts as the revocation handle (embedded in the JWT).
    pub session_id: String,
    pub expires_at: i64,
    /// Client IP captured at login; compared on every secure request.
    pub ip_address: Option<String>,
}

/// A full session row returned from the database.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    /// UUID embedded in the JWT claims.
    pub session_id: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub last_activity: i64,
    /// IP stored at login time; validated on secure (mutating) requests.
    pub ip_address: Option<String>,
}

// ---------------------------------------------------------------------------
// Login request / response wire types
// ---------------------------------------------------------------------------

/// Incoming login payload (form or JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginData {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub remember_me: bool,
}

/// Successful login response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginResponse {
    Success {
        user_id: i64,
        username: String,
        /// The signed JWT string — also set as the `auth_id` cookie.
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

#[derive(Debug, Clone, PartialEq)]
pub enum LoginError {
    MissingField(String),
    InvalidCredentials,
    UserBanned,
    DatabaseError,
    InternalError,
}

impl LoginError {
    pub fn to_code(&self) -> &'static str {
        match self {
            LoginError::MissingField(_) => "MISSING_FIELD",
            LoginError::InvalidCredentials => "INVALID_CREDENTIALS",
            LoginError::UserBanned => "USER_BANNED",
            LoginError::DatabaseError => "DATABASE_ERROR",
            LoginError::InternalError => "INTERNAL_ERROR",
        }
    }

    pub fn to_message(&self) -> String {
        match self {
            LoginError::MissingField(f) => format!("Missing field: {}", f),
            LoginError::InvalidCredentials => "Invalid username or password".to_string(),
            LoginError::UserBanned => "This account has been banned".to_string(),
            LoginError::DatabaseError => "A database error occurred".to_string(),
            LoginError::InternalError => "An internal error occurred".to_string(),
        }
    }

    pub fn to_response(&self) -> LoginResponse {
        LoginResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }
}
