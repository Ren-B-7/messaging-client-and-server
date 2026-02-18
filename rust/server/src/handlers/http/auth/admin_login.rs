use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::http::utils::{self, deliver_serialized_json};

/// Login request data (supports both form-encoded and JSON)
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

/// Main admin login handler
pub async fn handle_login(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing admin login request");

    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let login_data = if content_type.contains("application/json") {
        match parse_login_json(req).await {
            Ok(data) => data,
            Err(e) => {
                warn!("Admin login JSON parsing failed: {:?}", e.to_code());
                return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
            }
        }
    } else {
        match parse_login_form(req).await {
            Ok(data) => data,
            Err(e) => {
                warn!("Admin login form parsing failed: {:?}", e.to_code());
                return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
            }
        }
    };

    if let Err(e) = validate_login(&login_data) {
        warn!("Admin login validation failed: {:?}", e.to_code());
        return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
    }

    match attempt_login(&login_data, &state).await {
        Ok((user_id, username, token)) => {
            info!(
                "Admin logged in successfully: {} (ID: {})",
                username, user_id
            );

            let token_expiry_secs = state.config.auth.token_expiry_minutes * 60;

            // The session token is stored both in the cookie and returned in the JSON
            // body so the frontend can send it as a Bearer header on subsequent requests.
            let instance_cookie = if login_data.remember_me {
                let max_age = std::time::Duration::from_secs(token_expiry_secs);
                utils::create_persistent_cookie("instance_id", &token, max_age, true)
                    .context("Failed to create persistent instance cookie")?
            } else {
                utils::create_session_cookie("instance_id", &token, true)
                    .context("Failed to create session instance cookie")?
            };

            let response_data = LoginResponse::Success {
                user_id,
                username,
                token,
                expires_in: token_expiry_secs,
                message: "Admin login successful".to_string(),
                redirect: "/admin".to_string(),
            };

            let json =
                serde_json::to_string(&response_data).context("Failed to serialize response")?;

            let response = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .header("set-cookie", instance_cookie)
                .body(utils::deliver_page::full(Bytes::from(json)))
                .context("Failed to build response")?;

            Ok(response)
        }
        Err(e) => {
            warn!("Admin login failed: {:?}", e.to_code());
            deliver_serialized_json(&e.to_response(), StatusCode::UNAUTHORIZED)
        }
    }
}

/// Parse login JSON data
async fn parse_login_json(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<LoginData, LoginError> {
    let body = req
        .collect()
        .await
        .map_err(|_| LoginError::InternalError)?
        .to_bytes();

    serde_json::from_slice::<LoginData>(&body).map_err(|e| {
        error!("Failed to parse admin login JSON: {}", e);
        LoginError::InternalError
    })
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
        .or_else(|| params.get("email"))
        .ok_or(LoginError::MissingField("username".to_string()))?
        .trim()
        .to_string();

    let password = params
        .get("password")
        .ok_or(LoginError::MissingField("password".to_string()))?
        .to_string();

    let remember_me = params
        .get("remember_me")
        .or_else(|| params.get("remember"))
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

/// Attempt to log in the admin user using the database
async fn attempt_login(
    data: &LoginData,
    state: &AppState,
) -> std::result::Result<(i64, String, String), LoginError> {
    use crate::database::login as db_login;

    info!("Attempting admin login for user: {}", data.username);

    let admin_auth = db_login::get_admin_auth(&state.db, data.username.clone())
        .await
        .map_err(|e| {
            error!("Database error getting admin auth: {}", e);
            LoginError::DatabaseError
        })?
        .ok_or_else(|| {
            warn!("Admin not found: {}", data.username);
            LoginError::InvalidCredentials
        })?;

    if admin_auth.is_banned {
        warn!("Banned admin attempted login: {}", data.username);
        return Err(LoginError::UserBanned);
    }

    let password_valid =
        crate::database::utils::verify_password(&admin_auth.password_hash, &data.password)
            .map_err(|e| {
                error!("Password verification error: {}", e);
                LoginError::InternalError
            })?;

    if !password_valid {
        warn!("Invalid password for admin: {}", data.username);
        return Err(LoginError::InvalidCredentials);
    }

    let token = crate::database::utils::generate_session_token();
    let token_expiry_secs = state.config.auth.token_expiry_minutes * 60;
    let expires_at = crate::database::utils::calculate_expiry(token_expiry_secs as i64);

    db_login::create_admin_session(
        &state.db,
        db_login::NewSession {
            user_id: admin_auth.id,
            session_token: token.clone(),
            expires_at,
            ip_address: None,
            user_agent: None,
        },
    )
    .await
    .map_err(|e| {
        error!("Failed to create admin session: {}", e);
        LoginError::DatabaseError
    })?;

    db_login::update_admin_last_login(&state.db, admin_auth.id)
        .await
        .map_err(|e| {
            error!("Failed to update admin last login: {}", e);
        })
        .ok();

    info!(
        "Admin login successful for user: {} (ID: {})",
        admin_auth.username, admin_auth.id
    );

    Ok((admin_auth.id, admin_auth.username, token))
}
