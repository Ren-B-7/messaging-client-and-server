use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use tracing::info;

use crate::AppState;
use crate::handlers::http::utils::deliver_error_json;

/// Handle get profile (requires authentication)
pub async fn handle_get_profile(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing get profile request");

    // Extract user_id from session
    let user_id = match extract_user_from_request(&req, &state).await {
        Ok(id) => id,
        Err(_err) => {
            return deliver_error_json(
                "UNAUTHORIZED",
                "Authentication required",
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    // Get user profile from database
    use crate::database::register as db_register;

    let user = db_register::get_user_by_id(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("User not found"))?;

    let profile_json = serde_json::json!({
        "status": "success",
        "data": {
            "user_id": user.id,
            "username": user.username,
            "email": user.email,
            "created_at": user.created_at
        }
    });

    let json_bytes = Bytes::from(profile_json.to_string());

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes).boxed())
        .context("Failed to build profile response")?)
}

async fn extract_user_from_request(req: &Request<IncomingBody>, state: &AppState) -> Result<i64> {
    use crate::database::login as db_login;

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
        .ok_or_else(|| anyhow::anyhow!("No auth token"))?;

    let user_id = db_login::validate_session(&state.db, token.to_string())
        .await
        .map_err(|_| anyhow::anyhow!("Invalid session"))?
        .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

    Ok(user_id)
}
