use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Request body for POST /api/register and POST /register.
///
/// # Removed field: `full_name`
///
/// The previous struct had `pub full_name: Option<String>` which was
/// deserialized from the request body but never written to the database
/// (the `users` table has no such column).  Clients submitting a name
/// received a 200/201 success response while their data was silently
/// discarded — a data integrity bug.
///
/// The field has been removed.  If full name support is added in the future,
/// the `users` table schema and `register_user` DB function must be updated
/// simultaneously so there is no window where the field is accepted but lost.
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterData {
    pub username: String,
    pub password: String,
    pub confirm_password: String,
    pub email: Option<String>,
}

/// Registration response codes
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RegisterResponse {
    Success {
        user_id: i64,
        username: String,
        message: String,
        redirect: String,
        token: Option<String>,
    },
    Error {
        code: String,
        message: String,
    },
}

/// Error codes for registration
#[derive(Error, Clone, Debug)]
pub enum RegisterError {
    UsernameTaken,
    EmailTaken,
    InvalidUsername,
    InvalidPassword,
    InvalidEmail,
    EmailRequired,
    PasswordMismatch,
    MissingField(String),
    DatabaseError,
    InternalError,
    WeakPassword,
}

impl RegisterError {
    pub fn to_code(&self) -> &'static str {
        match self {
            Self::UsernameTaken => "USERNAME_TAKEN",
            Self::EmailTaken => "EMAIL_TAKEN",
            Self::InvalidUsername => "INVALID_USERNAME",
            Self::InvalidPassword => "INVALID_PASSWORD",
            Self::InvalidEmail => "INVALID_EMAIL",
            Self::EmailRequired => "EMAIL_REQUIRED",
            Self::PasswordMismatch => "PASSWORD_MISMATCH",
            Self::MissingField(_) => "MISSING_FIELD",
            Self::DatabaseError => "DATABASE_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
            Self::WeakPassword => "WEAK_PASSWORD",
        }
    }

    pub fn to_message(&self) -> String {
        match self {
            Self::UsernameTaken => "Username is already taken".to_string(),
            Self::EmailTaken => "Email is already registered".to_string(),
            Self::InvalidUsername => {
                "Username must be 3-32 characters, alphanumeric, underscores, or hyphens only"
                    .to_string()
            }
            Self::InvalidPassword => {
                "Password must be 8-128 characters with at least one letter and one number"
                    .to_string()
            }
            Self::InvalidEmail => "Invalid email format".to_string(),
            Self::EmailRequired => "Email is required for registration".to_string(),
            Self::PasswordMismatch => "Passwords do not match".to_string(),
            Self::MissingField(field) => format!("Missing required field: {}", field),
            Self::DatabaseError => "Database error occurred".to_string(),
            Self::InternalError => "An internal error occurred".to_string(),
            Self::WeakPassword => "Password is too weak".to_string(),
        }
    }

    pub fn to_response(&self) -> RegisterResponse {
        RegisterResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "code={}, message={}", self.to_code(), self.to_message())
    }
}
