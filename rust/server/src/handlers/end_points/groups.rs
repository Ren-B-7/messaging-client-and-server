use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use std::collections::HashMap;
use tracing::info;

use crate::AppState;
use crate::handlers::utils::error_response::ErrorResponse;

/// Get all groups for authenticated user
pub async fn handle_get_groups(
    _req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    info!("Fetching groups for user");

    // TODO: Fetch actual groups from database
    let groups_json = serde_json::json!({
        "status": "success",
        "data": {
            "groups": [
                {
                    "id": 1,
                    "name": "Project Team",
                    "description": "Main project discussion",
                    "member_count": 5,
                    "created_at": "2024-01-01T00:00:00Z"
                },
                {
                    "id": 2,
                    "name": "Friends",
                    "description": "Casual hangout group",
                    "member_count": 8,
                    "created_at": "2024-01-15T00:00:00Z"
                }
            ]
        }
    });

    let json_string: String = groups_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build groups response")?;

    Ok(response)
}

/// Create a new group
pub async fn handle_create_group(
    req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    info!("Creating new group");

    // Parse request body
    let collected_body = req.collect().await.context("Failed to read request body")?;

    let body: Bytes = collected_body.to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let name: &String = params
        .get("name")
        .ok_or_else(|| anyhow::anyhow!("Missing group name"))?;

    let description: Option<String> = params.get("description").map(|s| s.to_string());

    if name.trim().is_empty() {
        return deliver_error_json(
            "INVALID_INPUT",
            "Group name cannot be empty",
            StatusCode::BAD_REQUEST,
        );
    }

    // TODO: Create group in database
    let group_id: i64 = 123; // Placeholder

    let response_json = serde_json::json!({
        "status": "success",
        "message": "Group created successfully",
        "data": {
            "group_id": group_id,
            "name": name,
            "description": description
        }
    });

    let json_string: String = response_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::CREATED)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build response")?;

    Ok(response)
}

/// Get group members
pub async fn handle_get_members(
    _req: Request<IncomingBody>,
    _state: AppState,
    group_id: i64,
) -> Result<Response<Full<Bytes>>> {
    info!("Fetching members for group {}", group_id);

    // TODO: Fetch actual members from database
    let members_json = serde_json::json!({
        "status": "success",
        "data": {
            "group_id": group_id,
            "members": [
                {
                    "user_id": 1,
                    "username": "Alice",
                    "role": "admin",
                    "joined_at": "2024-01-01T00:00:00Z"
                },
                {
                    "user_id": 2,
                    "username": "Bob",
                    "role": "member",
                    "joined_at": "2024-01-02T00:00:00Z"
                }
            ]
        }
    });

    let json_string: String = members_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build members response")?;

    Ok(response)
}

/// Add member to group
pub async fn handle_add_member(
    req: Request<IncomingBody>,
    _state: AppState,
    group_id: i64,
) -> Result<Response<Full<Bytes>>> {
    info!("Adding member to group {}", group_id);

    // Parse request body
    let collected_body = req.collect().await.context("Failed to read request body")?;

    let body: Bytes = collected_body.to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid user_id"))?;

    // TODO: Add member to group in database

    let response_json = serde_json::json!({
        "status": "success",
        "message": "Member added successfully",
        "data": {
            "group_id": group_id,
            "user_id": user_id
        }
    });

    let json_string: String = response_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build response")?;

    Ok(response)
}

/// Remove member from group
pub async fn handle_remove_member(
    req: Request<IncomingBody>,
    _state: AppState,
    group_id: i64,
) -> Result<Response<Full<Bytes>>> {
    info!("Removing member from group {}", group_id);

    // Parse request body
    let collected_body = req.collect().await.context("Failed to read request body")?;

    let body: Bytes = collected_body.to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid user_id"))?;

    // TODO: Remove member from group in database

    let response_json = serde_json::json!({
        "status": "success",
        "message": "Member removed successfully",
        "data": {
            "group_id": group_id,
            "user_id": user_id
        }
    });

    let json_string: String = response_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build response")?;

    Ok(response)
}

/// Deliver JSON error response
fn deliver_error_json(
    code: &str,
    message: &str,
    status: StatusCode,
) -> Result<Response<Full<Bytes>>> {
    let error: ErrorResponse = ErrorResponse::new(code, message);
    let json: String = serde_json::to_string(&error).unwrap_or_else(|_| {
        r#"{"status":"error","code":"INTERNAL_ERROR","message":"Failed to serialize error"}"#
            .to_string()
    });

    let json_bytes: Bytes = Bytes::from(json);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build error response")?;

    Ok(response)
}
