use std::fmt;

use serde::{Deserialize, Serialize};

/// Send message request data — unified chat_id for both DMs and groups
#[derive(Debug, Deserialize)]
pub struct SendMessageData {
    pub chat_id: i64,
    pub content: String,
    pub message_type: Option<String>,
}

/// Get messages query parameters — unified to use chat_id
#[derive(Debug, Deserialize)]
pub struct GetMessagesQuery {
    #[serde(alias = "group_id")] // Accept legacy group_id from older clients
    pub chat_id: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Message stored in database
#[derive(Debug, Clone)]
pub struct Message {
    pub id: i64,
    pub sender_id: i64,
    pub chat_id: i64,
    pub content: Vec<u8>, // Compressed/encrypted message data
    pub sent_at: i64,
    pub delivered_at: Option<i64>,
    pub read_at: Option<i64>,
    pub is_encrypted: bool,
    pub message_type: String,
}

/// New message to insert into database
#[derive(Debug, Clone)]
pub struct NewMessage {
    pub sender_id: i64,
    pub chat_id: i64,
    pub content: Vec<u8>,
    pub is_encrypted: bool,
    pub message_type: String,
}

/// Message response sent to client
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: i64,
    pub sender_id: i64,
    pub chat_id: i64,
    pub content: String, // Decompressed content
    pub sent_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivered_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
#[derive(Debug, Clone, Copy)]
pub enum MessageError {
    Unauthorized,
    MissingChat,
    InvalidChat,
    NotMemberOfChat,
    MessageTooLong,
    EmptyMessage,
    MissingField,
    DatabaseError,
    InternalError,
    SenderBanned,
}

impl MessageError {
    pub fn to_code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "UNAUTHORIZED",
            Self::MissingChat => "MISSING_CHAT",
            Self::InvalidChat => "INVALID_CHAT",
            Self::NotMemberOfChat => "NOT_MEMBER_OF_CHAT",
            Self::MessageTooLong => "MESSAGE_TOO_LONG",
            Self::EmptyMessage => "EMPTY_MESSAGE",
            Self::MissingField => "MISSING_FIELD",
            Self::DatabaseError => "DATABASE_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
            Self::SenderBanned => "SENDER_BANNED",
        }
    }

    pub fn to_message(&self) -> String {
        match self {
            Self::Unauthorized => "Authentication required".to_string(),
            Self::MissingChat => "Must specify chat_id".to_string(),
            Self::InvalidChat => "Chat not found".to_string(),
            Self::NotMemberOfChat => "You are not a member of this chat".to_string(),
            Self::MessageTooLong => {
                "Message exceeds maximum length (10,000 characters)".to_string()
            }
            Self::EmptyMessage => "Message cannot be empty".to_string(),
            Self::MissingField => "Missing required field: content".to_string(),
            Self::DatabaseError => "Database error occurred".to_string(),
            Self::InternalError => "An internal error occurred".to_string(),
            Self::SenderBanned => "You are banned and cannot send messages".to_string(),
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

impl fmt::Display for MessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.to_code(), self.to_message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_message_data_deserialize() {
        let json = r#"{"chat_id": 5, "content": "hello"}"#;
        let data: SendMessageData = serde_json::from_str(json).unwrap();
        assert_eq!(data.chat_id, 5);
        assert_eq!(data.content, "hello");
        assert_eq!(data.message_type, None);
    }

    #[test]
    fn send_message_data_with_type() {
        let json = r#"{"chat_id": 5, "content": "hello", "message_type": "text"}"#;
        let data: SendMessageData = serde_json::from_str(json).unwrap();
        assert_eq!(data.message_type, Some("text".to_string()));
    }

    #[test]
    fn get_messages_query_chat_id() {
        let json = r#"{"chat_id": 5, "limit": 50}"#;
        let query: GetMessagesQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.chat_id, Some(5));
        assert_eq!(query.limit, Some(50));
    }

    #[test]
    fn get_messages_query_group_id_alias() {
        let json = r#"{"group_id": 5, "limit": 50}"#;
        let query: GetMessagesQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.chat_id, Some(5));
    }

    #[test]
    fn message_response_serialization() {
        let msg = MessageResponse {
            id: 1,
            sender_id: 42,
            chat_id: 5,
            content: "hello".to_string(),
            sent_at: 1709038211,
            delivered_at: None,
            read_at: None,
            message_type: "text".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""id":1"#));
        assert!(json.contains(r#""chat_id":5"#));
        assert!(!json.contains("recipient_id"));
        assert!(!json.contains("delivered_at")); // skip_serializing_if
    }

    #[test]
    fn message_error_codes() {
        assert_eq!(MessageError::MissingChat.to_code(), "MISSING_CHAT");
        assert_eq!(
            MessageError::NotMemberOfChat.to_code(),
            "NOT_MEMBER_OF_CHAT"
        );
        assert_eq!(MessageError::SenderBanned.to_code(), "SENDER_BANNED");
    }

    #[test]
    fn message_error_display() {
        let err = MessageError::MessageTooLong;
        let display = format!("{}", err);
        assert!(display.contains("MESSAGE_TOO_LONG"));
        assert!(display.contains("10,000"));
    }
}
