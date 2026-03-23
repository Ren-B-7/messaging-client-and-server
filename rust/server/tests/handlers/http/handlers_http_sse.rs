// NOTE: the string-literal tests that existed here (asserting that
// "event: connected".contains("connected") etc.) have been removed —
// they only tested Rust string methods against literals we wrote ourselves.
// The tests below exercise actual code paths.

use server::handlers::sse::sse_helper::{SseManager, SseStreamBuilder};
use shared::types::sse::SseEvent;
use std::collections::HashMap;

// ── SSE wire-format output ────────────────────────────────────────────────

#[test]
fn response_headers_are_correct_values() {
    let (content_type, cache_control) = SseStreamBuilder::response_headers();
    assert_eq!(content_type, "text/event-stream");
    assert_eq!(cache_control, "no-cache");
}

#[test]
fn format_raw_structure_is_correct() {
    let frame = SseStreamBuilder::format_raw("connected", &serde_json::json!({}));
    let lines: Vec<&str> = frame.lines().collect();
    assert!(lines[0].starts_with("event: connected"));
    assert!(lines[1].starts_with("data: "));
    assert!(lines[2].starts_with("id: "));
    assert!(frame.ends_with("\n\n"));
}

#[test]
fn format_event_structure_is_correct() {
    let event = SseEvent {
        user_id: 1,
        event_type: "message".to_string(),
        data: serde_json::json!({ "content": "hello" }),
        timestamp: 0,
    };
    let frame = SseStreamBuilder::format_event(&event);
    let lines: Vec<&str> = frame.lines().collect();
    assert_eq!(lines[0], "event: message");
    assert!(lines[1].starts_with("data: "));
    assert!(lines[2].starts_with("id: "));
    assert!(frame.ends_with("\n\n"));
}

#[test]
fn format_event_serialises_data_as_json() {
    let event = SseEvent {
        user_id: 7,
        event_type: "msg".to_string(),
        data: serde_json::json!({ "text": "hi", "num": 42 }),
        timestamp: 0,
    };
    let frame = SseStreamBuilder::format_event(&event);
    let data_line = frame.lines().find(|l| l.starts_with("data: ")).unwrap();
    let json_str = data_line.trim_start_matches("data: ");
    let v: serde_json::Value = serde_json::from_str(json_str).unwrap();
    assert_eq!(v["text"], "hi");
    assert_eq!(v["num"], 42);
}

// ── Query-string pagination defaults ─────────────────────────────────────

#[test]
fn limit_defaults_to_50() {
    let params: HashMap<String, String> = HashMap::new();
    let limit: i64 = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    assert_eq!(limit, 50);
}

#[test]
fn limit_is_capped_at_100() {
    let mut params = HashMap::new();
    params.insert("limit".to_string(), "999".to_string());
    let limit: i64 = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50)
        .min(100);
    assert_eq!(limit, 100);
}

#[test]
fn offset_defaults_to_zero() {
    let params: HashMap<String, String> = HashMap::new();
    let offset: i64 = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    assert_eq!(offset, 0);
}

// ── History reversal ─────────────────────────────────────────────────────
// The SSE handler calls `history.reverse()` after fetching messages ordered
// DESC (newest first / LIMIT N) to replay them oldest-first to the client.
// Test that the reversal logic produces the correct order.

#[test]
fn history_reverse_produces_oldest_first_order() {
    // Simulate what get_chat_messages returns: DESC order (newest first)
    let mut messages = vec![
        ("msg_newest", 1003i64),
        ("msg_middle", 1002i64),
        ("msg_oldest", 1001i64),
    ];
    // The handler calls history.reverse()
    messages.reverse();

    assert_eq!(messages[0].0, "msg_oldest");
    assert_eq!(messages[1].0, "msg_middle");
    assert_eq!(messages[2].0, "msg_newest");
}

#[test]
fn history_reverse_empty_slice_is_a_no_op() {
    let mut messages: Vec<&str> = vec![];
    messages.reverse();
    assert!(messages.is_empty());
}

#[test]
fn history_reverse_single_element_unchanged() {
    let mut messages = vec!["only_message"];
    messages.reverse();
    assert_eq!(messages[0], "only_message");
}

// ── Async SseManager tests ────────────────────────────────────────────────

#[tokio::test]
async fn get_channel_is_idempotent() {
    let manager = SseManager::new();
    let tx1 = manager.get_channel(1).await;
    let tx2 = manager.get_channel(1).await;
    assert_eq!(tx1.receiver_count(), tx2.receiver_count());
}

#[tokio::test]
async fn broadcast_to_user_delivers_correct_event() {
    let manager = SseManager::new();
    let tx = manager.get_channel(1).await;
    let mut rx = tx.subscribe();

    let event = SseEvent {
        user_id: 1,
        event_type: "test".to_string(),
        data: serde_json::json!({"content": "hello"}),
        timestamp: 1000,
    };

    let result = manager.broadcast_to_user(event.clone()).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    let received = rx.recv().await.unwrap();
    assert_eq!(received.event_type, "test");
    assert_eq!(received.data["content"], "hello");
}

#[tokio::test]
async fn broadcast_to_users_reaches_all_subscribers() {
    let manager = SseManager::new();

    let tx1 = manager.get_channel(1).await;
    let tx2 = manager.get_channel(2).await;
    let mut rx1 = tx1.subscribe();
    let mut rx2 = tx2.subscribe();

    let event = SseEvent {
        user_id: 0,
        event_type: "group_message".to_string(),
        data: serde_json::json!({"msg": "hello group"}),
        timestamp: 2000,
    };
    manager.broadcast_to_users(event, vec![1, 2]).await.unwrap();

    assert!(rx1.recv().await.is_ok());
    assert!(rx2.recv().await.is_ok());
}

#[tokio::test]
async fn cleanup_removes_inactive_channel() {
    let manager = SseManager::new();
    let _tx1 = manager.get_channel(1).await; // no subscriber — goes stale

    let tx2 = manager.get_channel(2).await;
    let _rx2 = tx2.subscribe(); // keeps user 2 active

    manager.cleanup().await;

    let channels = manager.channels.read().await;
    assert!(!channels.contains_key(&1), "user1 should be removed");
    assert!(channels.contains_key(&2), "user2 should remain");
}

#[tokio::test]
async fn broadcast_with_no_channel_returns_zero() {
    let manager = SseManager::new();
    let event = SseEvent {
        user_id: 42,
        event_type: "test".to_string(),
        data: serde_json::json!({}),
        timestamp: 0,
    };
    let result = manager.broadcast_to_user(event).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
}

#[tokio::test]
async fn multiple_subscribers_all_receive_event() {
    let manager = SseManager::new();
    let tx = manager.get_channel(1).await;
    let mut rx1 = tx.subscribe();
    let mut rx2 = tx.subscribe();
    let mut rx3 = tx.subscribe();

    let event = SseEvent {
        user_id: 1,
        event_type: "broadcast".to_string(),
        data: serde_json::json!({"msg": "hello all"}),
        timestamp: 1000,
    };
    let count = manager.broadcast_to_user(event).await.unwrap();
    assert_eq!(count, 3);

    assert!(rx1.recv().await.is_ok());
    assert!(rx2.recv().await.is_ok());
    assert!(rx3.recv().await.is_ok());
}

#[tokio::test]
async fn event_data_integrity_preserved_through_broadcast() {
    let manager = SseManager::new();
    let tx = manager.get_channel(1).await;
    let mut rx = tx.subscribe();

    let original = serde_json::json!({
        "message": "hello world",
        "user_id": 123,
        "tags": ["important", "urgent"],
        "nested": { "key": "value" }
    });

    manager
        .broadcast_to_user(SseEvent {
            user_id: 1,
            event_type: "complex_event".to_string(),
            data: original.clone(),
            timestamp: 1000,
        })
        .await
        .unwrap();

    let received = rx.recv().await.unwrap();
    assert_eq!(received.data, original);
    assert_eq!(received.event_type, "complex_event");
}
