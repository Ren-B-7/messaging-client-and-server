use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct RegistrationData {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
}

/// Registration response codes
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RegistrationResponse {
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
pub enum RegistrationError {
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
}

impl RegistrationError {
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
        }
    }

    pub fn to_response(&self) -> RegistrationResponse {
        RegistrationResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }
}
