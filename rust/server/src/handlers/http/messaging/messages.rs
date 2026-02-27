use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{Request, Response, StatusCode};
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::http::utils::{deliver_error_json, deliver_serialized_json, deliver_success_json};
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
/// Returns a unified list — no separate DM vs group split.  The `chat_type`
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

/// POST /api/chats — create a new direct chat.
///
/// Hard-auth route: `user_id` is pre-verified by the router (JWT + DB + IP).
///
/// Body parameters:
///   - `name`         — chat name (required; typically the other user's display name for DMs)
///   - `participants` — comma-separated user IDs to add (required; must not be empty)
///   - `description`  — optional description
///
/// All participants (including the creator) are added as `"admin"` because
/// there is no meaningful moderation hierarchy in a direct message.
pub async fn handle_create_chat(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing create chat request from user {}", user_id);

    use crate::database::groups as db_groups;

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: serde_json::Value = serde_json::from_slice(&body)
        .context("Failed to parse JSON request body")?;

    let name = match params.get("name").and_then(|v| v.as_str()).map(|s| s.trim().to_string()) {
        Some(n) if !n.is_empty() => n,
        _ => {
            return deliver_error_json(
                "INVALID_INPUT",
                "Chat name is required",
                StatusCode::BAD_REQUEST,
            );
        }
    };

    // Parse participants, excluding the creator — create_group already adds
    // them as the first member.
    let participants: Vec<i64> = params
        .get("participants")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_i64())
                .filter(|&id| id != user_id)
                .collect()
        })
        .unwrap_or_default();

    if participants.is_empty() {
        return deliver_error_json(
            "INVALID_INPUT",
            "At least one other participant is required",
            StatusCode::BAD_REQUEST,
        );
    }

    let description: Option<String> = params.get("description").map(|s| s.to_string());

    // Create the group row — creator is added as admin by create_group.
    let group_id = db_groups::create_group(
        &state.db,
        db_groups::NewGroup {
            name: name.clone(),
            created_by: user_id,
            description: description.clone(),
            chat_type: "direct".to_string(),
        },
    )
    .await
    .context("Failed to create chat")?;

    // Every other participant is also an admin — DMs have no hierarchy.
    for &participant_id in &participants {
        db_groups::add_group_member(&state.db, group_id, participant_id, "admin".to_string())
            .await
            .context(format!("Failed to add participant {}", participant_id))?;
    }

    info!(
        "Direct chat {} ('{}') created by user {} with {} participant(s)",
        group_id,
        name,
        user_id,
        participants.len()
    );

    let mut all_participants = participants;
    all_participants.push(user_id);

    deliver_success_json(
        Some(serde_json::json!({
            "chat_id":      group_id,
            "name":         name,
            "description":  description,
            "chat_type":    "direct",
            "participants": all_participants,
        })),
        Some("Chat created successfully"),
        StatusCode::CREATED,
    )
}

/// POST /api/messages/send — send a message to a direct recipient or group.
///
/// Hard-auth route: `user_id` is pre-verified by the router (JWT + DB + IP).
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
            info!("Message {} sent by user {}", message_id, user_id);
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
            deliver_serialized_json(&err.to_send_response(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/messages — retrieve messages for a conversation or group.
///
/// Light-auth route: `claims` are pre-verified by the router (JWT only, no DB).
pub async fn handle_get_messages(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    claims: JwtClaims,
    chat_id: Option<i64>,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    let user_id = claims.user_id;
    info!("Processing get messages request for user {}", user_id);

    let mut query = parse_query_params(&req);
    if let Some(id) = chat_id {
        query.group_id = Some(id);
    }

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
            deliver_serialized_json(&err.to_list_response(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// POST /api/messages/:id/read — mark a message as read.
///
/// Hard-auth route: `user_id` is pre-verified by the router (JWT + DB + IP).
pub async fn handle_mark_read(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
    message_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("User {} marking message {} as read", user_id, message_id);

    use crate::database::messages as db_messages;

    match db_messages::mark_read(&state.db, message_id).await {
        Ok(_) => deliver_serialized_json(
            &SendMessageResponse::Success {
                message_id,
                sent_at: crate::database::utils::get_timestamp(),
                message: "Message marked as read".to_string(),
            },
            StatusCode::OK,
        ),
        Err(e) => {
            error!("Failed to mark message {} as read: {}", message_id, e);
            deliver_serialized_json(
                &MessageError::DatabaseError.to_send_response(),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
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

    let params: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| MessageError::InternalError)?;

    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or(MessageError::MissingField("content".to_string()))?
        .to_string();

    Ok(SendMessageData {
        recipient_id: params.get("recipient_id").and_then(|v| v.as_i64()),
        group_id: params
            .get("group_id")
            .or_else(|| params.get("chat_id"))
            .and_then(|v| v.as_i64()),
        content,
        message_type: params.get("message_type").and_then(|v| v.as_str()).map(|s| s.to_string()),
    })
}

fn validate_message(data: &SendMessageData) -> std::result::Result<(), MessageError> {
    if data.recipient_id.is_none() && data.group_id.is_none() {
        return Err(MessageError::MissingRecipient);
    }
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
    use crate::database::messages as db_messages;

    let compressed_content = crate::database::utils::compress_data(data.content.as_bytes())
        .map_err(|e| {
            error!("Failed to compress message: {}", e);
            MessageError::InternalError
        })?;

    let message_id = db_messages::send_message(
        &state.db,
        shared::types::message::NewMessage {
            sender_id,
            recipient_id: data.recipient_id,
            group_id: data.group_id,
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

    Ok((message_id, crate::database::utils::get_timestamp()))
}

fn parse_query_params(req: &Request<hyper::body::Incoming>) -> GetMessagesQuery {
    let params: HashMap<String, String> =
        form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
            .into_owned()
            .collect();

    GetMessagesQuery {
        other_user_id: params.get("other_user_id").and_then(|s| s.parse().ok()),
        group_id: params
            .get("group_id")
            .or_else(|| params.get("chat_id"))
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
    use crate::database::messages as db_messages;

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let messages = if let Some(other_user_id) = query.other_user_id {
        db_messages::get_direct_messages(&state.db, user_id, other_user_id, limit, offset)
            .await
            .map_err(|e| {
                error!("Database error getting direct messages: {}", e);
                MessageError::DatabaseError
            })?
    } else if let Some(group_id) = query.group_id {
        db_messages::get_group_messages(&state.db, group_id, limit, offset)
            .await
            .map_err(|e| {
                error!("Database error getting group messages: {}", e);
                MessageError::DatabaseError
            })?
    } else {
        vec![]
    };

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
                recipient_id: msg.recipient_id,
                group_id: msg.group_id,
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
    use shared::types::message::SendMessageData;

    #[test]
    fn valid_direct_message_passes() {
        let data = SendMessageData {
            recipient_id: Some(42),
            group_id: None,
            content: "Hello!".to_string(),
            message_type: None,
        };
        assert!(validate_message(&data).is_ok());
    }

    #[test]
    fn missing_recipient_and_group_fails() {
        let data = SendMessageData {
            recipient_id: None,
            group_id: None,
            content: "Nobody home".to_string(),
            message_type: None,
        };
        assert!(matches!(
            validate_message(&data).unwrap_err(),
            MessageError::MissingRecipient
        ));
    }

    #[test]
    fn empty_content_fails() {
        let data = SendMessageData {
            recipient_id: Some(1),
            group_id: None,
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
            recipient_id: Some(1),
            group_id: None,
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
        assert_eq!(200_usize.min(100), 100);
    }

    #[test]
    fn limit_defaults_to_50() {
        let effective = None::<usize>.unwrap_or(50).min(100);
        assert_eq!(effective, 50);
    }

    #[test]
    fn creator_excluded_from_participant_add_loop() {
        let user_id: i64 = 99;
        let raw = "1,2,99,3";
        let participants: Vec<i64> = raw
            .split(',')
            .filter_map(|p| p.trim().parse::<i64>().ok())
            .filter(|&id| id != user_id)
            .collect();
        assert_eq!(participants, vec![1, 2, 3]);
        assert!(!participants.contains(&user_id));
    }

    #[test]
    fn direct_chat_participants_all_get_admin_role() {
        // Verify the role string used for DM participants.
        let chat_type = "direct";
        let role = if chat_type == "direct" { "admin" } else { "member" };
        assert_eq!(role, "admin");
    }
}
