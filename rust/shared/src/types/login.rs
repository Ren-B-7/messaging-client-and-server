use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct LoginData {
    #[serde(alias = "email")]
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub remember_me: bool,
}

/// Login response codes (for API-style responses)
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginResponse {
    Success {
        user_id: i64,
        username: String,
        token: String,
        expires_in: u64,
        message: String,
    },
    Error {
        code: String,
        message: String,
    },
}

/// Error codes for login
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

#[derive(Debug, Clone)]
pub struct LoginCredentials {
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Clone)]
pub struct UserAuth {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
}

/// Auth record for admin accounts — same users table, filtered by is_admin = 1
#[derive(Debug, Clone)]
pub struct AdminAuth {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    pub session_token: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub last_activity: i64,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewSession {
    pub user_id: i64,
    pub session_token: String,
    pub expires_at: i64,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl fmt::Display for NewSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "id - {:?}, ip - {:?}, agent - {:?}",
            self.user_id, self.ip_address, self.user_agent
        )
    }
}

impl fmt::Display for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "id - {:?}, ip - {:?}, agent - {:?}",
            self.user_id, self.ip_address, self.user_agent
        )
    }
}
