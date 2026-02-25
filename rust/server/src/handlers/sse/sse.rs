use bytes::Bytes;
use form_urlencoded;
use futures_util::StreamExt;
use http_body_util::{BodyExt, StreamBody, combinators::BoxBody};
use hyper::{Request, Response, StatusCode, body::Frame, header::HeaderValue};
use shared::types::sse::{SseError, SseEvent, SseResult};
use std::collections::HashMap;
use std::convert::Infallible;
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::AppState;
use crate::database::login as db_login;
use crate::database::messages as db_messages;
use crate::database::utils as db_utils;
use shared::types::login::Session;

// ---------------------------------------------------------------------------
// Chat context — parsed from the SSE request query string
// ---------------------------------------------------------------------------

/// Which conversation to load history for on SSE connect.
///
/// Exactly one variant must be present; if neither is supplied the handshake
/// is rejected.
#[derive(Debug, Clone)]
pub enum ChatContext {
    /// Direct-message conversation between the authenticated user and another user
    Direct { other_user_id: i64 },
    /// Group / multi-user chat identified by its chat/group id
    Group { group_id: i64 },
}

impl ChatContext {
    /// Parse from query-string params. Accepts `group_id` and `chat_id` as synonyms.
    pub fn from_params(params: &HashMap<String, String>) -> Option<Self> {
        if let Some(id) = params.get("other_user_id").and_then(|s| s.parse().ok()) {
            return Some(Self::Direct { other_user_id: id });
        }
        if let Some(id) = params
            .get("group_id")
            .or_else(|| params.get("chat_id"))
            .and_then(|s| s.parse().ok())
        {
            return Some(Self::Group { group_id: id });
        }
        None
    }
}

// ---------------------------------------------------------------------------
// SseManager
// ---------------------------------------------------------------------------

/// SSE connection manager — holds one broadcast channel per connected user.
#[derive(Debug)]
pub struct SseManager {
    /// user_id → broadcast sender
    pub channels:
        tokio::sync::RwLock<std::collections::HashMap<String, broadcast::Sender<SseEvent>>>,
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
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .clone()
    }

    /// Broadcast an event to a specific user
    pub async fn broadcast_to_user(&self, event: SseEvent) -> SseResult<usize> {
        let channels = self.channels.read().await;
        if let Some(tx) = channels.get(&event.user_id) {
            let count = tx.receiver_count();
            info!(
                "Broadcasting {} event to user {} ({} subscribers)",
                event.event_type, event.user_id, count
            );
            tx.send(event).map_err(|_| {
                error!("Failed to send SSE event to user");
                SseError::ChannelSendFailed("Failed to send event".to_string())
            })?;
            Ok(count)
        } else {
            info!("No channel found for user: {}", event.user_id);
            Ok(0)
        }
    }

    /// Broadcast the same event to multiple users (user_id is overwritten per recipient)
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

    /// Remove channels with no active subscribers
    pub async fn cleanup(&self) {
        let mut channels = self.channels.write().await;
        let before = channels.len();
        channels.retain(|_, tx| tx.receiver_count() > 0);
        let after = channels.len();
        if before != after {
            info!(
                "SSE cleanup: removed {} inactive channels ({} → {} remaining)",
                before - after,
                before,
                after
            );
        }
    }
}

// ---------------------------------------------------------------------------
// SseStreamBuilder
// ---------------------------------------------------------------------------

/// Helpers for formatting SSE wire frames
pub struct SseStreamBuilder;

impl SseStreamBuilder {
    /// Standard SSE response headers
    pub fn response_headers() -> (HeaderValue, HeaderValue) {
        (
            HeaderValue::from_static("text/event-stream"),
            HeaderValue::from_static("no-cache"),
        )
    }

    /// Serialise an [`SseEvent`] into the SSE wire format
    pub fn format_event(event: &SseEvent) -> String {
        let data = serde_json::to_string(&event.data).unwrap_or_else(|_| "{}".to_string());
        format!(
            "event: {}\ndata: {}\nid: {}\n\n",
            event.event_type,
            data,
            Uuid::new_v4()
        )
    }

    /// Emit a simple named event carrying arbitrary JSON data (no SseEvent wrapper)
    pub fn format_raw(event_type: &str, data: &serde_json::Value) -> String {
        let data_str = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
        format!(
            "event: {}\ndata: {}\nid: {}\n\n",
            event_type,
            data_str,
            Uuid::new_v4()
        )
    }
}

// ---------------------------------------------------------------------------
// Request helpers
// ---------------------------------------------------------------------------

/// Extract a session token from the request.
///
/// Checks, in order:
/// 1. `Authorization: Bearer <token>` header
/// 2. `auth_id=<token>` cookie  (set by login / register handlers)
/// 3. `auth_token=<token>` cookie   (legacy fallback)
fn extract_token(req: &Request<hyper::body::Incoming>) -> Option<String> {
    // 1. Bearer header
    if let Some(token) = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
    {
        return Some(token);
    }

    // 2 & 3. Cookie jar — accept either name
    req.headers()
        .get("cookie")
        .and_then(|h| h.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let cookie = cookie.trim();
                // auth_id wins; fall through to auth_token
                for prefix in &["auth_id=", "auth_token="] {
                    if let Some(val) = cookie.strip_prefix(prefix) {
                        if !val.is_empty() {
                            return Some(val.to_string());
                        }
                    }
                }
                None
            })
        })
}

/// Parse query-string params into a `HashMap`
fn parse_query(req: &Request<hyper::body::Incoming>) -> HashMap<String, String> {
    form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
        .into_owned()
        .collect()
}

// ---------------------------------------------------------------------------
// SSE subscribe handler
// ---------------------------------------------------------------------------

/// Authenticate, load history, then stream live events.
///
/// ### Query parameters
/// | Param           | Description                                          |
/// |-----------------|------------------------------------------------------|
/// | `other_user_id` | Load DM history with this user                       |
/// | `group_id`      | Load group-chat history (alias: `chat_id`)           |
/// | `limit`         | Max history messages to replay (default 50, max 100) |
/// | `offset`        | Pagination offset into history (default 0)           |
///
/// ### Event sequence emitted
/// ```
/// event: connected        — handshake OK
/// event: history_start    — client enters history-replay mode
/// event: history_message  — one per historical message (oldest first)
/// event: history_end      — client switches to live mode
/// event: <live events>    — forwarded from the SseManager broadcast channel
/// event: reconnect        — client lagged; should reconnect
/// ```
pub async fn handle_sse_subscribe(
    req: Request<hyper::body::Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>, SseError> {
    // ── 1. Authenticate ────────────────────────────────────────────────────
    let token = extract_token(&req).ok_or_else(|| {
        warn!("SSE subscribe rejected: missing auth token");
        SseError::ChannelSendFailed("Unauthorized".to_string())
    })?;

    let session: Session =
        db_login::validate_session_id(&state.db, token)
        .await
        .map_err(|e| {
            error!("SSE auth DB error: {}", e);
            SseError::ChannelSendFailed("Database error".to_string())
        })?
        .ok_or_else(|| {
            warn!("SSE subscribe rejected: invalid session");
            SseError::ChannelSendFailed("Unauthorized".to_string())
        })?;
    let user_id = session.user_id;

    // ── 2. Parse chat context & pagination ─────────────────────────────────
    let params = parse_query(&req);

    let chat_ctx = ChatContext::from_params(&params).ok_or_else(|| {
        warn!(
            "SSE subscribe rejected for user {}: missing chat context",
            user_id
        );
        SseError::ChannelSendFailed(
            "Missing required param: other_user_id or group_id/chat_id".to_string(),
        )
    })?;

    let limit: i64 = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50)
        .min(100);
    let offset: i64 = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    info!(
        "SSE subscribe: user={} context={:?} limit={} offset={}",
        user_id, chat_ctx, limit, offset
    );

    // ── 3. Fetch history ───────────────────────────────────────────────────
    let history = match &chat_ctx {
        ChatContext::Direct { other_user_id } => {
            db_messages::get_direct_messages(&state.db, user_id, *other_user_id, limit, offset)
                .await
                .map_err(|e| {
                    error!("SSE history fetch (DM) failed: {}", e);
                    SseError::ChannelSendFailed("Failed to fetch message history".to_string())
                })?
        }
        ChatContext::Group { group_id } => {
            db_messages::get_group_messages(&state.db, *group_id, limit, offset)
                .await
                .map_err(|e| {
                    error!("SSE history fetch (group) failed: {}", e);
                    SseError::ChannelSendFailed("Failed to fetch message history".to_string())
                })?
        }
    };

    // Decompress all messages up front — fail fast before opening the stream
    let mut history_frames: Vec<String> = Vec::with_capacity(history.len() + 2);

    history_frames.push(SseStreamBuilder::format_raw(
        "history_start",
        &serde_json::json!({ "count": history.len() }),
    ));

    for msg in &history {
        let content = db_utils::decompress_data(&msg.content).map_err(|e| {
            error!("SSE history decompress failed for msg {}: {}", msg.id, e);
            SseError::ChannelSendFailed("Failed to decompress message".to_string())
        })?;

        let content_str = String::from_utf8_lossy(&content).to_string();

        history_frames.push(SseStreamBuilder::format_raw(
            "history_message",
            &serde_json::json!({
                "id":           msg.id,
                "sender_id":    msg.sender_id,
                "recipient_id": msg.recipient_id,
                "group_id":     msg.group_id,
                "content":      content_str,
                "message_type": msg.message_type,
                "sent_at":      msg.sent_at,
                "delivered_at": msg.delivered_at,
                "read_at":      msg.read_at,
            }),
        ));
    }

    history_frames.push(SseStreamBuilder::format_raw(
        "history_end",
        &serde_json::json!({}),
    ));

    // ── 4. Subscribe to live events ────────────────────────────────────────
    let tx = state.sse_manager.get_channel(user_id.to_string()).await;
    let mut rx = tx.subscribe();

    let (content_type, cache_control) = SseStreamBuilder::response_headers();

    // ── 5. Build the stream ────────────────────────────────────────────────
    let stream = async_stream::stream! {
        let connected = "event: connected\ndata: {}\n\n";
        info!("SSE connected: user={}", user_id);
        yield Ok::<Bytes, Infallible>(Bytes::from(connected));

        for frame in history_frames {
            yield Ok::<Bytes, Infallible>(Bytes::from(frame));
        }

        loop {
            match rx.recv().await {
                Ok(event) => {
                    let formatted = SseStreamBuilder::format_event(&event);
                    info!("SSE live event '{}' → user={}", event.event_type, user_id);
                    yield Ok::<Bytes, Infallible>(Bytes::from(formatted));
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("SSE client lagged by {} messages, sending reconnect hint", n);
                    let frame = format!(
                        "event: reconnect\ndata: {}\n\n",
                        serde_json::json!({ "reason": "lagged", "missed": n })
                    );
                    yield Ok::<Bytes, Infallible>(Bytes::from(frame));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("SSE channel closed: user={}", user_id);
                    break;
                }
            }
        }
    };

    let body = BodyExt::boxed(StreamBody::new(
        stream.map(|result| result.map(Frame::data)),
    ));

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", content_type)
        .header("cache-control", cache_control)
        .header("connection", "keep-alive")
        .header("x-accel-buffering", "no")
        .body(body)
        .map_err(|e| {
            error!("Failed to build SSE response: {}", e);
            SseError::ChannelSendFailed("Failed to build SSE response".to_string())
        })
}
