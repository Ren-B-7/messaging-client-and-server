use std::fmt;

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct PasswordResetToken {
    pub id: i64,
    pub user_id: i64,
    pub token: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub used: bool,
}

#[derive(Error, Debug, Clone, Serialize)]
pub enum PasswordResetError {
    InvalidId,
    Expired,
    AlreadyUsed,
    MissingField(String),
    InvalidToken,
    IncorrectUserId,
}

impl PasswordResetError {
    pub fn to_code(&self) -> &'static str {
        match self {
            Self::InvalidId => "ID_WAS_INCORRECT",
            Self::Expired => "TOKEN_HAS_EXPIRED",
            Self::AlreadyUsed => "TOKEN_ALREADY_SUED",
            Self::MissingField(_) => "MISSING_FIELD",
            Self::IncorrectUserId => "USER_ID_INCORRECT",
            Self::InvalidToken => "TOKEN_INVALID",
        }
    }

    pub fn to_message(&self) -> String {
        match self {
            Self::InvalidId => "The token id was invalid".to_string(),
            Self::Expired => "The token has already expired".to_string(),
            Self::AlreadyUsed => "Token has already been used".to_string(),
            Self::MissingField(field) => format!("Missing required field: {}", field),
            Self::IncorrectUserId => "User id is incorrect".to_string(),
            Self::InvalidToken => "Token was invalid".to_string(),
        }
    }
}

impl fmt::Display for PasswordResetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "code={}, message={}", self.to_code(), self.to_message())
    }
}
