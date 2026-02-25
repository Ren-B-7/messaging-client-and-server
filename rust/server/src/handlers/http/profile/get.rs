use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use tracing::info;

use crate::AppState;
use crate::handlers::http::utils::{decode_jwt_claims, deliver_error_json};

/// Handle GET /api/profile — fast JWT path, zero DB reads for auth.
pub async fn handle_get_profile(
    req: Request<IncomingBody>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing get profile request");

    // FAST PATH — decode JWT signature only, no DB read needed.
    let claims = match decode_jwt_claims(&req, &state.jwt_secret) {
        Ok(c) => c,
        Err(_) => {
            return deliver_error_json(
                "UNAUTHORIZED",
                "Authentication required",
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    // Fetch the full profile from DB (we still need email, created_at etc.)
    use crate::database::register as db_register;

    let user = db_register::get_user_by_id(&state.db, claims.user_id)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("User not found"))?;

    let profile_json = serde_json::json!({
        "status": "success",
        "data": {
            "user_id":  user.id,
            "username": user.username,
            "email":    user.email,
            "is_admin": claims.is_admin,
            "created_at": user.created_at,
        }
    });

    let json_bytes = Bytes::from(profile_json.to_string());

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes).boxed())
        .context("Failed to build profile response")?)
}
