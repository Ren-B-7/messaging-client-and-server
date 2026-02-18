use serde::{Deserialize, Serialize};

/// Profile data
#[derive(Debug, Serialize)]
pub struct ProfileData {
    pub user_id: i64,
    pub username: String,
    pub email: Option<String>,
    pub created_at: i64,
    pub last_login: Option<i64>,
}

/// Update profile request
#[derive(Debug, Deserialize)]
pub struct UpdateProfileData {
    pub username: Option<String>,
    pub email: Option<String>,
}

/// Profile response
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProfileResponse {
    Success {
        profile: ProfileData,
        message: String,
    },
    Error {
        code: String,
        message: String,
    },
}

/// Update response
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UpdateResponse {
    Success { message: String },
    Error { code: String, message: String },
}

/// Profile error codes
pub enum ProfileError {
    Unauthorized,
    UserNotFound,
    InvalidUsername,
    InvalidEmail,
    UsernameTaken,
    EmailTaken,
    MissingField(String),
    DatabaseError,
    InternalError,
}

impl ProfileError {
    pub fn to_code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "UNAUTHORIZED",
            Self::UserNotFound => "USER_NOT_FOUND",
            Self::InvalidUsername => "INVALID_USERNAME",
            Self::InvalidEmail => "INVALID_EMAIL",
            Self::UsernameTaken => "USERNAME_TAKEN",
            Self::EmailTaken => "EMAIL_TAKEN",
            Self::MissingField(_) => "MISSING_FIELD",
            Self::DatabaseError => "DATABASE_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }

    pub fn to_message(&self) -> String {
        match self {
            Self::Unauthorized => "Authentication required".to_string(),
            Self::UserNotFound => "User not found".to_string(),
            Self::InvalidUsername => {
                "Username must be 3-32 characters, alphanumeric, underscores, or hyphens only"
                    .to_string()
            }
            Self::InvalidEmail => "Invalid email format".to_string(),
            Self::UsernameTaken => "Username is already taken".to_string(),
            Self::EmailTaken => "Email is already registered".to_string(),
            Self::MissingField(field) => format!("Missing required field: {}", field),
            Self::DatabaseError => "Database error occurred".to_string(),
            Self::InternalError => "An internal error occurred".to_string(),
        }
    }

    pub fn to_profile_response(&self) -> ProfileResponse {
        ProfileResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }

    pub fn to_update_response(&self) -> UpdateResponse {
        UpdateResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }
}
