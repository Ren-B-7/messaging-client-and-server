use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::{Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::http::utils;

/// Login request data
#[derive(Debug, Deserialize)]
pub struct LoginData {
    pub username: String,
    pub password: String,
    pub remember_me: bool,
}

/// Login response codes
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginResponse {
    Success {
        user_id: i64,
        username: String,
        token: String,
        expires_in: u64,
        message: String,
        redirect: String,
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
    fn to_code(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::UserBanned => "USER_BANNED",
            Self::UserNotFound => "USER_NOT_FOUND",
            Self::MissingField(_) => "MISSING_FIELD",
            Self::DatabaseError => "DATABASE_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }

    fn to_message(&self) -> String {
        match self {
            Self::InvalidCredentials => "Invalid username or password".to_string(),
            Self::UserBanned => "This account has been banned".to_string(),
            Self::UserNotFound => "User not found".to_string(),
            Self::MissingField(field) => format!("Missing required field: {}", field),
            Self::DatabaseError => "Database error occurred".to_string(),
            Self::InternalError => "An internal error occurred".to_string(),
        }
    }

    fn to_response(&self) -> LoginResponse {
        LoginResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }
}

/// Main login handler
pub async fn handle_login(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<http_body_util::Full<Bytes>>> {
    info!("Processing login request");

    // Parse login form
    let login_data = match parse_login_form(req).await {
        Ok(data) => data,
        Err(login_error) => {
            warn!("Login parsing failed: {:?}", login_error.to_code());
            return deliver_json_response(login_error.to_response(), StatusCode::BAD_REQUEST);
        }
    };

    // Validate input
    if let Err(login_error) = validate_login(&login_data) {
        warn!("Login validation failed: {:?}", login_error.to_code());
        return deliver_json_response(login_error.to_response(), StatusCode::BAD_REQUEST);
    }

    // Attempt login
    match attempt_login(&login_data, &state).await {
        Ok((user_id, username, token)) => {
            info!(
                "User logged in successfully: {} (ID: {})",
                username, user_id
            );

            // Calculate token expiry
            let token_expiry_secs = state.config.auth.token_expiry_minutes * 60;
            let token_expiry = Duration::from_secs(token_expiry_secs);

            // Create cookie with token
            let cookie = if login_data.remember_me {
                utils::create_persistent_cookie("auth_token", &token, token_expiry, true)
                    .context("Failed to create persistent cookie")?
            } else {
                utils::create_session_cookie("auth_token", &token, true)
                    .context("Failed to create session cookie")?
            };

            let response_data = LoginResponse::Success {
                user_id,
                username,
                token: token.clone(),
                expires_in: token_expiry_secs,
                message: "Login successful".to_string(),
                redirect: "/chat".to_string(),
            };

            let json =
                serde_json::to_string(&response_data).context("Failed to serialize response")?;

            let response = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .header("set-cookie", cookie)
                .body(http_body_util::Full::new(Bytes::from(json)))
                .context("Failed to build response")?;

            Ok(response)
        }
        Err(login_error) => {
            warn!("Login failed: {:?}", login_error.to_code());
            deliver_json_response(login_error.to_response(), StatusCode::UNAUTHORIZED)
        }
    }
}

/// Parse login form data
async fn parse_login_form(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<LoginData, LoginError> {
    let body = req
        .collect()
        .await
        .map_err(|_| LoginError::InternalError)?
        .to_bytes();

    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    let username = params
        .get("username")
        .ok_or(LoginError::MissingField("username".to_string()))?
        .trim()
        .to_string();

    let password = params
        .get("password")
        .ok_or(LoginError::MissingField("password".to_string()))?
        .to_string();

    let remember_me = params
        .get("remember_me")
        .map(|v| v == "on" || v == "true" || v == "1")
        .unwrap_or(false);

    Ok(LoginData {
        username,
        password,
        remember_me,
    })
}

/// Validate login data
fn validate_login(data: &LoginData) -> std::result::Result<(), LoginError> {
    if data.username.is_empty() {
        return Err(LoginError::MissingField("username".to_string()));
    }

    if data.password.is_empty() {
        return Err(LoginError::MissingField("password".to_string()));
    }

    Ok(())
}

/// Attempt to log in the user using the database
async fn attempt_login(
    data: &LoginData,
    state: &AppState,
) -> std::result::Result<(i64, String, String), LoginError> {
    use crate::database::login as db_login;

    info!("Attempting login for user: {}", data.username);

    // Get user authentication data from database
    let user_auth = db_login::get_user_auth(&state.db, data.username.clone())
        .await
        .map_err(|e| {
            error!("Database error getting user auth: {}", e);
            LoginError::DatabaseError
        })?
        .ok_or_else(|| {
            warn!("User not found: {}", data.username);
            LoginError::InvalidCredentials
        })?;

    // Check if user is banned
    if user_auth.is_banned {
        warn!("Banned user attempted login: {}", data.username);
        return Err(LoginError::UserBanned);
    }

    // Verify password using utils (argon2)
    let password_valid =
        crate::database::utils::verify_password(&user_auth.password_hash, &data.password).map_err(
            |e| {
                error!("Password verification error: {}", e);
                LoginError::InternalError
            },
        )?;

    if !password_valid {
        warn!("Invalid password for user: {}", data.username);
        return Err(LoginError::InvalidCredentials);
    }

    // Generate secure session token
    let token = crate::database::utils::generate_session_token();

    // Calculate token expiry
    let token_expiry_secs = state.config.auth.token_expiry_minutes * 60;
    let expires_at = crate::database::utils::calculate_expiry(token_expiry_secs as i64);

    // Create session in database
    db_login::create_session(
        &state.db,
        db_login::NewSession {
            user_id: user_auth.id,
            session_token: token.clone(),
            expires_at,
            ip_address: None, // TODO: Extract from request
            user_agent: None, // TODO: Extract from request
        },
    )
    .await
    .map_err(|e| {
        error!("Failed to create session: {}", e);
        LoginError::DatabaseError
    })?;

    // Update last login timestamp
    db_login::update_last_login(&state.db, user_auth.id)
        .await
        .map_err(|e| {
            error!("Failed to update last login: {}", e);
            // Don't fail login for this
        })
        .ok();

    info!(
        "Login successful for user: {} (ID: {})",
        user_auth.username, user_auth.id
    );

    Ok((user_auth.id, user_auth.username, token))
}

/// Deliver JSON response
fn deliver_json_response(
    response: LoginResponse,
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
