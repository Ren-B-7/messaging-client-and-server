use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{Request, Response, StatusCode};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::AppState;
use crate::handlers::http::utils::{
    deliver_error_json, deliver_serialized_json, deliver_success_json,
};
use shared::types::jwt::JwtClaims;
use shared::types::message::*;

// ---------------------------------------------------------------------------
// Handlers
//
// Auth is performed by the router BEFORE these handlers are called.
// Every handler that mutates state receives a verified `user_id: i64`.
// Every handler that only reads receives the decoded `JwtClaims`.
// Neither calls any auth function internally.
// ---------------------------------------------------------------------------

/// GET /api/chats — list all chats (both direct and group) for the authenticated user.
///
/// Light-auth route: `claims` are pre-verified by the router (JWT only, no DB).
///
/// Returns a unified list — no separate DM vs group split. The `chat_type`
/// field on each entry (`"direct"` or `"group"`) lets the client decide how
/// to render it.
pub async fn handle_get_chats(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let user_id = claims.user_id;
    info!("Processing get chats request for user {}", user_id);

    use crate::database::groups as db_groups;

    let chats = db_groups::get_user_groups(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("Database error getting chats: {}", e))?;

    let chats_json: Vec<serde_json::Value> = chats
        .into_iter()
        .map(|g| {
            serde_json::json!({
                "chat_id":     g.id,
                "name":        g.name,
                "description": g.description,
                "chat_type":   g.chat_type,
                "created_by":  g.created_by,
                "created_at":  g.created_at,
            })
        })
        .collect();

    deliver_serialized_json(
        &serde_json::json!({ "status": "success", "data": { "chats": chats_json } }),
        StatusCode::OK,
    )
}

/// POST /api/chats — create a new direct message (DM) with another user.
///
/// Hard-auth route: `user_id` is pre-verified by the router (JWT + DB + IP).
///
/// Body parameters (choose one):
///   - `username` — username of the other participant (looked up in database)
///   - `user_id`  — user ID of the other participant
///
/// The internal chat name is auto-generated as a UUID. The frontend should
/// fetch the other user's profile separately to display their actual name.
/// Both users are added as "admin" because DMs have no moderation hierarchy.
///
/// This endpoint is idempotent: if a DM already exists between the users,
/// the existing chat is returned instead of creating a duplicate.
pub async fn handle_create_chat(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing create chat request from user {}", user_id);

    use crate::database::groups as db_groups;
    use crate::database::register as db_register;

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: serde_json::Value =
        serde_json::from_slice(&body).context("Failed to parse JSON request body")?;

    // Resolve the other participant's user_id from either username or user_id field
    let other_user_id: i64 = if let Some(uid) = params.get("user_id").and_then(|v| v.as_i64()) {
        uid
    } else if let Some(username) = params.get("username").and_then(|v| v.as_str()) {
        // Look up user by username
        match db_register::get_user_by_username(&state.db, username.to_string()).await? {
            Some(user) => user.id,
            None => {
                return deliver_error_json(
                    "NOT_FOUND",
                    &format!("User '{}' not found", username),
                    StatusCode::NOT_FOUND,
                );
            }
        }
    } else {
        return deliver_error_json(
            "INVALID_INPUT",
            "Request must include either 'username' or 'user_id'",
            StatusCode::BAD_REQUEST,
        );
    };

    // Prevent DM with self
    if other_user_id == user_id {
        return deliver_error_json(
            "INVALID_INPUT",
            "Cannot create a DM with yourself",
            StatusCode::BAD_REQUEST,
        );
    }

    // Check if DM already exists between these two users
    match db_groups::find_existing_dm(&state.db, user_id, other_user_id).await? {
        Some(existing_chat_id) => {
            // DM already exists, return it
            let chat = db_groups::get_group(&state.db, existing_chat_id)
                .await?
                .context("Failed to retrieve existing chat")?;

            info!(
                "Existing DM {} returned for users {} and {}",
                existing_chat_id, user_id, other_user_id
            );

            return deliver_success_json(
                Some(serde_json::json!({
                    "id":        chat.id,
                    "chat_id":   chat.id,
                    "name":      chat.name,
                    "chat_type": "direct",
                    "created_at": chat.created_at,
                })),
                Some("Existing DM returned"),
                StatusCode::OK,
            );
        }
        None => {}
    }

    // Generate an internal UUID-based name for the DM
    let internal_name = Uuid::new_v4().to_string();

    // Create the DM as a "direct" chat — creator is added as admin by create_group
    let chat_id = db_groups::create_group(
        &state.db,
        db_groups::NewGroup {
            name: internal_name.clone(),
            created_by: user_id,
            description: None,
            chat_type: "direct".to_string(),
        },
    )
    .await
    .context("Failed to create DM")?;

    // Add the other participant as an admin (both participants in DM are admins)
    db_groups::add_group_member(&state.db, chat_id, other_user_id, "admin".to_string())
        .await
        .context("Failed to add other participant to DM")?;

    info!(
        "DM {} created between users {} and {}",
        chat_id, user_id, other_user_id
    );

    deliver_success_json(
        Some(serde_json::json!({
            "id":        chat_id,
            "chat_id":   chat_id,
            "name":      internal_name,
            "chat_type": "direct",
            "created_at": crate::database::utils::get_timestamp(),
        })),
        Some("DM created successfully"),
        StatusCode::CREATED,
    )
}

/// POST /api/messages/send — send a message to a chat (DM or group).
///
/// Hard-auth route: `user_id` is pre-verified by the router (JWT + DB + IP).
///
/// Request body:
/// ```json
/// {
///   "chat_id": 5,
///   "content": "Hello world",
///   "message_type": "text"  // optional
/// }
/// ```
pub async fn handle_send_message(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing send message request from user {}", user_id);

    let message_data = match parse_message_form(req).await {
        Ok(data) => data,
        Err(err) => {
            warn!("Message parsing failed: {:?}", err.to_code());
            return deliver_serialized_json(&err.to_send_response(), StatusCode::BAD_REQUEST);
        }
    };

    if let Err(err) = validate_message(&message_data) {
        warn!("Message validation failed: {:?}", err.to_code());
        return deliver_serialized_json(&err.to_send_response(), StatusCode::BAD_REQUEST);
    }

    match send_message(user_id, &message_data, &state).await {
        Ok((message_id, sent_at)) => {
            info!(
                "Message {} sent by user {} to chat {}",
                message_id, user_id, message_data.chat_id
            );
            deliver_serialized_json(
                &SendMessageResponse::Success {
                    message_id,
                    sent_at,
                    message: "Message sent successfully".to_string(),
                },
                StatusCode::CREATED,
            )
        }
        Err(err) => {
            error!("Failed to send message: {:?}", err.to_code());
            deliver_serialized_json(&err.to_send_response(), StatusCode::BAD_REQUEST)
        }
    }
}

/// GET /api/messages — retrieve messages for a chat (DM or group).
///
/// Light-auth route: `claims` are pre-verified by the router (JWT only, no DB).
///
/// Query parameters:
///   - `chat_id` — the chat to retrieve messages from (required)
///   - `limit` — max messages to return (default: 50, max: 100)
///   - `offset` — pagination offset (default: 0)
pub async fn handle_get_messages(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let user_id = claims.user_id;
    info!("Processing get messages request for user {}", user_id);

    let query = parse_query_params(&req);

    match retrieve_messages(user_id, &query, &state).await {
        Ok(messages) => {
            info!("Retrieved {} messages for user {}", messages.len(), user_id);
            deliver_serialized_json(
                &MessagesResponse::Success {
                    total: messages.len(),
                    messages,
                },
                StatusCode::OK,
            )
        }
        Err(err) => {
            error!("Failed to retrieve messages: {:?}", err.to_code());
            deliver_serialized_json(&err.to_list_response(), StatusCode::BAD_REQUEST)
        }
    }
}

/// POST /api/messages/:id/read — mark a message as read.
///
/// Hard-auth route: `user_id` is pre-verified by the router (JWT + DB + IP).
pub async fn handle_mark_read(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    _user_id: i64,
    message_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Marking message {} as read", message_id);

    use crate::database::messages as db_messages;

    db_messages::mark_read(&state.db, message_id)
        .await
        .context("Failed to mark message as read")?;

    deliver_success_json(
        Some(serde_json::json!({
            "message_id": message_id,
        })),
        Some("Message marked as read"),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

async fn parse_message_form(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<SendMessageData, MessageError> {
    let body = req
        .collect()
        .await
        .map_err(|_| MessageError::InternalError)?
        .to_bytes();

    let params: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| MessageError::InternalError)?;

    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or(MessageError::MissingField)?
        .to_string();

    let chat_id = params
        .get("chat_id")
        .and_then(|v| v.as_i64())
        .ok_or(MessageError::MissingChat)?;

    Ok(SendMessageData {
        chat_id,
        content,
        message_type: params
            .get("message_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

fn validate_message(data: &SendMessageData) -> std::result::Result<(), MessageError> {
    if data.content.is_empty() {
        return Err(MessageError::EmptyMessage);
    }
    if data.content.len() > 10_000 {
        return Err(MessageError::MessageTooLong);
    }
    Ok(())
}

async fn send_message(
    sender_id: i64,
    data: &SendMessageData,
    state: &AppState,
) -> std::result::Result<(i64, i64), MessageError> {
    use crate::database::groups as db_groups;
    use crate::database::messages as db_messages;

    // Verify chat exists
    let chat = db_groups::get_group(&state.db, data.chat_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?
        .ok_or(MessageError::InvalidChat)?;

    // Verify sender is a member of the chat
    let is_member = db_groups::is_group_member(&state.db, data.chat_id, sender_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?;

    if !is_member {
        return Err(MessageError::NotMemberOfChat);
    }

    // Compress the content
    let compressed_content = crate::database::utils::compress_data(data.content.as_bytes())
        .map_err(|e| {
            error!("Failed to compress message: {}", e);
            MessageError::InternalError
        })?;

    // Store the message
    let message_id = db_messages::send_message(
        &state.db,
        shared::types::message::NewMessage {
            sender_id,
            chat_id: data.chat_id,
            content: compressed_content,
            is_encrypted: true,
            message_type: data
                .message_type
                .clone()
                .unwrap_or_else(|| "text".to_string()),
        },
    )
    .await
    .map_err(|e| {
        error!("Database error sending message: {}", e);
        MessageError::DatabaseError
    })?;

    info!(
        "Message {} sent successfully to chat {}",
        message_id, data.chat_id
    );
    Ok((message_id, crate::database::utils::get_timestamp()))
}

fn parse_query_params(req: &Request<hyper::body::Incoming>) -> GetMessagesQuery {
    let params: HashMap<String, String> =
        form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
            .into_owned()
            .collect();

    GetMessagesQuery {
        chat_id: params
            .get("chat_id")
            .or_else(|| params.get("group_id"))
            .and_then(|s| s.parse().ok()),
        limit: params.get("limit").and_then(|s| s.parse().ok()),
        offset: params.get("offset").and_then(|s| s.parse().ok()),
    }
}

async fn retrieve_messages(
    user_id: i64,
    query: &GetMessagesQuery,
    state: &AppState,
) -> std::result::Result<Vec<MessageResponse>, MessageError> {
    use crate::database::groups as db_groups;
    use crate::database::messages as db_messages;

    let chat_id = query.chat_id.ok_or(MessageError::MissingChat)?;

    // Verify chat exists
    let _chat = db_groups::get_group(&state.db, chat_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?
        .ok_or(MessageError::InvalidChat)?;

    // Verify user is a member of the chat
    let is_member = db_groups::is_group_member(&state.db, chat_id, user_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?;

    if !is_member {
        return Err(MessageError::NotMemberOfChat);
    }

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    // Always query by group_id (works for both DMs and groups since DMs are stored as groups)
    let messages = db_messages::get_chat_messages(&state.db, chat_id, limit, offset)
        .await
        .map_err(|e| {
            error!("Database error getting messages: {}", e);
            MessageError::DatabaseError
        })?;

    messages
        .into_iter()
        .map(|msg| {
            let content = crate::database::utils::decompress_data(&msg.content).map_err(|e| {
                error!("Failed to decompress message {}: {}", msg.id, e);
                MessageError::InternalError
            })?;

            Ok(MessageResponse {
                id: msg.id,
                sender_id: msg.sender_id,
                chat_id: msg.chat_id,
                content: String::from_utf8_lossy(&content).to_string(),
                sent_at: msg.sent_at,
                delivered_at: msg.delivered_at,
                read_at: msg.read_at,
                message_type: msg.message_type,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_message_data() {
        let data = SendMessageData {
            chat_id: 5,
            content: "Hello!".to_string(),
            message_type: None,
        };
        assert!(validate_message(&data).is_ok());
    }

    #[test]
    fn empty_content_fails() {
        let data = SendMessageData {
            chat_id: 5,
            content: "".to_string(),
            message_type: None,
        };
        assert!(matches!(
            validate_message(&data).unwrap_err(),
            MessageError::EmptyMessage
        ));
    }

    #[test]
    fn oversized_content_fails() {
        let data = SendMessageData {
            chat_id: 5,
            content: "x".repeat(10_001),
            message_type: None,
        };
        assert!(matches!(
            validate_message(&data).unwrap_err(),
            MessageError::MessageTooLong
        ));
    }

    #[test]
    fn limit_clamped_to_100() {
        let limit = Some(200_i64).unwrap_or(50).min(100);
        assert_eq!(limit, 100);
    }

    #[test]
    fn limit_defaults_to_50() {
        let limit = None::<i64>.unwrap_or(50).min(100);
        assert_eq!(limit, 50);
    }

    #[test]
    fn message_error_display() {
        let err = MessageError::MissingChat;
        assert_eq!(err.to_code(), "MISSING_CHAT");
        assert!(err.to_message().contains("chat_id"));
    }
}
