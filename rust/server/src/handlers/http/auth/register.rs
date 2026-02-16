use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::{Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};

use crate::AppState;

/// Registration request data
#[derive(Debug, Clone, Deserialize)]
pub struct RegistrationData {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
}

/// Registration response codes
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RegistrationResponse {
    Success {
        user_id: i64,
        username: String,
        message: String,
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
    fn to_code(&self) -> &'static str {
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

    fn to_message(&self) -> String {
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

    fn to_response(&self) -> RegistrationResponse {
        RegistrationResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }
}

/// Main registration handler
pub async fn handle_register(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<http_body_util::Full<Bytes>>> {
    info!("Processing registration request");

    // Parse and validate registration data
    let registration_data = match parse_and_validate_registration(req, &state).await {
        Ok(data) => data,
        Err(reg_error) => {
            warn!("Registration validation failed: {:?}", reg_error.to_code());
            return deliver_json_response(reg_error.to_response(), StatusCode::BAD_REQUEST);
        }
    };

    // Hash password
    let hashed_password =
        hash_password(&registration_data.password).context("Failed to hash password")?;

    // Attempt to register user
    match attempt_registration(&registration_data, &hashed_password, &state).await {
        Ok((user_id, username)) => {
            info!(
                "User registered successfully: {} (ID: {})",
                username, user_id
            );

            let response = RegistrationResponse::Success {
                user_id,
                username,
                message: "Registration successful".to_string(),
            };

            deliver_json_response(response, StatusCode::CREATED)
        }
        Err(reg_error) => {
            error!("Registration failed: {:?}", reg_error.to_code());
            deliver_json_response(reg_error.to_response(), StatusCode::BAD_REQUEST)
        }
    }
}

/// Parse and validate registration form data
async fn parse_and_validate_registration(
    req: Request<hyper::body::Incoming>,
    state: &AppState,
) -> std::result::Result<RegistrationData, RegistrationError> {
    // Collect the request body
    let body = req
        .collect()
        .await
        .map_err(|_| RegistrationError::InternalError)?
        .to_bytes();

    // Parse the form data
    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    // Extract and validate username
    let username = params
        .get("username")
        .ok_or(RegistrationError::MissingField("username".to_string()))?
        .trim()
        .to_string();

    validate_username(&username)?;

    // Extract and validate password
    let password = params
        .get("password")
        .ok_or(RegistrationError::MissingField("password".to_string()))?
        .to_string();

    validate_password(&password)?;

    // Verify password confirmation if provided
    if let Some(confirm) = params.get("password_confirm") {
        if password != *confirm {
            return Err(RegistrationError::PasswordMismatch);
        }
    }

    // Extract and validate email
    let email = params
        .get("email")
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty());

    // Check if email is required by config
    if state.config.auth.email_required && email.is_none() {
        return Err(RegistrationError::EmailRequired);
    }

    // Validate email format if provided
    if let Some(ref email_str) = email {
        if !is_valid_email(email_str) {
            return Err(RegistrationError::InvalidEmail);
        }
    }

    Ok(RegistrationData {
        username,
        password,
        email,
    })
}

/// Validate username format
fn validate_username(username: &str) -> std::result::Result<(), RegistrationError> {
    if username.is_empty() || username.len() < 3 || username.len() > 32 {
        return Err(RegistrationError::InvalidUsername);
    }

    if !username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(RegistrationError::InvalidUsername);
    }

    Ok(())
}

/// Validate password format
fn validate_password(password: &str) -> std::result::Result<(), RegistrationError> {
    if password.is_empty() || password.len() < 8 || password.len() > 128 {
        return Err(RegistrationError::InvalidPassword);
    }

    if !password.chars().any(|c| c.is_numeric()) {
        return Err(RegistrationError::InvalidPassword);
    }

    if !password.chars().any(|c| c.is_alphabetic()) {
        return Err(RegistrationError::InvalidPassword);
    }

    Ok(())
}

/// Basic email validation
fn is_valid_email(email: &str) -> bool {
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let domain_parts: Vec<&str> = parts[1].split('.').collect();
    if domain_parts.len() < 2 {
        return false;
    }

    !parts[0].is_empty() && !parts[1].is_empty() && domain_parts.iter().all(|p| !p.is_empty())
}

/// Hash password using Argon2
fn hash_password(password: &str) -> Result<String> {
    crate::database::utils::hash_password(password).context("Failed to hash password with Argon2")
}

/// Attempt to register the user in the database
async fn attempt_registration(
    data: &RegistrationData,
    hashed_password: &str,
    state: &AppState,
) -> std::result::Result<(i64, String), RegistrationError> {
    use crate::database::register as db_register;

    info!("Attempting registration for user: {}", data.username);

    // Check if username already exists
    let username_exists = db_register::username_exists(&state.db, data.username.clone())
        .await
        .map_err(|e| {
            error!("Database error checking username: {}", e);
            RegistrationError::DatabaseError
        })?;

    if username_exists {
        warn!("Username already taken: {}", data.username);
        return Err(RegistrationError::UsernameTaken);
    }

    // Check if email already exists (if provided)
    if let Some(ref email) = data.email {
        let email_exists = db_register::email_exists(&state.db, email.clone())
            .await
            .map_err(|e| {
                error!("Database error checking email: {}", e);
                RegistrationError::DatabaseError
            })?;

        if email_exists {
            warn!("Email already registered: {}", email);
            return Err(RegistrationError::EmailTaken);
        }
    }

    // Register the user
    let user_id = db_register::register_user(
        &state.db,
        db_register::NewUser {
            username: data.username.clone(),
            password_hash: hashed_password.to_string(),
            email: data.email.clone(),
        },
    )
    .await
    .map_err(|e| {
        error!("Database error creating user: {}", e);
        RegistrationError::DatabaseError
    })?;

    info!(
        "User registered successfully: {} (ID: {})",
        data.username, user_id
    );

    Ok((user_id, data.username.clone()))
}

/// Deliver JSON response
fn deliver_json_response(
    response: RegistrationResponse,
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
