// shared/src/types/sse.rs
// SSE event types - minimal, no external dependencies

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SseEvent {
    pub user_id: String,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: i64,
}

#[derive(Clone, Debug)]
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

impl std::error::Error for SseError {}

pub type SseResult<T> = Result<T, SseError>;
