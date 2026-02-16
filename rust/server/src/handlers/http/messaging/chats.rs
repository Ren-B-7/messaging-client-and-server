use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming as IncomingBody;
use hyper::{Request, Response, StatusCode};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::info;

use crate::AppState;
use crate::handlers::http::utils::deliver_error_json;

/// Chat creation request
#[derive(Debug, Deserialize)]
pub struct CreateChatRequest {
    pub name: Option<String>,
    pub participants: Vec<i64>,
}

/// Get all chats for authenticated user
pub async fn handle_get_chats(
    _req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    info!("Fetching chats for user");

    // TODO: Fetch actual chats from database
    let chats_json = serde_json::json!({
        "status": "success",
        "data": {
            "chats": [
                {
                    "id": 1,
                    "name": "General",
                    "type": "group",
                    "last_message": "Hello everyone!",
                    "last_message_at": "2024-02-14T12:30:00Z",
                    "unread_count": 3
                },
                {
                    "id": 2,
                    "name": "Alice",
                    "type": "direct",
                    "last_message": "See you tomorrow",
                    "last_message_at": "2024-02-14T11:15:00Z",
                    "unread_count": 0
                }
            ]
        }
    });

    let json_string: String = chats_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build chats response")?;

    Ok(response)
}

/// Create a new chat
pub async fn handle_create_chat(
    req: Request<IncomingBody>,
    _state: AppState,
) -> Result<Response<Full<Bytes>>> {
    info!("Creating new chat");

    // Parse request body
    let collected_body = req.collect().await.context("Failed to read request body")?;

    let body: Bytes = collected_body.to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let name: Option<String> = params.get("name").map(|s| s.to_string());
    let participants_str: &String = params
        .get("participants")
        .ok_or_else(|| anyhow::anyhow!("Missing participants"))?;

    // Parse comma-separated participant IDs
    let participants: Vec<i64> = participants_str
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if participants.is_empty() {
        return deliver_error_json(
            "INVALID_INPUT",
            "At least one participant required",
            StatusCode::BAD_REQUEST,
        );
    }

    // TODO: Create chat in database
    let chat_id: i64 = 123; // Placeholder

    let response_json = serde_json::json!({
        "status": "success",
        "message": "Chat created successfully",
        "data": {
            "chat_id": chat_id,
            "name": name,
            "participants": participants
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

/// Get messages for a specific chat
pub async fn handle_get_messages(
    _req: Request<IncomingBody>,
    _state: AppState,
    chat_id: i64,
) -> Result<Response<Full<Bytes>>> {
    info!("Fetching messages for chat {}", chat_id);

    // TODO: Fetch actual messages from database
    let messages_json = serde_json::json!({
        "status": "success",
        "data": {
            "chat_id": chat_id,
            "messages": [
                {
                    "id": 1,
                    "user_id": 1,
                    "username": "Alice",
                    "content": "Hello!",
                    "created_at": "2024-02-14T12:00:00Z"
                },
                {
                    "id": 2,
                    "user_id": 2,
                    "username": "Bob",
                    "content": "Hi there!",
                    "created_at": "2024-02-14T12:01:00Z"
                }
            ]
        }
    });

    let json_string: String = messages_json.to_string();
    let json_bytes: Bytes = Bytes::from(json_string);

    let response: Response<Full<Bytes>> = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(json_bytes))
        .context("Failed to build messages response")?;

    Ok(response)
}

/// Send a message to a chat
pub async fn handle_send_message(
    req: Request<IncomingBody>,
    _state: AppState,
    chat_id: i64,
) -> Result<Response<Full<Bytes>>> {
    info!("Sending message to chat {}", chat_id);

    // Parse request body
    let collected_body = req.collect().await.context("Failed to read request body")?;

    let body: Bytes = collected_body.to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let content: &String = params
        .get("content")
        .ok_or_else(|| anyhow::anyhow!("Missing message content"))?;

    if content.trim().is_empty() {
        return deliver_error_json(
            "INVALID_INPUT",
            "Message content cannot be empty",
            StatusCode::BAD_REQUEST,
        );
    }

    // TODO: Save message to database
    let message_id: i64 = 456; // Placeholder

    let response_json = serde_json::json!({
        "status": "success",
        "message": "Message sent successfully",
        "data": {
            "message_id": message_id,
            "chat_id": chat_id,
            "content": content
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
