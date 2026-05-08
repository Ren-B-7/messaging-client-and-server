use std::collections::HashMap;
use std::convert::Infallible;

use anyhow::Context;
use bytes::Bytes;
use form_urlencoded;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use tracing::{error, info, warn};
use uuid::Uuid;

use shared::types::groups::*;
use shared::types::jwt::JwtClaims;
use shared::types::message::*;
use shared::types::sse::SseEvent;

use crate::AppState;
use crate::database::{groups, messages, utils};
use crate::handlers::http::utils::{
    deliver_error_json, deliver_serialized_json, deliver_success_json,
};

pub const MAX_MESSAGE_LENGTH: usize = 10_000;
pub const DEFAULT_LIMIT: i64 = 50;
pub const MAX_LIMIT: i64 = 100;

// ---------------------------------------------------------------------------
// GET /api/chats
// ---------------------------------------------------------------------------

/// List all chats (DM + group) for the authenticated user.
pub async fn handle_get_chats(
    _req: Request<Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    let user_id = claims.user_id;
    info!("Processing get chats request for user {}", user_id);

    let chats = groups::get_user_groups(&state.db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("Database error getting chats: {}", e))?;

    let dm_ids: Vec<i64> = chats
        .iter()
        .filter(|g| g.chat_type == "direct")
        .map(|g| g.id)
        .collect();

    let mut dm_info: HashMap<i64, (String, Option<String>)> = HashMap::new();
    if !dm_ids.is_empty() {
        // sqlx doesn't support binding Vec directly in IN clause easily without macros or extensions.
        // We'll use a manual join for now or multiple queries if necessary.
        // For efficiency, we'll build a query with placeholders.
        let mut query_builder = sqlx::QueryBuilder::new(
            "SELECT gm.chat_id, u.username, u.avatar_path
             FROM   chat_members gm
             JOIN   users u ON u.id = gm.user_id
             WHERE  gm.user_id != ",
        );
        query_builder.push_bind(user_id);
        query_builder.push(" AND gm.chat_id IN (");
        let mut separated = query_builder.separated(", ");
        for id in dm_ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let rows: Vec<(i64, String, Option<String>)> = query_builder
            .build_query_as()
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

        for (chat_id, username, avatar_path) in rows {
            let avatar_url = avatar_path.map(|_| format!("/api/avatar/{}", chat_id));
            dm_info.insert(chat_id, (username, avatar_url));
        }
    }

    let mut chats_json: Vec<serde_json::Value> = Vec::with_capacity(chats.len());

    for g in chats {
        let (display_name, avatar_url) = if g.chat_type == "direct" {
            dm_info
                .get(&g.id)
                .cloned()
                .unwrap_or_else(|| (g.name.clone(), None))
        } else {
            (g.name.clone(), None)
        };

        chats_json.push(serde_json::json!({
            "chat_id":     g.id,
            "name":        display_name,
            "description": g.description,
            "chat_type":   g.chat_type,
            "created_by":  g.created_by,
            "created_at":  g.created_at,
            "avatar_url":  avatar_url,
        }));
    }

    deliver_serialized_json(
        &serde_json::json!({ "status": "success", "data": { "chats": chats_json } }),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// POST /api/chats
// ---------------------------------------------------------------------------

/// Create a new direct message (DM) with another user.
pub async fn handle_create_chat(
    req: Request<Incoming>,
    state: AppState,
    user_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Processing create chat request from user {}", user_id);

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: serde_json::Value =
        serde_json::from_slice(&body).context("Failed to parse JSON request body")?;

    let other_user_id: i64 = if let Some(uid) = params.get("user_id").and_then(|v| v.as_i64()) {
        uid
    } else if let Some(username) = params.get("username").and_then(|v| v.as_str()) {
        match utils::get_user_by_username(&state.db, username.to_string()).await? {
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

    let other_username = utils::get_user_by_id(&state.db, other_user_id)
        .await
        .ok()
        .flatten()
        .map(|u| u.username)
        .unwrap_or_else(|| format!("user_{}", other_user_id));

    let other_avatar_url = utils::get_user_avatar(&state.db, other_user_id)
        .await
        .ok()
        .flatten()
        .map(|_| format!("/api/avatar/{}", other_user_id));

    if let Some(existing_chat_id) =
        groups::find_existing_dm(&state.db, user_id, other_user_id).await?
    {
        if !groups::is_group_member(&state.db, existing_chat_id, user_id).await? {
            groups::add_group_member(&state.db, existing_chat_id, user_id, "admin".to_string())
                .await
                .context("Failed to rejoin existing DM")?;
        }

        let chat = groups::get_group(&state.db, existing_chat_id)
            .await?
            .context("Failed to retrieve existing chat")?;

        return deliver_success_json(
            Some(serde_json::json!({
                "id":         chat.id,
                "chat_id":    chat.id,
                "name":       other_username,
                "chat_type":  "direct",
                "created_at": chat.created_at,
                "avatar_url": other_avatar_url,
            })),
            Some("Existing DM returned"),
            StatusCode::OK,
        );
    }

    let internal_name = Uuid::new_v4().to_string();
    let chat_id = groups::create_group(
        &state.db,
        NewGroup {
            name: internal_name,
            created_by: user_id,
            description: None,
            chat_type: "direct".to_string(),
        },
    )
    .await
    .context("Failed to create DM")?;

    groups::add_group_member(&state.db, chat_id, other_user_id, "admin".to_string())
        .await
        .context("Failed to add other participant to DM")?;

    info!(
        "DM {} created between users {} and {}",
        chat_id, user_id, other_user_id
    );

    sse_broadcast_chat_created(&state, chat_id, user_id, other_user_id, "direct").await;

    deliver_success_json(
        Some(serde_json::json!({
            "id":         chat_id,
            "chat_id":    chat_id,
            "name":       other_username,
            "chat_type":  "direct",
            "created_at": utils::get_timestamp(),
            "avatar_url": other_avatar_url,
        })),
        Some("DM created successfully"),
        StatusCode::CREATED,
    )
}

// ---------------------------------------------------------------------------
// POST /api/messages/send
// ---------------------------------------------------------------------------

pub async fn handle_send_message(
    req: Request<Incoming>,
    state: AppState,
    user_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
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

// ---------------------------------------------------------------------------
// GET /api/messages
// ---------------------------------------------------------------------------

pub async fn handle_get_messages(
    req: Request<Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    let user_id = claims.user_id;
    info!("Processing get messages request for user {}", user_id);

    let query = parse_query_params(&req);

    match retrieve_messages(user_id, &query, &state).await {
        Ok(msgs) => deliver_serialized_json(
            &MessagesResponse::Success {
                total: msgs.len(),
                messages: msgs,
            },
            StatusCode::OK,
        ),
        Err(err) => {
            error!("Failed to retrieve messages: {:?}", err.to_code());
            deliver_serialized_json(&err.to_list_response(), StatusCode::BAD_REQUEST)
        }
    }
}

/// Mark a single message as read and — now also as delivered if not already.
pub async fn handle_mark_read(
    _req: Request<Incoming>,
    state: AppState,
    user_id: i64,
    message_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Marking message {} as read by user {}", message_id, user_id);

    let msg = messages::get_message_by_id(&state.db, message_id)
        .await
        .context("Failed to fetch message")?;

    messages::mark_delivered(&state.db, message_id)
        .await
        .context("Failed to mark message as delivered")?;

    messages::mark_read(&state.db, message_id)
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

/// Delete a message the caller sent.
pub async fn handle_delete_message(
    _req: Request<Incoming>,
    state: AppState,
    user_id: i64,
    message_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    info!(
        "Delete message {} requested by user {}",
        message_id, user_id
    );

    let msg = match messages::get_message_by_id(&state.db, message_id).await? {
        Some(m) => m,
        None => {
            return deliver_error_json("NOT_FOUND", "Message not found", StatusCode::NOT_FOUND);
        }
    };

    let is_member = groups::is_group_member(&state.db, msg.chat_id, user_id)
        .await
        .context("DB error checking membership")?;

    if !is_member {
        return deliver_error_json(
            "FORBIDDEN",
            "You are not a member of this chat",
            StatusCode::FORBIDDEN,
        );
    }

    let deleted = messages::delete_message(&state.db, message_id, user_id)
        .await
        .context("Failed to delete message")?;

    if !deleted {
        return deliver_error_json(
            "FORBIDDEN",
            "You can only delete your own messages",
            StatusCode::FORBIDDEN,
        );
    }

    info!("Message {} deleted by user {}", message_id, user_id);

    sse_broadcast_message_deleted(&state, message_id, user_id, msg.chat_id).await;

    deliver_success_json(
        Some(serde_json::json!({
            "message_id": message_id,
            "chat_id":    msg.chat_id,
        })),
        Some("Message deleted"),
        StatusCode::OK,
    )
}

/// Return the total number of unread messages for the authenticated user
/// across all their chats, and a per-chat breakdown.
pub async fn handle_get_unread(
    req: Request<Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    let user_id = claims.user_id;
    info!("Unread count request for user {}", user_id);

    let chat_id_filter: Option<i64> =
        form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
            .find(|(k, _)| k == "chat_id")
            .and_then(|(_, v)| v.parse().ok());

    if let Some(chat_id) = chat_id_filter {
        let count = messages::get_unread_count_for_chat(&state.db, chat_id, user_id)
            .await
            .context("Failed to fetch unread count for chat")?;

        return deliver_success_json(
            Some(serde_json::json!({
                "total": count,
                "by_chat": [{ "chat_id": chat_id, "unread": count }],
            })),
            None,
            StatusCode::OK,
        );
    }

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM messages m
         INNER JOIN chat_members gm ON gm.chat_id = m.chat_id
         WHERE gm.user_id = ?
           AND m.sender_id != ?
           AND m.read_at IS NULL",
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or((0,));

    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT m.chat_id, COUNT(*) as unread
         FROM messages m
         INNER JOIN chat_members gm ON gm.chat_id = m.chat_id
         WHERE gm.user_id = ?
           AND m.sender_id != ?
           AND m.read_at IS NULL
         GROUP BY m.chat_id
         ORDER BY unread DESC",
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let by_chat: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "chat_id": r.0,
                "unread":  r.1,
            })
        })
        .collect();

    deliver_success_json(
        Some(serde_json::json!({
            "total":   total.0,
            "by_chat": by_chat,
        })),
        None,
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// POST /api/typing
// ---------------------------------------------------------------------------

pub async fn handle_typing(
    req: Request<Incoming>,
    state: AppState,
    user_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
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

    let members = groups::get_group_members(&state.db, chat_id)
        .await
        .unwrap_or_default();

    let recipients: Vec<i64> = members
        .iter()
        .filter(|m| m.user_id != user_id)
        .map(|m| m.user_id)
        .collect();

    if !recipients.is_empty() {
        let event = SseEvent {
            user_id: 0,
            event_type: "typing".to_string(),
            data: serde_json::json!({
                "chat_id":   chat_id,
                "user_id":   user_id,
                "is_typing": is_typing,
            }),
            timestamp: utils::get_timestamp(),
        };

        if let Err(e) = state
            .sse_manager
            .broadcast_to_users(event, recipients)
            .await
        {
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

async fn sse_broadcast_message_sent(
    state: &AppState,
    message_id: i64,
    sender_id: i64,
    chat_id: i64,
    content: &str,
    message_type: &str,
    sent_at: i64,
) {
    let members = match groups::get_group_members(&state.db, chat_id).await {
        Ok(m) => m,
        Err(e) => {
            error!(
                "SSE message_sent: failed to fetch members for chat {}: {}",
                chat_id, e
            );
            return;
        }
    };

    let recipients: Vec<i64> = members.iter().map(|m| m.user_id).collect();
    if recipients.is_empty() {
        return;
    }

    let event = SseEvent {
        user_id: 0,
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

    if let Err(e) = state
        .sse_manager
        .broadcast_to_users(event, recipients)
        .await
    {
        error!(
            "SSE message_sent broadcast failed for chat {}: {:?}",
            chat_id, e
        );
    }
}

async fn sse_broadcast_message_read(
    state: &AppState,
    message_id: i64,
    reader_id: i64,
    chat_id: i64,
    sender_id: i64,
) {
    let now = utils::get_timestamp();

    let members = match groups::get_group_members(&state.db, chat_id).await {
        Ok(m) => m,
        Err(_) => {
            let event = SseEvent {
                user_id: sender_id,
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

    let recipients: Vec<i64> = members
        .iter()
        .filter(|m| m.user_id != reader_id)
        .map(|m| m.user_id)
        .collect();

    if recipients.is_empty() {
        return;
    }

    let event = SseEvent {
        user_id: 0,
        event_type: "message_read".to_string(),
        data: serde_json::json!({
            "message_id": message_id,
            "chat_id":    chat_id,
            "reader_id":  reader_id,
            "read_at":    now,
        }),
        timestamp: now,
    };

    if let Err(e) = state
        .sse_manager
        .broadcast_to_users(event, recipients)
        .await
    {
        error!("SSE message_read broadcast failed: {:?}", e);
    }
}

async fn sse_broadcast_chat_created(
    state: &AppState,
    chat_id: i64,
    creator_id: i64,
    other_user_id: i64,
    chat_type: &str,
) {
    let now = utils::get_timestamp();
    let event = SseEvent {
        user_id: 0,
        event_type: "chat_created".to_string(),
        data: serde_json::json!({
            "chat_id":    chat_id,
            "chat_type":  chat_type,
            "created_by": creator_id,
        }),
        timestamp: now,
    };
    let recipients = vec![creator_id, other_user_id];
    if let Err(e) = state
        .sse_manager
        .broadcast_to_users(event, recipients)
        .await
    {
        warn!("SSE chat_created broadcast failed: {:?}", e);
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

async fn parse_message_body(
    req: Request<Incoming>,
) -> anyhow::Result<SendMessageData, MessageError> {
    let body = req
        .collect()
        .await
        .map_err(|_| MessageError::InternalError)?
        .to_bytes();

    let params: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| MessageError::InternalError)?;

    Ok(SendMessageData {
        chat_id: params
            .get("chat_id")
            .and_then(|v| v.as_i64())
            .ok_or(MessageError::MissingChat)?,
        content: params
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or(MessageError::MissingField)?
            .to_string(),
        message_type: params
            .get("message_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

pub fn validate_message(data: &SendMessageData) -> anyhow::Result<(), MessageError> {
    if data.content.trim().is_empty() {
        return Err(MessageError::EmptyMessage);
    }
    if data.content.len() > MAX_MESSAGE_LENGTH {
        return Err(MessageError::MessageTooLong);
    }
    Ok(())
}

pub fn parse_limit(raw: Option<i64>) -> i64 {
    raw.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT)
}

async fn persist_message(
    sender_id: i64,
    data: &SendMessageData,
    state: &AppState,
) -> std::result::Result<(i64, i64), MessageError> {
    let _chat = groups::get_group(&state.db, data.chat_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?
        .ok_or(MessageError::InvalidChat)?;

    if !groups::is_group_member(&state.db, data.chat_id, sender_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?
    {
        return Err(MessageError::NotMemberOfChat);
    }

    let compressed = utils::compress_data(data.content.as_bytes()).map_err(|e| {
        error!("Failed to compress message: {}", e);
        MessageError::InternalError
    })?;

    let message_id = messages::send_message(
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

    Ok((message_id, utils::get_timestamp()))
}

fn parse_query_params(req: &Request<Incoming>) -> GetMessagesQuery {
    let params: HashMap<String, String> =
        form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
            .into_owned()
            .collect();

    GetMessagesQuery {
        chat_id: params.get("chat_id").and_then(|s| s.parse().ok()),
        limit: params.get("limit").and_then(|s| s.parse().ok()),
        offset: params.get("offset").and_then(|s| s.parse().ok()),
    }
}

async fn retrieve_messages(
    user_id: i64,
    query: &GetMessagesQuery,
    state: &AppState,
) -> std::result::Result<Vec<MessageResponse>, MessageError> {
    let chat_id = query.chat_id.ok_or(MessageError::MissingChat)?;

    let _chat = groups::get_group(&state.db, chat_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?
        .ok_or(MessageError::InvalidChat)?;

    if !groups::is_group_member(&state.db, chat_id, user_id)
        .await
        .map_err(|_| MessageError::DatabaseError)?
    {
        return Err(MessageError::NotMemberOfChat);
    }

    let limit = parse_limit(query.limit);
    let offset = query.offset.unwrap_or(0);

    messages::get_chat_messages(&state.db, chat_id, limit, offset)
        .await
        .map_err(|e| {
            error!("Database error getting messages: {}", e);
            MessageError::DatabaseError
        })?
        .into_iter()
        .map(|msg| {
            let content_bytes = utils::decompress_data(&msg.content).map_err(|e| {
                error!("Failed to decompress message {}: {}", msg.id, e);
                MessageError::InternalError
            })?;
            let content = String::from_utf8(content_bytes).map_err(|e| {
                error!("Failed to decode message {} as UTF-8: {}", msg.id, e);
                MessageError::InternalError
            })?;
            Ok(MessageResponse {
                id: msg.id,
                sender_id: msg.sender_id,
                chat_id: msg.chat_id,
                content,
                sent_at: msg.sent_at,
                delivered_at: msg.delivered_at,
                read_at: msg.read_at,
                message_type: msg.message_type,
            })
        })
        .collect()
}

async fn sse_broadcast_message_deleted(
    state: &AppState,
    message_id: i64,
    deleted_by: i64,
    chat_id: i64,
) {
    let members = match groups::get_group_members(&state.db, chat_id).await {
        Ok(m) => m,
        Err(e) => {
            error!(
                "SSE message_deleted: failed to fetch members for chat {}: {}",
                chat_id, e
            );
            return;
        }
    };

    let recipients: Vec<i64> = members.iter().map(|m| m.user_id).collect();
    if recipients.is_empty() {
        return;
    }

    let now = utils::get_timestamp();
    let event = SseEvent {
        user_id: 0,
        event_type: "message_deleted".to_string(),
        data: serde_json::json!({
            "message_id": message_id,
            "chat_id":    chat_id,
            "deleted_by": deleted_by,
            "deleted_at": now,
        }),
        timestamp: now,
    };

    if let Err(e) = state
        .sse_manager
        .broadcast_to_users(event, recipients)
        .await
    {
        error!(
            "SSE message_deleted broadcast failed for chat {}: {:?}",
            chat_id, e
        );
    } else {
        info!(
            "SSE message_deleted: msg={} chat={} by={}",
            message_id, chat_id, deleted_by
        );
    }
}
