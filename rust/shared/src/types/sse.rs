use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A live event pushed over an SSE stream to a connected client.
///
/// `user_id` identifies the recipient (matches `users.id`).
///
/// Previously this was `String`, which required `.to_string()` conversions
/// at every call site while the rest of the codebase uses `i64` for user IDs.
/// It is now `i64` throughout for consistency.  Call sites that previously
/// passed `user_id.to_string()` should pass `user_id` directly; call sites
/// that previously compared against a `String` key can compare integers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SseEvent {
    pub user_id: i64,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: i64,
}

#[derive(Error, Clone, Debug)]
pub enum SseError {
    ChannelSendFailed(String),
    ChannelClosed,
}

impl std::fmt::Display for SseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SseError::ChannelSendFailed(msg) => write!(f, "Failed to broadcast event: {}", msg),
            SseError::ChannelClosed => write!(f, "Broadcast channel closed"),
        }
    }
}

pub type SseResult<T> = Result<T, SseError>;
