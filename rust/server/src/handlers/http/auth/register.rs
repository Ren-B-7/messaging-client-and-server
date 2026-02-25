use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::combinators::BoxBody;
use hyper::{Request, Response, StatusCode};
use std::collections::HashMap;
use std::convert::Infallible;
use tracing::{error, info, warn};

use crate::AppState;
use crate::database::register as db_register;
use crate::handlers::http::utils::{
    create_session_cookie, deliver_redirect_with_cookie, deliver_serialized_json, encode_jwt,
    get_client_ip, get_user_agent, is_https,
};

use shared::types::jwt::JwtClaims;
use shared::types::login::*;
use shared::types::register::*;

/// Main registration handler.
pub async fn handle_register(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing registration request");

    // Extract IP and UA *before* consuming the body.
    let ip_address = get_client_ip(&req);
    let user_agent = get_user_agent(&req).unwrap_or_default();
    let secure_cookie = is_https(&req);

    let registration_data = match parse_and_validate_registration(req, &state).await {
        Ok(data) => data,
        Err(e) => {
            warn!("Register failed: {:?}", e.to_code());
            return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
        }
    };

    // Create user in DB.
    let user_id = match create_user(&registration_data, &state).await {
        Ok(id) => id,
        Err(e) => {
            warn!("User creation failed: {:?}", e.to_code());
            return deliver_serialized_json(&e.to_response(), StatusCode::BAD_REQUEST);
        }
    };

    // The first registered user is auto-promoted to admin (see db::register).
    // Re-fetch the flag so the JWT carries the correct value.
    let is_admin = state
        .db
        .call(move |conn| {
            let v: i64 = conn
                .query_row("SELECT is_admin FROM users WHERE id = ?1", [user_id], |r| {
                    r.get(0)
                })
                .unwrap_or(0);
            Ok::<_, tokio_rusqlite::rusqlite::Error>(v != 0)
        })
        .await
        .unwrap_or(false);

    // Create the session row and mint a JWT.
    let session_id = crate::database::utils::generate_uuid_token();
    let session_created = create_session_for_new_user(
        user_id,
        &session_id,
        &state,
        ip_address,
    )
    .await;

    if let Err(e) = session_created {
        error!(
            "Failed to create session after registration: {}",
            e.to_code()
        );
        return deliver_serialized_json(
            &RegisterError::DatabaseError.to_response(),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
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
            return deliver_serialized_json(
                &RegisterError::DatabaseError.to_response(),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    info!("User registered successfully: ID {}", user_id);

    let instance_cookie = create_session_cookie("auth_id", &jwt, secure_cookie)
        .context("Failed to create session cookie")?;

    Ok(deliver_redirect_with_cookie("/chat", Some(instance_cookie))?)
}

// ---------------------------------------------------------------------------
// Parsing / validation
// ---------------------------------------------------------------------------

/// Parse and validate the registration body.
/// Consumes `req` — IP/UA must be extracted before calling this.
async fn parse_and_validate_registration(
    req: Request<hyper::body::Incoming>,
    state: &AppState,
) -> std::result::Result<RegisterData, RegisterError> {
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let data = if content_type.contains("application/json") {
        parse_registration_json(req).await?
    } else {
        parse_registration_form(req).await?
    };

    validate_registration(&data)?;
    check_username_available(&data.username, state).await?;
    if let Some(ref email) = data.email {
        check_email_available(email, state).await?;
    }

    Ok(data)
}

async fn parse_registration_json(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<RegisterData, RegisterError> {
    let body = req
        .collect()
        .await
        .map_err(|_| RegisterError::InternalError)?
        .to_bytes();

    serde_json::from_slice::<RegisterData>(&body).map_err(|e| {
        error!("Failed to parse registration JSON: {}", e);
        RegisterError::InternalError
    })
}

async fn parse_registration_form(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<RegisterData, RegisterError> {
    let body = req
        .collect()
        .await
        .map_err(|_| RegisterError::InternalError)?
        .to_bytes();

    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    let username = params
        .get("username")
        .ok_or(RegisterError::MissingField("username".to_string()))?
        .trim()
        .to_string();

    let password = params
        .get("password")
        .ok_or(RegisterError::MissingField("password".to_string()))?
        .to_string();

    let confirm_password = params
        .get("confirm_password")
        .or_else(|| params.get("password_confirm"))
        .ok_or(RegisterError::MissingField("confirm_password".to_string()))?
        .to_string();

    let email = params
        .get("email")
        .filter(|e| !e.is_empty())
        .map(|e| e.trim().to_string());

    let full_name = params
        .get("full_name")
        .or_else(|| params.get("fullName"))
        .filter(|n| !n.is_empty())
        .map(|n| n.trim().to_string());

    Ok(RegisterData {
        username,
        password,
        confirm_password,
        email,
        full_name,
    })
}

fn validate_registration(data: &RegisterData) -> std::result::Result<(), RegisterError> {
    validate_username(&data.username)?;
    validate_password(&data.password)?;

    if data.password != data.confirm_password {
        return Err(RegisterError::PasswordMismatch);
    }

    if let Some(ref email) = data.email {
        if !is_valid_email(email) {
            return Err(RegisterError::InvalidEmail);
        }
    }

    Ok(())
}

pub(crate) fn validate_username(username: &str) -> std::result::Result<(), RegisterError> {
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

pub(crate) fn validate_password(password: &str) -> std::result::Result<(), RegisterError> {
    if password.len() < 8 || password.len() > 128 {
        return Err(RegisterError::WeakPassword);
    }
    if !password.chars().any(|c| c.is_alphabetic())
        || !password.chars().any(|c| c.is_numeric())
    {
        return Err(RegisterError::WeakPassword);
    }
    Ok(())
}

pub(crate) fn is_valid_email(email: &str) -> bool {
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
) -> std::result::Result<(), RegisterError> {
    let exists = db_register::username_exists(&state.db, username.to_string())
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

async fn check_email_available(
    email: &str,
    state: &AppState,
) -> std::result::Result<(), RegisterError> {
    let exists = db_register::email_exists(&state.db, email.to_string())
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

async fn create_user(
    data: &RegisterData,
    state: &AppState,
) -> std::result::Result<i64, RegisterError> {
    let password_hash =
        crate::database::utils::hash_password(&data.password).map_err(|e| {
            error!("Password hashing failed: {}", e);
            RegisterError::InternalError
        })?;

    db_register::register_user(
        &state.db,
        db_register::NewUser {
            username: data.username.clone(),
            password_hash,
            email: data.email.clone(),
        },
    )
    .await
    .map_err(|e| {
        error!("Failed to register user: {}", e);
        RegisterError::DatabaseError
    })
}

/// Create a session row for a newly registered user.
/// `session_id` is the UUID that will be embedded in the JWT.
async fn create_session_for_new_user(
    user_id: i64,
    session_id: &str,
    state: &AppState,
    ip_address: Option<String>,
) -> std::result::Result<(), RegisterError> {
    use crate::database::login as db_login;

    let token_expiry_secs = state.config.read().await.auth.token_expiry_minutes * 60;
    let expires_at = crate::database::utils::calculate_expiry(token_expiry_secs as i64);

    db_login::create_session(
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

// ---------------------------------------------------------------------------
// Tests  (unchanged from original)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_username_passes() {
        assert!(validate_username("alice_123").is_ok());
        assert!(validate_username("Bob-Smith").is_ok());
        assert!(validate_username("abc").is_ok());
    }

    #[test]
    fn username_too_short_fails() {
        assert!(validate_username("ab").is_err());
        assert!(validate_username("").is_err());
    }

    #[test]
    fn username_too_long_fails() {
        let long: String = "a".repeat(33);
        assert!(validate_username(&long).is_err());
    }

    #[test]
    fn username_invalid_chars_fails() {
        assert!(validate_username("alice!").is_err());
        assert!(validate_username("bob@mail").is_err());
        assert!(validate_username("eve space").is_err());
    }

    #[test]
    fn username_max_length_passes() {
        let max: String = "a".repeat(32);
        assert!(validate_username(&max).is_ok());
    }

    #[test]
    fn valid_password_passes() {
        assert!(validate_password("Password1").is_ok());
        assert!(validate_password("abc12345").is_ok());
    }

    #[test]
    fn password_too_short_fails() {
        assert!(validate_password("Abc1").is_err());
        assert!(validate_password("").is_err());
    }

    #[test]
    fn password_no_digit_fails() {
        assert!(validate_password("onlyletters").is_err());
    }

    #[test]
    fn password_no_letter_fails() {
        assert!(validate_password("12345678").is_err());
    }

    #[test]
    fn valid_email_passes() {
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("a.b+tag@sub.domain.org"));
    }

    #[test]
    fn email_missing_at_fails() {
        assert!(!is_valid_email("notanemail.com"));
    }

    #[test]
    fn email_empty_local_part_fails() {
        assert!(!is_valid_email("@example.com"));
    }

    #[test]
    fn email_multiple_at_signs_fails() {
        assert!(!is_valid_email("a@b@c.com"));
    }
}
