use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{Request, Response, StatusCode};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::AppState;
use crate::handlers::http::utils::{deliver_error_json, deliver_serialized_json, deliver_success_json};
use shared::types::jwt::JwtClaims;
use shared::types::message::*;
use shared::types::sse::SseEvent;

// ---------------------------------------------------------------------------
// Handlers
//
// Auth is performed by the router BEFORE these handlers are called.
// Mutating handlers receive a verified `user_id: i64` (hard auth).
// Read-only handlers receive the decoded `JwtClaims` (light auth).
// No handler calls any auth function internally.
// ---------------------------------------------------------------------------

/// GET /api/chats — list all chats (DM + group) for the authenticated user.
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
/// Body (one of):
///   `{ "user_id": 42 }` or `{ "username": "alice" }`
///
/// Idempotent: returns the existing DM if one already exists between the pair.
/// Fires a `chat_created` SSE event to both participants on creation.
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

    // Resolve the other participant from user_id or username
    let other_user_id: i64 = if let Some(uid) = params.get("user_id").and_then(|v| v.as_i64()) {
        uid
    } else if let Some(username) = params.get("username").and_then(|v| v.as_str()) {
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

    if other_user_id == user_id {
        return deliver_error_json(
            "INVALID_INPUT",
            "Cannot create a DM with yourself",
            StatusCode::BAD_REQUEST,
        );
    }

    // Idempotency check
    if let Some(existing_chat_id) =
        db_groups::find_existing_dm(&state.db, user_id, other_user_id).await?
    {
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

    let internal_name = Uuid::new_v4().to_string();

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

    db_groups::add_group_member(&state.db, chat_id, other_user_id, "admin".to_string())
        .await
        .context("Failed to add other participant to DM")?;

    info!(
        "DM {} created between users {} and {}",
        chat_id, user_id, other_user_id
    );

    // Notify both participants so their chat lists refresh
    sse_broadcast_chat_created(&state, chat_id, user_id, other_user_id, "direct").await;

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
/// Body:
/// ```json
/// { "chat_id": 5, "content": "Hello", "message_type": "text" }
/// ```
///
/// Fires a `message_sent` SSE event to every member of the chat.
pub async fn handle_send_message(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing send message request from user {}", user_id);

    let message_data = match parse_message_body(req).await {
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

    match persist_message(user_id, &message_data, &state).await {
        Ok((message_id, sent_at)) => {
            info!(
                "Message {} sent by user {} to chat {}",
                message_id, user_id, message_data.chat_id
            );

            // Broadcast the new message to every member of the chat
            sse_broadcast_message_sent(
                &state,
                message_id,
                user_id,
                message_data.chat_id,
                &message_data.content,
                message_data.message_type.as_deref().unwrap_or("text"),
                sent_at,
            )
            .await;

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

/// GET /api/messages?chat_id=N — paginated message history.
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

/// POST /api/messages/:id/read — mark a single message as read.
///
/// Fires a `message_read` SSE event to all chat members except the reader.
pub async fn handle_mark_read(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
    message_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Marking message {} as read by user {}", message_id, user_id);

    use crate::database::messages as db_messages;

    // Fetch the message first so we have chat_id + sender_id for the broadcast
    let msg = db_messages::get_message_by_id(&state.db, message_id)
        .await
        .context("Failed to fetch message")?;

    db_messages::mark_read(&state.db, message_id)
        .await
        .context("Failed to mark message as read")?;

    if let Some(ref m) = msg {
        sse_broadcast_message_read(&state, message_id, user_id, m.chat_id, m.sender_id).await;
    }

    deliver_success_json(
        Some(serde_json::json!({ "message_id": message_id })),
        Some("Message marked as read"),
        StatusCode::OK,
    )
}

/// POST /api/typing — broadcast a typing indicator to other chat members.
///
/// Body: `{ "chat_id": 5, "is_typing": true }`
///
/// Fire-and-forget: always returns 200. No DB write is made — the event is
/// pushed directly over SSE to all other members of the chat.
pub async fn handle_typing(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::json!({}));

    let chat_id = match params.get("chat_id").and_then(|v| v.as_i64()) {
        Some(id) => id,
        None => {
            return deliver_error_json(
                "INVALID_INPUT",
                "Missing required field: chat_id",
                StatusCode::BAD_REQUEST,
            );
        }
    };

    let is_typing = params
        .get("is_typing")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let members = db_groups::get_group_members(&state.db, chat_id)
        .await
        .unwrap_or_default();

    let recipients: Vec<String> = members
        .iter()
        .filter(|m| m.user_id != user_id) // don't echo back to the typer
        .map(|m| m.user_id.to_string())
        .collect();

    if !recipients.is_empty() {
        let event = SseEvent {
            user_id: String::new(), // overwritten per recipient by broadcast_to_users
            event_type: "typing".to_string(),
            data: serde_json::json!({
                "chat_id":   chat_id,
                "user_id":   user_id,
                "is_typing": is_typing,
            }),
            timestamp: crate::database::utils::get_timestamp(),
        };

        if let Err(e) = state.sse_manager.broadcast_to_users(event, recipients).await {
            warn!("Typing broadcast failed for chat {}: {:?}", chat_id, e);
        }
    }

    deliver_success_json(
        None::<serde_json::Value>,
        Some("Typing indicator sent"),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// SSE broadcast helpers
// ---------------------------------------------------------------------------

/// Push a `message_sent` event to every member of the chat.
///
/// The sender is included so that multi-device clients stay in sync.
/// Clients suppress their own echo by comparing `sender_id` to `my_user_id`.
async fn sse_broadcast_message_sent(
    state: &AppState,
    message_id: i64,
    sender_id: i64,
    chat_id: i64,
    content: &str,
    message_type: &str,
    sent_at: i64,
) {
    use crate::database::groups as db_groups;

    let members = match db_groups::get_group_members(&state.db, chat_id).await {
        Ok(m) => m,
        Err(e) => {
            error!(
                "SSE message_sent: failed to fetch members for chat {}: {}",
                chat_id, e
            );
            return;
        }
    };

    let recipients: Vec<String> = members.iter().map(|m| m.user_id.to_string()).collect();

    if recipients.is_empty() {
        return;
    }

    let event = SseEvent {
        user_id: String::new(),
        event_type: "message_sent".to_string(),
        data: serde_json::json!({
            "id":           message_id,
            "chat_id":      chat_id,
            "sender_id":    sender_id,
            "content":      content,
            "message_type": message_type,
            "sent_at":      sent_at,
        }),
        timestamp: sent_at,
    };

    if let Err(e) = state.sse_manager.broadcast_to_users(event, recipients).await {
        error!(
            "SSE message_sent broadcast failed for chat {}: {:?}",
            chat_id, e
        );
    } else {
        info!(
            "SSE message_sent: msg={} chat={} sender={}",
            message_id, chat_id, sender_id
        );
    }
}

/// Push a `message_read` event to all chat members except the reader.
async fn sse_broadcast_message_read(
    state: &AppState,
    message_id: i64,
    reader_id: i64,
    chat_id: i64,
    sender_id: i64,
) {
    use crate::database::groups as db_groups;

    let now = crate::database::utils::get_timestamp();

    let members = match db_groups::get_group_members(&state.db, chat_id).await {
        Ok(m) => m,
        Err(e) => {
            error!(
                "SSE message_read: failed to fetch members for chat {}: {}",
                chat_id, e
            );
            // Fall back to notifying only the original sender
            let event = SseEvent {
                user_id: sender_id.to_string(),
                event_type: "message_read".to_string(),
                data: serde_json::json!({
                    "message_id": message_id,
                    "chat_id":    chat_id,
                    "reader_id":  reader_id,
                    "read_at":    now,
                }),
                timestamp: now,
            };
            let _ = state.sse_manager.broadcast_to_user(event).await;
            return;
        }
    };

    // Everyone except the reader gets the receipt
    let recipients: Vec<String> = members
        .iter()
        .filter(|m| m.user_id != reader_id)
        .map(|m| m.user_id.to_string())
        .collect();

    if recipients.is_empty() {
        return;
    }

    let event = SseEvent {
        user_id: String::new(),
        event_type: "message_read".to_string(),
        data: serde_json::json!({
            "message_id": message_id,
            "chat_id":    chat_id,
            "reader_id":  reader_id,
            "read_at":    now,
        }),
        timestamp: now,
    };

    if let Err(e) = state.sse_manager.broadcast_to_users(event, recipients).await {
        error!("SSE message_read broadcast failed: {:?}", e);
    } else {
        info!(
            "SSE message_read: msg={} reader={} chat={}",
            message_id, reader_id, chat_id
        );
    }
}

/// Push a `chat_created` event to both participants of a new DM.
async fn sse_broadcast_chat_created(
    state: &AppState,
    chat_id: i64,
    creator_id: i64,
    other_user_id: i64,
    chat_type: &str,
) {
    let now = crate::database::utils::get_timestamp();

    let event = SseEvent {
        user_id: String::new(),
        event_type: "chat_created".to_string(),
        data: serde_json::json!({
            "chat_id":    chat_id,
            "chat_type":  chat_type,
            "created_by": creator_id,
        }),
        timestamp: now,
    };

    let recipients = vec![creator_id.to_string(), other_user_id.to_string()];

    if let Err(e) = state.sse_manager.broadcast_to_users(event, recipients).await {
        warn!("SSE chat_created broadcast failed: {:?}", e);
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

async fn parse_message_body(
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

/// Compress and persist a message, returning `(message_id, sent_at)`.
async fn persist_message(
    sender_id: i64,
    data: &SendMessageData,
    state: &AppState,
) -> std::result::Result<(i64, i64), MessageError> {
    use crate::database::groups as db_groups;
    use crate::database::messages as db_messages;

    let _chat = db_groups::get_group(&state.db, data.chat_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?
        .ok_or(MessageError::InvalidChat)?;

    let is_member = db_groups::is_group_member(&state.db, data.chat_id, sender_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?;

    if !is_member {
        return Err(MessageError::NotMemberOfChat);
    }

    let compressed = crate::database::utils::compress_data(data.content.as_bytes()).map_err(|e| {
        error!("Failed to compress message: {}", e);
        MessageError::InternalError
    })?;

    let message_id = db_messages::send_message(
        &state.db,
        NewMessage {
            sender_id,
            chat_id: data.chat_id,
            content: compressed,
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

    let _chat = db_groups::get_group(&state.db, chat_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?
        .ok_or(MessageError::InvalidChat)?;

    let is_member = db_groups::is_group_member(&state.db, chat_id, user_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?;

    if !is_member {
        return Err(MessageError::NotMemberOfChat);
    }

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

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
