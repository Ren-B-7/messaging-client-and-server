use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::{Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};

use crate::AppState;

/// Send message request data
#[derive(Debug, Deserialize)]
pub struct SendMessageData {
    pub recipient_id: Option<i64>,
    pub group_id: Option<i64>,
    pub content: String,
    pub message_type: Option<String>,
}

/// Get messages query parameters
#[derive(Debug, Deserialize)]
pub struct GetMessagesQuery {
    pub other_user_id: Option<i64>,
    pub group_id: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Message response
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: i64,
    pub sender_id: i64,
    pub recipient_id: Option<i64>,
    pub group_id: Option<i64>,
    pub content: String,
    pub sent_at: i64,
    pub delivered_at: Option<i64>,
    pub read_at: Option<i64>,
    pub message_type: String,
}

/// Messages list response
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MessagesResponse {
    Success {
        messages: Vec<MessageResponse>,
        total: usize,
    },
    Error {
        code: String,
        message: String,
    },
}

/// Send message response
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SendMessageResponse {
    Success {
        message_id: i64,
        sent_at: i64,
        message: String,
    },
    Error {
        code: String,
        message: String,
    },
}

/// Message error codes
pub enum MessageError {
    Unauthorized,
    MissingRecipient,
    InvalidRecipient,
    MessageTooLong,
    EmptyMessage,
    MissingField(String),
    DatabaseError,
    InternalError,
}

impl MessageError {
    fn to_code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "UNAUTHORIZED",
            Self::MissingRecipient => "MISSING_RECIPIENT",
            Self::InvalidRecipient => "INVALID_RECIPIENT",
            Self::MessageTooLong => "MESSAGE_TOO_LONG",
            Self::EmptyMessage => "EMPTY_MESSAGE",
            Self::MissingField(_) => "MISSING_FIELD",
            Self::DatabaseError => "DATABASE_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }

    fn to_message(&self) -> String {
        match self {
            Self::Unauthorized => "Authentication required".to_string(),
            Self::MissingRecipient => "Must specify either recipient_id or group_id".to_string(),
            Self::InvalidRecipient => "Invalid recipient or group".to_string(),
            Self::MessageTooLong => "Message exceeds maximum length".to_string(),
            Self::EmptyMessage => "Message cannot be empty".to_string(),
            Self::MissingField(field) => format!("Missing required field: {}", field),
            Self::DatabaseError => "Database error occurred".to_string(),
            Self::InternalError => "An internal error occurred".to_string(),
        }
    }

    fn to_send_response(&self) -> SendMessageResponse {
        SendMessageResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }

    fn to_list_response(&self) -> MessagesResponse {
        MessagesResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }
}

/// Send a message handler
pub async fn handle_send_message(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<http_body_util::Full<Bytes>>> {
    info!("Processing send message request");

    // Extract user_id from session (authenticated request)
    let user_id = match extract_user_from_request(&req, &state).await {
        Ok(id) => id,
        Err(err) => {
            warn!("Unauthorized message send attempt");
            return deliver_send_response(err.to_send_response(), StatusCode::UNAUTHORIZED);
        }
    };

    // Parse message data
    let message_data = match parse_message_form(req).await {
        Ok(data) => data,
        Err(err) => {
            warn!("Message parsing failed: {:?}", err.to_code());
            return deliver_send_response(err.to_send_response(), StatusCode::BAD_REQUEST);
        }
    };

    // Validate message
    if let Err(err) = validate_message(&message_data) {
        warn!("Message validation failed: {:?}", err.to_code());
        return deliver_send_response(err.to_send_response(), StatusCode::BAD_REQUEST);
    }

    // Send the message
    match send_message(user_id, &message_data, &state).await {
        Ok((message_id, sent_at)) => {
            info!("Message sent successfully: ID {}", message_id);

            let response = SendMessageResponse::Success {
                message_id,
                sent_at,
                message: "Message sent successfully".to_string(),
            };

            deliver_send_response(response, StatusCode::CREATED)
        }
        Err(err) => {
            error!("Failed to send message: {:?}", err.to_code());
            deliver_send_response(err.to_send_response(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get messages handler
pub async fn handle_get_messages(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<http_body_util::Full<Bytes>>> {
    info!("Processing get messages request");

    // Extract user_id from session
    let user_id = match extract_user_from_request(&req, &state).await {
        Ok(id) => id,
        Err(err) => {
            warn!("Unauthorized get messages attempt");
            return deliver_list_response(err.to_list_response(), StatusCode::UNAUTHORIZED);
        }
    };

    // Parse query parameters
    let query = parse_query_params(&req);

    // Get messages
    match retrieve_messages(user_id, &query, &state).await {
        Ok(messages) => {
            info!("Retrieved {} messages", messages.len());

            let response = MessagesResponse::Success {
                total: messages.len(),
                messages,
            };

            deliver_list_response(response, StatusCode::OK)
        }
        Err(err) => {
            error!("Failed to retrieve messages: {:?}", err.to_code());
            deliver_list_response(err.to_list_response(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Mark message as read handler
pub async fn handle_mark_read(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    message_id: i64,
) -> Result<Response<http_body_util::Full<Bytes>>> {
    info!("Processing mark as read request for message {}", message_id);

    // Extract user_id from session
    let _user_id = match extract_user_from_request(&req, &state).await {
        Ok(id) => id,
        Err(err) => {
            return deliver_send_response(err.to_send_response(), StatusCode::UNAUTHORIZED);
        }
    };

    // Mark as read
    use crate::database::messages as db_messages;

    match db_messages::mark_read(&state.db, message_id).await {
        Ok(_) => {
            info!("Message {} marked as read", message_id);

            let response = SendMessageResponse::Success {
                message_id,
                sent_at: crate::database::utils::get_timestamp(),
                message: "Message marked as read".to_string(),
            };

            deliver_send_response(response, StatusCode::OK)
        }
        Err(e) => {
            error!("Failed to mark message as read: {}", e);
            deliver_send_response(
                MessageError::DatabaseError.to_send_response(),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

/// Extract authenticated user from request
async fn extract_user_from_request(
    req: &Request<hyper::body::Incoming>,
    state: &AppState,
) -> std::result::Result<i64, MessageError> {
    use crate::database::login as db_login;

    // Extract token from Authorization header or cookie
    let token = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .or_else(|| {
            // Try to get from cookie
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
        .ok_or(MessageError::Unauthorized)?;

    // Validate session token
    let user_id = db_login::validate_session(&state.db, token.to_string())
        .await
        .map_err(|_| MessageError::DatabaseError)?
        .ok_or(MessageError::Unauthorized)?;

    Ok(user_id)
}

/// Parse message form data
async fn parse_message_form(
    req: Request<hyper::body::Incoming>,
) -> std::result::Result<SendMessageData, MessageError> {
    let body = req
        .collect()
        .await
        .map_err(|_| MessageError::InternalError)?
        .to_bytes();

    let params = form_urlencoded::parse(body.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    let content = params
        .get("content")
        .ok_or(MessageError::MissingField("content".to_string()))?
        .to_string();

    let recipient_id = params
        .get("recipient_id")
        .and_then(|s| s.parse::<i64>().ok());

    let group_id = params.get("group_id").and_then(|s| s.parse::<i64>().ok());

    let message_type = params.get("message_type").cloned();

    Ok(SendMessageData {
        recipient_id,
        group_id,
        content,
        message_type,
    })
}

/// Validate message data
fn validate_message(data: &SendMessageData) -> std::result::Result<(), MessageError> {
    // Must have either recipient or group
    if data.recipient_id.is_none() && data.group_id.is_none() {
        return Err(MessageError::MissingRecipient);
    }

    // Check message length
    if data.content.is_empty() {
        return Err(MessageError::EmptyMessage);
    }

    if data.content.len() > 10000 {
        // 10KB limit
        return Err(MessageError::MessageTooLong);
    }

    Ok(())
}

/// Send message to database
async fn send_message(
    sender_id: i64,
    data: &SendMessageData,
    state: &AppState,
) -> std::result::Result<(i64, i64), MessageError> {
    use crate::database::messages as db_messages;

    // Compress the message content
    let compressed_content = crate::database::utils::compress_data(data.content.as_bytes())
        .map_err(|e| {
            error!("Failed to compress message: {}", e);
            MessageError::InternalError
        })?;

    // Create new message
    let message_id = db_messages::send_message(
        &state.db,
        db_messages::NewMessage {
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

    let sent_at = crate::database::utils::get_timestamp();
    Ok((message_id, sent_at))
}

/// Parse query parameters
fn parse_query_params(req: &Request<hyper::body::Incoming>) -> GetMessagesQuery {
    let query_str = req.uri().query().unwrap_or("");
    let params: HashMap<String, String> = form_urlencoded::parse(query_str.as_bytes())
        .into_owned()
        .collect();

    GetMessagesQuery {
        other_user_id: params.get("other_user_id").and_then(|s| s.parse().ok()),
        group_id: params.get("group_id").and_then(|s| s.parse().ok()),
        limit: params.get("limit").and_then(|s| s.parse().ok()),
        offset: params.get("offset").and_then(|s| s.parse().ok()),
    }
}

/// Retrieve messages from database
async fn retrieve_messages(
    user_id: i64,
    query: &GetMessagesQuery,
    state: &AppState,
) -> std::result::Result<Vec<MessageResponse>, MessageError> {
    use crate::database::messages as db_messages;

    let limit = query.limit.unwrap_or(50).min(100); // Max 100 messages
    let offset = query.offset.unwrap_or(0);

    // Get messages based on query type
    let messages = if let Some(other_user_id) = query.other_user_id {
        // Direct messages
        db_messages::get_direct_messages(&state.db, user_id, other_user_id, limit, offset)
            .await
            .map_err(|e| {
                error!("Database error getting messages: {}", e);
                MessageError::DatabaseError
            })?
    } else if let Some(group_id) = query.group_id {
        // Group messages
        db_messages::get_group_messages(&state.db, group_id, limit, offset)
            .await
            .map_err(|e| {
                error!("Database error getting group messages: {}", e);
                MessageError::DatabaseError
            })?
    } else {
        // Return empty if no filter specified
        vec![]
    };

    // Convert to response format and decompress content
    let mut responses = Vec::new();
    for msg in messages {
        // Decompress content
        let content = crate::database::utils::decompress_data(&msg.content).map_err(|e| {
            error!("Failed to decompress message: {}", e);
            MessageError::InternalError
        })?;

        let content_str = String::from_utf8_lossy(&content).to_string();

        responses.push(MessageResponse {
            id: msg.id,
            sender_id: msg.sender_id,
            recipient_id: msg.recipient_id,
            group_id: msg.group_id,
            content: content_str,
            sent_at: msg.sent_at,
            delivered_at: msg.delivered_at,
            read_at: msg.read_at,
            message_type: msg.message_type,
        });
    }

    Ok(responses)
}

/// Deliver send message JSON response
fn deliver_send_response(
    response: SendMessageResponse,
    status: StatusCode,
) -> Result<Response<http_body_util::Full<Bytes>>> {
    let json = serde_json::to_string(&response).context("Failed to serialize response")?;

    let response = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(json)))
        .context("Failed to build response")?;

    Ok(response)
}

/// Deliver messages list JSON response
fn deliver_list_response(
    response: MessagesResponse,
    status: StatusCode,
) -> Result<Response<http_body_util::Full<Bytes>>> {
    let json = serde_json::to_string(&response).context("Failed to serialize response")?;

    let response = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(Bytes::from(json)))
        .context("Failed to build response")?;

    Ok(response)
}
