use serde::{Deserialize, Serialize};

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
    pub fn to_code(&self) -> &'static str {
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

    pub fn to_message(&self) -> String {
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

    pub fn to_send_response(&self) -> SendMessageResponse {
        SendMessageResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }

    pub fn to_list_response(&self) -> MessagesResponse {
        MessagesResponse::Error {
            code: self.to_code().to_string(),
            message: self.to_message(),
        }
    }
}
