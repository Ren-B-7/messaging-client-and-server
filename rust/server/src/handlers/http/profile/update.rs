use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::{Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};

use crate::AppState;

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
    fn to_code(&self) -> &'static str {
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

    fn to_message(&self) -> String {
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

    fn to_profile_response(&self) -> ProfileResponse {
        ProfileResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }

    fn to_update_response(&self) -> UpdateResponse {
        UpdateResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }
}

/// Get user profile handler
pub async fn handle_get_profile(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<http_body_util::Full<Bytes>>> {
    info!("Processing get profile request");

    // Extract user_id from session
    let user_id = match extract_user_from_request(&req, &state).await {
        Ok(id) => id,
        Err(err) => {
            warn!("Unauthorized profile access attempt");
            return deliver_profile_response(err.to_profile_response(), StatusCode::UNAUTHORIZED);
        }
    };

    // Get user profile
    match get_user_profile(user_id, &state).await {
        Ok(profile) => {
            info!("Profile retrieved for user {}", user_id);

            let response = ProfileResponse::Success {
                profile,
                message: "Profile retrieved successfully".to_string(),
            };

            deliver_profile_response(response, StatusCode::OK)
        }
        Err(err) => {
            error!("Failed to get profile: {:?}", err.to_code());
            deliver_profile_response(err.to_profile_response(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Update user profile handler
pub async fn handle_update_profile(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<http_body_util::Full<Bytes>>> {
    info!("Processing update profile request");

    // Extract user_id from session
    let user_id = match extract_user_from_request(&req, &state).await {
        Ok(id) => id,
        Err(err) => {
            warn!("Unauthorized profile update attempt");
            return deliver_update_response(err.to_update_response(), StatusCode::UNAUTHORIZED);
        }
    };

    // Parse update data
    let update_data = match parse_update_form(req).await {
        Ok(data) => data,
        Err(err) => {
            warn!("Profile update parsing failed: {:?}", err.to_code());
            return deliver_update_response(err.to_update_response(), StatusCode::BAD_REQUEST);
        }
    };

    // Update profile
    match update_user_profile(user_id, &update_data, &state).await {
        Ok(_) => {
            info!("Profile updated for user {}", user_id);

            let response = UpdateResponse::Success {
                message: "Profile updated successfully".to_string(),
            };

            deliver_update_response(response, StatusCode::OK)
        }
        Err(err) => {
            error!("Failed to update profile: {:?}", err.to_code());
            deliver_update_response(err.to_update_response(), StatusCode::BAD_REQUEST)
        }
    }
}

/// Extract authenticated user from request
async fn extract_user_from_request(
    req: &Request<hyper::body::Incoming>,
    state: &AppState,
) -> std::result::Result<i64, ProfileError> {
    use crate::database::login as db_login;

    // Extract token from Authorization header or cookie
    let token = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .or_else(|| {
            req.headers()
                .get("cookie")
                .and_then(|h| h.to_str().ok())
                .and_then(|cookies| {
                    cookies
                        .split(';')
                        .find(|c| c.trim().starts_with("auth_token="))
                        .and_then(|c| c.split('=').nth(1))
                })
        })
        .ok_or(ProfileError::Unauthorized)?;

    // Validate session token
    let user_id = db_login::validate_session(&state.db, token.to_string())
        .await
        .map_err(|_| ProfileError::DatabaseError)?
        .ok_or(ProfileError::Unauthorized)?;

    Ok(user_id)
}

/// Get user profile from database
async fn get_user_profile(
    user_id: i64,
    state: &AppState,
) -> std::result::Result<ProfileData, ProfileError> {
    use crate::database::register as db_register;

    let user = db_register::get_user_by_id(&state.db, user_id)
        .await
        .map_err(|e| {
            error!("Database error getting user: {}", e);
            ProfileError::DatabaseError
        })?
        .ok_or(ProfileError::UserNotFound)?;

    // Get last login from users table (you might need to add this to the query)
    Ok(ProfileData {
        user_id: user.id,
        username: user.username,
        email: user.email,
        created_at: user.created_at,
        last_login: None, // TODO: Add last_login to User struct
    })
}

/// Parse update form data
async fn parse_update_form(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<UpdateProfileData, ProfileError> {
    let body = req
        .collect()
        .await
        .map_err(|_| ProfileError::InternalError)?
        .to_bytes();

    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    let username = params
        .get("username")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let email = params
        .get("email")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(UpdateProfileData { username, email })
}

/// Update user profile in database
async fn update_user_profile(
    user_id: i64,
    data: &UpdateProfileData,
    state: &AppState,
) -> std::result::Result<(), ProfileError> {
    use crate::database::register as db_register;

    // Update username if provided
    if let Some(ref new_username) = data.username {
        // Validate username format
        if !crate::database::utils::is_valid_username(new_username) {
            return Err(ProfileError::InvalidUsername);
        }

        // Check if username is already taken
        let exists = db_register::username_exists(&state.db, new_username.clone())
            .await
            .map_err(|e| {
                error!("Database error checking username: {}", e);
                ProfileError::DatabaseError
            })?;

        if exists {
            // Check if it's the same user (they might be updating other fields)
            let current_user = db_register::get_user_by_id(&state.db, user_id)
                .await
                .map_err(|_| ProfileError::DatabaseError)?
                .ok_or(ProfileError::UserNotFound)?;

            if &current_user.username != new_username {
                return Err(ProfileError::UsernameTaken);
            }
        } else {
            // Update username
            db_register::update_username(&state.db, user_id, new_username.clone())
                .await
                .map_err(|e| {
                    error!("Database error updating username: {}", e);
                    ProfileError::DatabaseError
                })?;
        }
    }

    // Update email if provided
    if let Some(ref new_email) = data.email {
        // Validate email format
        if !crate::database::utils::is_valid_email(new_email) {
            return Err(ProfileError::InvalidEmail);
        }

        // Check if email is already taken
        let exists = db_register::email_exists(&state.db, new_email.clone())
            .await
            .map_err(|e| {
                error!("Database error checking email: {}", e);
                ProfileError::DatabaseError
            })?;

        if exists {
            return Err(ProfileError::EmailTaken);
        }

        // TODO: Add update_email function to register module
        // db_register::update_email(&state.db, user_id, new_email.clone()).await?;
    }

    Ok(())
}

/// Deliver profile JSON response
fn deliver_profile_response(
    response: ProfileResponse,
    status: StatusCode,
) -> Result<Response<http_body_util::Full<Bytes>>> {
    let json = serde_json::to_string(&response).context("Failed to serialize response")?;

    let response = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(json)))
        .context("Failed to build response")?;

    Ok(response)
}

/// Deliver update JSON response
fn deliver_update_response(
    response: UpdateResponse,
    status: StatusCode,
) -> Result<Response<http_body_util::Full<Bytes>>> {
    let json = serde_json::to_string(&response).context("Failed to serialize response")?;

    let response = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(json)))
        .context("Failed to build response")?;

    Ok(response)
}
