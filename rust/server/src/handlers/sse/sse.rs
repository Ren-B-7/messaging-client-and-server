use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::{BodyExt, StreamBody, combinators::BoxBody};
use hyper::{Request, Response, StatusCode, body::Frame, header::HeaderValue};
use shared::types::sse::{SseError, SseEvent, SseResult};
use std::convert::Infallible;
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::AppState;

/// SSE connection manager
#[derive(Debug)]
pub struct SseManager {
    /// Map of user_id -> broadcast sender
    channels: tokio::sync::RwLock<std::collections::HashMap<String, broadcast::Sender<SseEvent>>>,
}

impl SseManager {
    pub fn new() -> Self {
        Self {
            channels: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Get or create a broadcast channel for a user
    pub async fn get_channel(&self, user_id: String) -> broadcast::Sender<SseEvent> {
        let mut channels = self.channels.write().await;
        channels
            .entry(user_id.clone())
            .or_insert_with(|| {
                info!("Creating new SSE channel for user: {}", user_id);
                let (tx, _) = broadcast::channel(100); // buffer 100 events per channel
                tx
            })
            .clone()
    }

    /// Broadcast an event to a specific user
    pub async fn broadcast_to_user(&self, event: SseEvent) -> SseResult<usize> {
        let channels = self.channels.read().await;
        if let Some(tx) = channels.get(&event.user_id) {
            let count = tx.receiver_count();
            if count == 0 {
                info!(
                    "Broadcasting {} event to user {} (no subscribers)",
                    event.event_type, event.user_id
                );
            } else {
                info!(
                    "Broadcasting {} event to user {} ({} subscribers)",
                    event.event_type, event.user_id, count
                );
            }
            tx.send(event).map_err(|_| {
                error!("Failed to send SSE event to user");
                SseError::ChannelSendFailed("Failed to send event".to_string())
            })?;
            Ok(count)
        } else {
            info!("No channel found for user: {}", event.user_id);
            Ok(0) // No subscribers
        }
    }

    /// Broadcast to multiple users
    pub async fn broadcast_to_users(
        &self,
        event: SseEvent,
        user_ids: Vec<String>,
    ) -> SseResult<()> {
        let channels = self.channels.read().await;
        info!(
            "Broadcasting {} event to {} users",
            event.event_type,
            user_ids.len()
        );

        for user_id in user_ids {
            if let Some(tx) = channels.get(&user_id) {
                let mut evt = event.clone();
                evt.user_id = user_id.clone();
                match tx.send(evt) {
                    Ok(_) => info!("Event sent to user: {}", user_id),
                    Err(_) => warn!("Failed to send event to user: {} (no receivers)", user_id),
                }
            } else {
                warn!("No channel for user: {}", user_id);
            }
        }
        Ok(())
    }

    /// Cleanup channel when no subscribers
    pub async fn cleanup(&self) {
        let mut channels = self.channels.write().await;
        let before_count = channels.len();
        channels.retain(|_, tx| tx.receiver_count() > 0);
        let after_count = channels.len();

        if before_count != after_count {
            info!(
                "SSE cleanup: removed {} inactive channels ({} -> {} remaining)",
                before_count - after_count,
                before_count,
                after_count
            );
        }
    }
}

/// SSE stream response builder
pub struct SseStreamBuilder;

impl SseStreamBuilder {
    /// Create SSE response headers
    pub fn response_headers() -> (HeaderValue, HeaderValue) {
        (
            HeaderValue::from_static("text/event-stream"),
            HeaderValue::from_static("no-cache"),
        )
    }

    /// Format event as SSE
    pub fn format_event(event: &SseEvent) -> String {
        let data_str = serde_json::to_string(&event.data).unwrap_or_else(|_| "{}".to_string());

        format!(
            "event: {}\ndata: {}\nid: {}\n\n",
            event.event_type,
            data_str,
            Uuid::new_v4()
        )
    }
}

/// Handle SSE upgrade and streaming
pub async fn handle_sse_subscribe(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: String,
) -> Result<Response<BoxBody<Bytes, Infallible>>, SseError> {
    info!("SSE subscribe request from user: {}", user_id);

    // Get or create broadcast channel for this user
    let sse_manager = &state.sse_manager;
    let tx = sse_manager.get_channel(user_id.clone()).await;
    let mut rx = tx.subscribe();

    // Create streaming response body
    let (content_type, cache_control) = SseStreamBuilder::response_headers();

    let stream = async_stream::stream! {
        // Send initial connection event
        let welcome = format!("event: connected\ndata: {{}}\n\n");
        info!("Sending SSE connected event to user: {}", user_id);
        yield Ok::<Bytes, Infallible>(Bytes::from(welcome));

        // Stream events from broadcast channel
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let formatted = SseStreamBuilder::format_event(&event);
                    info!("Streaming {} event to user", event.event_type);
                    yield Ok::<Bytes, Infallible>(Bytes::from(formatted));
                }
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    warn!("SSE client lagged by {} messages, sending reconnect", count);
                    let event = "event: reconnect\ndata: {\"reason\":\"lagged\"}\n\n";
                    yield Ok::<Bytes, Infallible>(Bytes::from(event));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("SSE channel closed for user: {}", user_id);
                    break;
                }
            }
        }
    };

    // Use StreamBody which properly wraps our stream
    let body = BodyExt::boxed(StreamBody::new(
        stream.map(|result| result.map(Frame::data)),
    ));

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", content_type)
        .header("cache-control", cache_control)
        .header("connection", "keep-alive")
        .header("x-accel-buffering", "no") // Disable nginx buffering
        .body(body)
        .map_err(|e| {
            error!("Failed to build SSE response: {}", e);
            SseError::ChannelSendFailed("Failed to build SSE response".to_string())
        })
}
