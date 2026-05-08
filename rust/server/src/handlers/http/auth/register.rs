use anyhow;
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use tracing::{error, info, warn};

use crate::AppState;
use crate::database::utils::{calculate_expiry, generate_uuid_token, hash_password};
use crate::database::{login, register};
use crate::handlers::http::utils::{
    create_session_cookie, deliver_redirect_with_cookie, deliver_serialized_json,
    deliver_serialized_json_with_cookie, encode_jwt, get_client_ip, get_user_agent, is_https,
};

use shared::types::jwt::JwtClaims;
use shared::types::login::*;
use shared::types::register::*;
use shared::types::user::*;

/// POST /api/register
pub async fn handle_register_api(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing api registration request");

    match register_internal(req, state).await {
        Ok((jwt, user_id, secure_cookie)) => {
            info!("API User registered successfully: ID {}", user_id);
            let cookie = create_session_cookie("auth_id", &jwt, secure_cookie)?;

            Ok(deliver_serialized_json_with_cookie(
                &serde_json::json!({ "status": "success", "user_id": user_id }),
                StatusCode::CREATED,
                cookie,
            )?)
        }
        Err(e) => deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST),
    }
}

/// POST /register
pub async fn handle_register(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing web registration request");

    match register_internal(req, state).await {
        Ok((jwt, user_id, secure_cookie)) => {
            info!("User registered successfully: ID {}", user_id);
            let cookie = create_session_cookie("auth_id", &jwt, secure_cookie)?;
            Ok(deliver_redirect_with_cookie("/chat", cookie)?)
        }
        Err(e) => deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST),
    }
}

async fn register_internal(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> anyhow::Result<(String, i64, bool), RegisterError> {
    info!("Processing registration request");

    let ip_address = get_client_ip(&req);
    let user_agent = get_user_agent(&req).unwrap_or_default();
    let secure_cookie = is_https(&req);

    let registration_data = match parse_and_validate_registration(req, &state).await {
        Ok(data) => data,
        Err(e) => {
            warn!("Register failed: {:?}", e.to_code());
            Err(e)?
        }
    };

    let user_id = match create_user(&registration_data, &state).await {
        Ok(id) => id,
        Err(e) => {
            warn!("User creation failed: {:?}", e.to_code());
            Err(e)?
        }
    };

    // The first registered user is auto-promoted to admin (see db::register).
    let row: (i64,) = sqlx::query_as("SELECT is_admin FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));
    let is_admin = row.0 != 0;

    let session_id = generate_uuid_token();
    let session_created =
        create_session_for_new_user(user_id, &session_id, &state, ip_address).await;

    if let Err(e) = session_created {
        error!(
            "Failed to create session after registration: {}",
            e.to_code()
        );
        Err(RegisterError::DatabaseError)?
    }

    let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = JwtClaims {
        sub: registration_data.username.clone(),
        user_id,
        session_id,
        user_agent,
        is_admin,
        exp: now + token_expiry_secs as usize,
        iat: now,
    };

    let jwt = match encode_jwt(&claims, &state.jwt_secret) {
        Ok(t) => t,
        Err(e) => {
            error!("JWT encoding failed after registration: {}", e);
            Err(RegisterError::DatabaseError)?
        }
    };

    Ok((jwt, user_id, secure_cookie))
}

// ---------------------------------------------------------------------------
// Parsing / validation
// ---------------------------------------------------------------------------

async fn parse_and_validate_registration(
    req: Request<hyper::body::Incoming>,
    state: &AppState,
) -> anyhow::Result<RegisterData, RegisterError> {
    let body = req
        .collect()
        .await
        .map_err(|_| RegisterError::InternalError)?
        .to_bytes();

    let data = serde_json::from_slice::<RegisterData>(&body).map_err(|e| {
        error!("Failed to parse registration JSON: {}", e);
        RegisterError::InternalError
    })?;

    validate_registration(&data)?;
    check_username_available(&data.username, state).await?;
    if let Some(ref email) = data.email {
        check_email_available(email, state).await?;
    }

    Ok(data)
}

fn validate_registration(data: &RegisterData) -> anyhow::Result<(), RegisterError> {
    validate_username(&data.username)?;
    validate_password(&data.password)?;

    if data.password != data.confirm_password {
        return Err(RegisterError::PasswordMismatch);
    }

    if let Some(ref email) = data.email
        && !is_valid_email(email)
    {
        return Err(RegisterError::InvalidEmail);
    }

    Ok(())
}

pub fn validate_username(username: &str) -> anyhow::Result<(), RegisterError> {
    if username.len() < 3 || username.len() > 32 {
        return Err(RegisterError::InvalidUsername);
    }
    if !username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(RegisterError::InvalidUsername);
    }
    Ok(())
}

pub fn validate_password(password: &str) -> anyhow::Result<(), RegisterError> {
    if password.len() < 8 || password.len() > 128 {
        return Err(RegisterError::WeakPassword);
    }
    if !password.chars().any(|c| c.is_alphabetic()) || !password.chars().any(|c| c.is_numeric()) {
        return Err(RegisterError::WeakPassword);
    }
    Ok(())
}

pub fn is_valid_email(email: &str) -> bool {
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 {
        return false;
    }
    let local = parts[0];
    let domain = parts[1];
    !local.is_empty() && domain.contains('.') && !domain.contains('@')
}

async fn check_username_available(
    username: &str,
    state: &AppState,
) -> anyhow::Result<(), RegisterError> {
    let exists = register::username_exists(&state.db, username.to_string())
        .await
        .map_err(|e| {
            error!("DB error checking username: {}", e);
            RegisterError::DatabaseError
        })?;
    if exists {
        return Err(RegisterError::UsernameTaken);
    }
    Ok(())
}

async fn check_email_available(email: &str, state: &AppState) -> anyhow::Result<(), RegisterError> {
    let exists = register::email_exists(&state.db, email.to_string())
        .await
        .map_err(|e| {
            error!("DB error checking email: {}", e);
            RegisterError::DatabaseError
        })?;
    if exists {
        return Err(RegisterError::EmailTaken);
    }
    Ok(())
}

async fn create_user(data: &RegisterData, state: &AppState) -> anyhow::Result<i64, RegisterError> {
    let password_hash = hash_password(&data.password).map_err(|e| {
        error!("Password hashing failed: {}", e);
        RegisterError::InternalError
    })?;

    register::register_user(
        &state.db,
        NewUser {
            username: data.username.clone(),
            password_hash,
            email: data.email.clone(),
            name: None,
        },
    )
    .await
    .map_err(|e| {
        error!("Failed to register user: {}", e);
        RegisterError::DatabaseError
    })
}

async fn create_session_for_new_user(
    user_id: i64,
    session_id: &str,
    state: &AppState,
    ip_address: Option<String>,
) -> anyhow::Result<(), RegisterError> {
    let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;
    let expires_at = calculate_expiry(token_expiry_secs as i64);

    login::create_session(
        &state.db,
        NewSession {
            user_id,
            session_id: session_id.to_string(),
            expires_at,
            ip_address,
        },
    )
    .await
    .map_err(|e| {
        error!("Failed to create session: {}", e);
        RegisterError::DatabaseError
    })?;

    Ok(())
}
