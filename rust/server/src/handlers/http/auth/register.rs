use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response, StatusCode};
use std::collections::HashMap;
use std::convert::Infallible;
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::http::utils::{
    create_session_cookie, deliver_serialized_json, deliver_serialized_json_with_cookie,
};
use shared::types::register::*;

/// Registration request data (supports both form-encoded and JSON)
/// Main registration handler - supports both JSON and form submissions
pub async fn handle_register(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing registration request");

    let registration_data = match parse_and_validate_registration(req, &state).await {
        Ok(data) => data,
        Err(e) => {
            warn!("Registration validation failed: {:?}", e.to_code());
            return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
        }
    };

    let hashed_password =
        hash_password(&registration_data.password).context("Failed to hash password")?;

    match attempt_registration(&registration_data, &hashed_password, &state).await {
        Ok((user_id, username)) => {
            info!(
                "User registered successfully: {} (ID: {})",
                username, user_id
            );

            // Create a session for the new user so they are auto-logged-in.
            let session_token = crate::database::utils::generate_uuid_token();
            let session_created = create_session_for_new_user(user_id, &session_token, &state)
                .await
                .is_ok();

            // The session token is stored both in the cookie and returned in the JSON
            // body so the frontend can send it as a Bearer header on subsequent requests.
            let instance_cookie = create_session_cookie("auth_id", &session_token, true)
                .context("Failed to create instance cookie")?;

            let response = RegistrationResponse::Success {
                user_id,
                username,
                message: "Registration successful".to_string(),
                redirect: "/chat".to_string(),
                token: if session_created {
                    Some(session_token)
                } else {
                    None
                },
            };

            Ok(
                deliver_serialized_json_with_cookie(&response, StatusCode::OK, instance_cookie)
                    .unwrap(),
            )
        }
        Err(e) => {
            error!("Registration failed: {:?}", e.to_code());
            deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST)
        }
    }
}

/// Parse and validate registration data from either JSON or form body
async fn parse_and_validate_registration(
    req: Request<hyper::body::Incoming>,
    state: &AppState,
) -> std::result::Result<RegistrationData, RegistrationError> {
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let data = if content_type.contains("application/json") {
        let body = req
            .collect()
            .await
            .map_err(|_| RegistrationError::InternalError)?
            .to_bytes();

        serde_json::from_slice::<RegistrationData>(&body).map_err(|e| {
            error!("Failed to parse registration JSON: {}", e);
            RegistrationError::InternalError
        })?
    } else {
        let body = req
            .collect()
            .await
            .map_err(|_| RegistrationError::InternalError)?
            .to_bytes();

        let params = form_urlencoded::parse(body.as_ref())
            .into_owned()
            .collect::<HashMap<String, String>>();

        let username = params
            .get("username")
            .ok_or(RegistrationError::MissingField("username".to_string()))?
            .trim()
            .to_string();

        let password = params
            .get("password")
            .ok_or(RegistrationError::MissingField("password".to_string()))?
            .to_string();

        if let Some(confirm) = params.get("password_confirm") {
            if password != *confirm {
                return Err(RegistrationError::PasswordMismatch);
            }
        }

        let email = params
            .get("email")
            .map(|e| e.trim().to_string())
            .filter(|e| !e.is_empty());

        let full_name = params
            .get("full_name")
            .or_else(|| params.get("fullName"))
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty());

        let avatar = params.get("avatar").cloned();

        RegistrationData {
            username,
            password,
            email,
            full_name,
            avatar,
        }
    };

    validate_username(&data.username)?;
    validate_password(&data.password)?;

    if state.config.read().await.auth.email_required && data.email.is_none() {
        return Err(RegistrationError::EmailRequired);
    }

    if let Some(ref email_str) = data.email {
        if !is_valid_email(email_str) {
            return Err(RegistrationError::InvalidEmail);
        }
    }

    Ok(data)
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

/// Create a session for a newly registered user (for auto-login)
async fn create_session_for_new_user(user_id: i64, token: &str, state: &AppState) -> Result<i64> {
    use crate::database::login as db_login;

    let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;
    let expires_at = crate::database::utils::calculate_expiry(token_expiry_secs as i64);

    db_login::create_session(
        &state.db,
        db_login::NewSession {
            user_id,
            session_token: token.to_string(),
            expires_at,
            ip_address: None,
            user_agent: None,
        },
    )
    .await
    .context("Failed to create session for new user")
}
