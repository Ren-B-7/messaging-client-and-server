use hyper::{body::Incoming, Request, Response, StatusCode};
use http_body_util::Full;
use bytes::Bytes;
use tracing::{info, error};

use crate::AppState;

/// Example: Login form handler
pub async fn handle_login(
    req: Request<Incoming>,
    state: AppState,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    info!("Processing login request");
    
    // Access config values
    let token_expiry = state.config.auth.token_expiry_minutes;
    let email_required = state.config.auth.email_required;
    
    // Parse request body
    // let body = req.collect().await?.to_bytes();
    
    // Your login logic here...
    
    if email_required {
        info!("Email validation required for login");
        // Validate email field
    }
    
    // Create token with configured expiry
    info!("Creating token with {} minute expiry", token_expiry);
    
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Full::new(Bytes::from("Login successful")))
        .unwrap())
}

/// Example: Registration form handler
pub async fn handle_register(
    req: Request<Incoming>,
    state: AppState,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    info!("Processing registration request");
    
    // Access config
    let email_required = state.config.auth.email_required;
    
    if email_required {
        // Validate email presence and format
        info!("Email is required for registration");
    }
    
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Full::new(Bytes::from("Registration successful")))
        .unwrap())
}

/// Example: Profile update handler
pub async fn handle_profile_update(
    req: Request<Incoming>,
    state: AppState,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    info!("Processing profile update");
    
    // Access icon directory if configured
    if let Some(icon_dir) = &state.config.paths.icons {
        info!("Icon directory: {}", icon_dir);
        // Handle icon upload to configured directory
    }
    
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Full::new(Bytes::from("Profile updated")))
        .unwrap())
}

/// Example: Settings handler
pub async fn handle_settings(
    req: Request<Incoming>,
    state: AppState,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    info!("Processing settings request");
    
    // Build settings response with config values
    let settings_info = format!(
        r#"{{
            "email_required": {},
            "token_expiry_minutes": {},
            "max_connections": {}
        }}"#,
        state.config.auth.email_required,
        state.config.auth.token_expiry_minutes,
        state.config.server.max_connections
    );
    
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(settings_info)))
        .unwrap())
}
