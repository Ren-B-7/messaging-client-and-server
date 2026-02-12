use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::Request;
use std::collections::HashMap;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// User registration data
#[derive(Debug, Clone)]
pub struct RegistrationData {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
}

/// Registration result
#[derive(Debug)]
pub enum RegistrationResult {
    Success { user_id: i64, username: String },
    UsernameTaken,
    EmailTaken,
    InvalidInput(String),
    DatabaseError(String),
}

/// Parse registration form data from request
pub async fn parse_registration_form(
    req: Request<hyper::body::Incoming>,
) -> Result<RegistrationData> {
    // Concatenate the body
    let body = req.collect().await?.to_bytes();
    
    // Parse the request body using form_urlencoded
    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    // Extract username
    let username = params
        .get("username")
        .ok_or("Missing username field")?
        .clone();

    // Extract password
    let password = params
        .get("password")
        .ok_or("Missing password field")?
        .clone();

    // Extract optional email
    let email = params.get("email").cloned();

    // Extract optional password confirmation
    if let Some(confirm) = params.get("password_confirm") {
        if password != *confirm {
            return Err("Passwords do not match".into());
        }
    }

    Ok(RegistrationData {
        username,
        password,
        email,
    })
}

/// Validate registration data
pub fn validate_registration(data: &RegistrationData) -> std::result::Result<(), String> {
    // Validate username
    if data.username.is_empty() {
        return Err("Username cannot be empty".to_string());
    }

    if data.username.len() < 3 {
        return Err("Username must be at least 3 characters".to_string());
    }

    if data.username.len() > 32 {
        return Err("Username must be at most 32 characters".to_string());
    }

    if !data
        .username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(
            "Username can only contain letters, numbers, underscores, and hyphens".to_string(),
        );
    }

    // Validate password
    if data.password.is_empty() {
        return Err("Password cannot be empty".to_string());
    }

    if data.password.len() < 8 {
        return Err("Password must be at least 8 characters".to_string());
    }

    if data.password.len() > 128 {
        return Err("Password must be at most 128 characters".to_string());
    }

    if !data.password.chars().any(|c| c.is_numeric()) {
        return Err("Password must contain at least one number".to_string());
    }

    if !data.password.chars().any(|c| c.is_alphabetic()) {
        return Err("Password must contain at least one letter".to_string());
    }

    // Validate email if provided
    if let Some(ref email) = data.email {
        if !email.is_empty() && !is_valid_email(email) {
            return Err("Invalid email format".to_string());
        }
    }

    Ok(())
}

/// Register a new user
/// This is a placeholder - implement with your actual database
pub async fn register_user(data: &RegistrationData) -> Result<RegistrationResult> {
    // Validate the registration data first
    if let Err(e) = validate_registration(data) {
        return Ok(RegistrationResult::InvalidInput(e));
    }

    // TODO: Implement actual database insertion
    // Example pseudo-code:
    //
    // // Check if username exists
    // if database::username_exists(&data.username).await? {
    //     return Ok(RegistrationResult::UsernameTaken);
    // }
    //
    // // Check if email exists (if provided)
    // if let Some(ref email) = data.email {
    //     if database::email_exists(email).await? {
    //         return Ok(RegistrationResult::EmailTaken);
    //     }
    // }
    //
    // // Hash the password
    // let password_hash = hash_password(&data.password)?;
    //
    // // Insert into database
    // let user_id = database::create_user(
    //     &data.username,
    //     &password_hash,
    //     data.email.as_deref(),
    // ).await?;
    //
    // Ok(RegistrationResult::Success {
    //     user_id,
    //     username: data.username.clone(),
    // })

    // Placeholder implementation
    if data.username == "admin" {
        Ok(RegistrationResult::UsernameTaken)
    } else {
        Ok(RegistrationResult::Success {
            user_id: 1,
            username: data.username.clone(),
        })
    }
}

/// Simple email validation
fn is_valid_email(email: &str) -> bool {
    // Basic email validation - just check for @ and .
    // For production, use a proper email validation library
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

/// Hash a password securely
/// TODO: Replace with proper password hashing (e.g., argon2, bcrypt)
pub fn hash_password(password: &str) -> String {
    // Placeholder - DO NOT USE IN PRODUCTION
    // Use argon2, bcrypt, or scrypt in real implementation
    format!("hashed_{}", password)
}

/// Generate a verification token for email confirmation
pub fn generate_verification_token(user_id: i64) -> String {
    // TODO: Implement secure token generation
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    format!("verify_{}_{}", user_id, timestamp)
}
