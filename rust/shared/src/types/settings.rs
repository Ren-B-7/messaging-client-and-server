use serde::{Deserialize, Serialize};

/// Change password request
#[derive(Debug, Deserialize)]
pub struct ChangePasswordData {
    pub current_password: String,
    pub new_password: String,
    pub confirm_password: String,
}

/// Settings response
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SettingsResponse {
    Success { message: String },
    Error { code: String, message: String },
}

/// Settings error codes
pub enum SettingsError {
    Unauthorized,
    InvalidCurrentPassword,
    InvalidNewPassword,
    PasswordMismatch,
    PasswordTooWeak,
    SamePassword,
    MissingField(String),
    DatabaseError,
    InternalError,
}

impl SettingsError {
    pub fn to_code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "UNAUTHORIZED",
            Self::InvalidCurrentPassword => "INVALID_CURRENT_PASSWORD",
            Self::InvalidNewPassword => "INVALID_NEW_PASSWORD",
            Self::PasswordMismatch => "PASSWORD_MISMATCH",
            Self::PasswordTooWeak => "PASSWORD_TOO_WEAK",
            Self::SamePassword => "SAME_PASSWORD",
            Self::MissingField(_) => "MISSING_FIELD",
            Self::DatabaseError => "DATABASE_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }

    pub fn to_message(&self) -> String {
        match self {
            Self::Unauthorized => "Authentication required".to_string(),
            Self::InvalidCurrentPassword => "Current password is incorrect".to_string(),
            Self::InvalidNewPassword => "Invalid new password format".to_string(),
            Self::PasswordMismatch => "New passwords do not match".to_string(),
            Self::PasswordTooWeak => {
                "Password must be 8-128 characters with at least one letter and one number"
                    .to_string()
            }
            Self::SamePassword => {
                "New password must be different from current password".to_string()
            }
            Self::MissingField(field) => format!("Missing required field: {}", field),
            Self::DatabaseError => "Database error occurred".to_string(),
            Self::InternalError => "An internal error occurred".to_string(),
        }
    }

    pub fn to_response(&self) -> SettingsResponse {
        SettingsResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }
}
