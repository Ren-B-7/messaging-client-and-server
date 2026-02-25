use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::Result;
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{Request, Response, StatusCode};
use tracing::{error, info, warn};

use crate::AppState;
use crate::handlers::http::utils::{
    decode_jwt_claims, deliver_serialized_json, validate_jwt_secure,
};
use shared::types::message::*;

/// Get all chats (direct + group) for the authenticated user.
pub async fn handle_get_chats(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing get chats request");

    // FAST PATH — GET request.
    let user_id = match extract_user_from_request(&req, &state) {
        Ok(id) => id,
        Err(err) => {
            warn!("Unauthorized get chats attempt");
            return deliver_serialized_json(&err.to_list_response(), StatusCode::UNAUTHORIZED);
        }
    };

    use crate::database::groups as db_groups;
    use crate::database::messages as db_messages;

    let conversations = db_messages::get_recent_conversations(&state.db, user_id, 50)
        .await
        .map_err(|e| anyhow::anyhow!("Database error getting conversations: {}", e))?;

    let dm_chats: Vec<serde_json::Value> = conversations
        .into_iter()
        .map(|(other_user_id, last_message_time)| {
            serde_json::json!({
                "type":            "direct",
                "other_user_id":   other_user_id,
                "last_message_at": last_message_time,
            })
        })
        .collect();

    let groups = db_groups::get_user_groups(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("Database error getting groups: {}", e))?;

    let group_chats: Vec<serde_json::Value> = groups
        .into_iter()
        .map(|g| {
            serde_json::json!({
                "type":       "group",
                "group_id":   g.id,
                "name":       g.name,
                "created_at": g.created_at,
            })
        })
        .collect();

    let mut chats = dm_chats;
    chats.extend(group_chats);

    deliver_serialized_json(
        &serde_json::json!({ "status": "success", "data": { "chats": chats } }),
        StatusCode::OK,
    )
}

/// Create a new group chat.
pub async fn handle_create_chat(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing create chat request");

    // SECURE PATH — POST (state-changing).
    let user_id = match extract_user_from_request_secure(&req, &state).await {
        Ok(id) => id,
        Err(err) => {
            warn!("Unauthorized create chat attempt: {err}");
            return deliver_serialized_json(
                &MessageError::Unauthorized.to_send_response(),
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    let body = req
        .collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read request body: {}", e))?
        .to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let name = params.get("name").map(|s| s.to_string());

    let participants: Vec<i64> = params
        .get("participants")
        .map(|s| s.split(',').filter_map(|p| p.trim().parse().ok()).collect())
        .unwrap_or_default();

    if participants.is_empty() {
        return deliver_serialized_json(
            &MessageError::MissingField("participants".to_string()).to_send_response(),
            StatusCode::BAD_REQUEST,
        );
    }

    let mut all_participants = participants;
    if !all_participants.contains(&user_id) {
        all_participants.push(user_id);
    }

    use crate::database::groups as db_groups;

    let group_id = db_groups::create_group(
        &state.db,
        db_groups::NewGroup {
            name: name.clone().unwrap_or_else(|| "Direct Chat".to_string()),
            created_by: user_id,
            description: None,
        },
    )
    .await
    .map_err(|e| {
        error!("Failed to create chat group: {}", e);
        anyhow::anyhow!("Database error: {}", e)
    })?;

    for &participant_id in all_participants.iter().filter(|&&id| id != user_id) {
        db_groups::add_group_member(&state.db, group_id, participant_id, "member".to_string())
            .await
            .map_err(|e| {
                error!("Failed to add participant {}: {}", participant_id, e);
                anyhow::anyhow!("Database error: {}", e)
            })?;
    }

    info!("Chat {} created by user {}", group_id, user_id);
    deliver_serialized_json(
        &serde_json::json!({
            "status": "success",
            "message": "Chat created successfully",
            "data": { "chat_id": group_id, "name": name, "participants": all_participants }
        }),
        StatusCode::CREATED,
    )
}

/// Send a message to a direct recipient or a group.
pub async fn handle_send_message(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing send message request");

    // SECURE PATH — POST (state-changing).
    let user_id = match extract_user_from_request_secure(&req, &state).await {
        Ok(id) => id,
        Err(_) => {
            warn!("Unauthorized message send attempt");
            return deliver_serialized_json(
                &MessageError::Unauthorized.to_send_response(),
                StatusCode::UNAUTHORIZED,
            );
        }
    };

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
            info!("Message sent successfully: ID {}", message_id);
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

/// Get messages for a direct conversation or a group.
pub async fn handle_get_messages(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    chat_id: Option<i64>,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing get messages request");

    // FAST PATH — GET request.
    let user_id = match extract_user_from_request(&req, &state) {
        Ok(id) => id,
        Err(err) => {
            warn!("Unauthorized get messages attempt");
            return deliver_serialized_json(&err.to_list_response(), StatusCode::UNAUTHORIZED);
        }
    };

    let mut query = parse_query_params(&req);
    if let Some(id) = chat_id {
        query.group_id = Some(id);
    }

    match retrieve_messages(user_id, &query, &state).await {
        Ok(messages) => {
            info!("Retrieved {} messages", messages.len());
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

/// Mark a message as read.
pub async fn handle_mark_read(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    message_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing mark as read request for message {}", message_id);

    // SECURE PATH — POST (state-changing).
    let _user_id = match extract_user_from_request_secure(&req, &state).await {
        Ok(id) => id,
        Err(_) => {
            return deliver_serialized_json(
                &MessageError::Unauthorized.to_send_response(),
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    use crate::database::messages as db_messages;

    match db_messages::mark_read(&state.db, message_id).await {
        Ok(_) => {
            info!("Message {} marked as read", message_id);
            deliver_serialized_json(
                &SendMessageResponse::Success {
                    message_id,
                    sent_at: crate::database::utils::get_timestamp(),
                    message: "Message marked as read".to_string(),
                },
                StatusCode::OK,
            )
        }
        Err(e) => {
            error!("Failed to mark message as read: {}", e);
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

/// FAST PATH — decode JWT signature only, no DB.  Used on GET requests.
fn extract_user_from_request(
    req: &Request<hyper::body::Incoming>,
    state: &AppState,
) -> std::result::Result<i64, MessageError> {
    decode_jwt_claims(req, &state.jwt_secret)
        .map(|claims| {
            info!("JWT fast-path OK for user_id={}", claims.user_id);
            claims.user_id
        })
        .map_err(|_| MessageError::Unauthorized)
}

/// SECURE PATH — JWT + DB session_id lookup + IP check.  Used on POST/PUT/DELETE.
async fn extract_user_from_request_secure(
    req: &Request<hyper::body::Incoming>,
    state: &AppState,
) -> std::result::Result<i64, MessageError> {
    validate_jwt_secure(req, state)
        .await
        .map(|(id, _)| id)
        .map_err(|_| MessageError::Unauthorized)
}

async fn parse_message_form(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<SendMessageData, MessageError> {
    let body = req
        .collect()
        .await
        .map_err(|_| MessageError::InternalError)?
        .to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let content = params
        .get("content")
        .ok_or(MessageError::MissingField("content".to_string()))?
        .to_string();

    Ok(SendMessageData {
        recipient_id: params.get("recipient_id").and_then(|s| s.parse().ok()),
        group_id: params
            .get("group_id")
            .or_else(|| params.get("chat_id"))
            .and_then(|s| s.parse().ok()),
        content,
        message_type: params.get("message_type").cloned(),
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

    let compressed_content =
        crate::database::utils::compress_data(data.content.as_bytes()).map_err(|e| {
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
            let content =
                crate::database::utils::decompress_data(&msg.content).map_err(|e| {
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
// Tests  (unchanged from original)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use shared::types::message::SendMessageData;

    #[test]
    fn valid_direct_message_passes() {
        let data = SendMessageData { recipient_id: Some(42), group_id: None, content: "Hello!".to_string(), message_type: None };
        assert!(validate_message(&data).is_ok());
    }

    #[test]
    fn missing_recipient_and_group_fails() {
        let data = SendMessageData { recipient_id: None, group_id: None, content: "Nobody home".to_string(), message_type: None };
        assert!(matches!(validate_message(&data).unwrap_err(), MessageError::MissingRecipient));
    }

    #[test]
    fn empty_content_fails() {
        let data = SendMessageData { recipient_id: Some(1), group_id: None, content: "".to_string(), message_type: None };
        assert!(matches!(validate_message(&data).unwrap_err(), MessageError::EmptyMessage));
    }

    #[test]
    fn oversized_content_fails() {
        let data = SendMessageData { recipient_id: Some(1), group_id: None, content: "x".repeat(10_001), message_type: None };
        assert!(matches!(validate_message(&data).unwrap_err(), MessageError::MessageTooLong));
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
}
