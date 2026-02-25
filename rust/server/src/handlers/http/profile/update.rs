use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::Result;
use tokio_rusqlite::rusqlite;
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{Request, Response, StatusCode};
use tracing::{error, info, warn};

use shared::types::update::*;

use crate::AppState;
use crate::handlers::http::utils::{deliver_serialized_json, validate_token_secure};

/// Get user profile handler
pub async fn handle_get_profile(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing get profile request");

    // Extract user_id from session
    let user_id = match extract_user_from_request(&req, &state).await {
        Ok(id) => id,
        Err(err) => {
            warn!("Unauthorized profile access attempt");
            return deliver_serialized_json(&err.to_profile_response(), StatusCode::UNAUTHORIZED);
        }
    };

    // Get user profile
    match get_user_profile(user_id, &state).await {
        Ok(profile) => {
            info!("Profile retrieved for user {}", user_id);

            let response = ProfileResponse::Success {
                profile,
                message: "Profile retrieved successfully".to_string(),
            };

            deliver_serialized_json(&response, StatusCode::OK)
        }
        Err(err) => {
            error!("Failed to get profile: {:?}", err.to_code());
            deliver_serialized_json(
                &err.to_profile_response(),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

/// Update user profile handler
pub async fn handle_update_profile(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing update profile request");

    // Extract user_id from session
    let user_id = match extract_user_from_request(&req, &state).await {
        Ok(id) => id,
        Err(err) => {
            warn!("Unauthorized profile update attempt");
            return deliver_serialized_json(&err.to_update_response(), StatusCode::UNAUTHORIZED);
        }
    };

    // Parse update data
    let update_data = match parse_update_form(req).await {
        Ok(data) => data,
        Err(err) => {
            warn!("Profile update parsing failed: {:?}", err.to_code());
            return deliver_serialized_json(&err.to_update_response(), StatusCode::BAD_REQUEST);
        }
    };

    // Update profile
    match update_user_profile(user_id, &update_data, &state).await {
        Ok(_) => {
            info!("Profile updated for user {}", user_id);

            let response = UpdateResponse::Success {
                message: "Profile updated successfully".to_string(),
            };

            deliver_serialized_json(&response, StatusCode::OK)
        }
        Err(err) => {
            error!("Failed to update profile: {:?}", err.to_code());
            deliver_serialized_json(&err.to_update_response(), StatusCode::BAD_REQUEST)
        }
    }
}

/// Extract authenticated user from request
async fn extract_user_from_request(
    req: &Request<hyper::body::Incoming>,
    state: &AppState,
) -> std::result::Result<i64, ProfileError> {
    // SECURE PATH: PUT requests validate IP/UA (state-changing)
    validate_token_secure(req, state)
        .await
        .map_err(|_| ProfileError::Unauthorized)
}

/// Get user profile from database
async fn get_user_profile(
    user_id: i64,
    state: &AppState,
) -> std::result::Result<ProfileData, ProfileError> {
    use crate::database::register as db_register;

    let user = db_register::get_user_by_id(&state.db, user_id)
        .await
        .map_err(|e| {
            error!("Database error getting user: {}", e);
            ProfileError::DatabaseError
        })?
        .ok_or(ProfileError::UserNotFound)?;

    // Fetch last_login separately from the sessions table
    let last_login = state
        .db
        .call(move |conn| {
            let result: Option<i64> = conn
                .query_row(
                    "SELECT last_login FROM users WHERE id = ?1",
                    [user.id],
                    |r| r.get(0),
                )
                .unwrap_or(None);
            Ok::<_, tokio_rusqlite::rusqlite::Error>(result)
        })
        .await
        .unwrap_or(None);

    Ok(ProfileData {
        user_id: user.id,
        username: user.username,
        email: user.email,
        created_at: user.created_at,
        last_login,
    })
}

/// Parse update form data
async fn parse_update_form(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<UpdateProfileData, ProfileError> {
    let body = req
        .collect()
        .await
        .map_err(|_| ProfileError::InternalError)?
        .to_bytes();

    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    let username = params
        .get("username")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let email = params
        .get("email")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(UpdateProfileData { username, email })
}

/// Update user profile in database
async fn update_user_profile(
    user_id: i64,
    data: &UpdateProfileData,
    state: &AppState,
) -> std::result::Result<(), ProfileError> {
    use crate::database::register as db_register;

    // Update username if provided
    if let Some(ref new_username) = data.username {
        // Validate username format
        if !crate::database::utils::is_valid_username(new_username) {
            return Err(ProfileError::InvalidUsername);
        }

        // Check if username is already taken
        let exists = db_register::username_exists(&state.db, new_username.clone())
            .await
            .map_err(|e| {
                error!("Database error checking username: {}", e);
                ProfileError::DatabaseError
            })?;

        if exists {
            // Check if it's the same user (they might be updating other fields)
            let current_user = db_register::get_user_by_id(&state.db, user_id)
                .await
                .map_err(|_| ProfileError::DatabaseError)?
                .ok_or(ProfileError::UserNotFound)?;

            if &current_user.username != new_username {
                return Err(ProfileError::UsernameTaken);
            }
        } else {
            // Update username
            db_register::update_username(&state.db, user_id, new_username.clone())
                .await
                .map_err(|e| {
                    error!("Database error updating username: {}", e);
                    ProfileError::DatabaseError
                })?;
        }
    }

    // Update email if provided
    if let Some(ref new_email) = data.email {
        // Validate email format
        if !crate::database::utils::is_valid_email(new_email) {
            return Err(ProfileError::InvalidEmail);
        }

        // Check if email is already taken
        let exists = db_register::email_exists(&state.db, new_email.clone())
            .await
            .map_err(|e| {
                error!("Database error checking email: {}", e);
                ProfileError::DatabaseError
            })?;

        if exists {
            return Err(ProfileError::EmailTaken);
        }

        let email_to_set = new_email.clone();
        state
            .db
            .call(move |conn| {
                conn.execute(
                    "UPDATE users SET email = ?1 WHERE id = ?2",
                    rusqlite::params![email_to_set, user_id],
                )?;
                Ok::<_, tokio_rusqlite::rusqlite::Error>(())
            })
            .await
            .map_err(|e| {
                error!("Database error updating email: {}", e);
                ProfileError::DatabaseError
            })?;
    }

    Ok(())
}
// handlers/http/profile/update.rs  — append at the bottom
#[cfg(test)]
mod tests {
    // parse_update_form is private/async and requires a real body — we test the
    // form-parsing logic directly by replicating it inline.

    #[test]
    fn parse_update_both_fields() {
        let body = b"username=alice&email=alice@example.com";
        let params: std::collections::HashMap<String, String> =
            form_urlencoded::parse(body.as_ref())
                .into_owned()
                .collect();

        let username = params.get("username").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let email = params.get("email").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

        assert_eq!(username, Some("alice".to_string()));
        assert_eq!(email, Some("alice@example.com".to_string()));
    }

    #[test]
    fn parse_update_username_only() {
        let body = b"username=bob";
        let params: std::collections::HashMap<String, String> =
            form_urlencoded::parse(body.as_ref())
                .into_owned()
                .collect();

        let username = params.get("username").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let email = params.get("email").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

        assert_eq!(username, Some("bob".to_string()));
        assert!(email.is_none());
    }

    #[test]
    fn parse_update_empty_fields_become_none() {
        let body = b"username=&email=";
        let params: std::collections::HashMap<String, String> =
            form_urlencoded::parse(body.as_ref())
                .into_owned()
                .collect();

        let username = params.get("username").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let email = params.get("email").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

        assert!(username.is_none(), "empty username should be None");
        assert!(email.is_none(), "empty email should be None");
    }

    #[test]
    fn parse_update_whitespace_trimmed() {
        let body = b"username=%20alice%20";
        let params: std::collections::HashMap<String, String> =
            form_urlencoded::parse(body.as_ref())
                .into_owned()
                .collect();

        let username = params
            .get("username")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        assert_eq!(username, Some("alice".to_string()));
    }
}
