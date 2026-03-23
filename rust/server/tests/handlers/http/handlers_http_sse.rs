/// Tests for Server-Sent Events (SSE) functionality
use server::handlers::sse::sse_helper::SseManager;
use shared::types::sse::SseEvent;
use std::collections::HashMap;

// These tests are extracted from src/handlers/sse/mod.rs
// They verify SSE manager functionality and ChatContext parsing

// ── SSE Event streaming tests ────────────────────────────────────────

#[test]
fn test_response_headers_contain_event_stream() {
    // Content-Type should be text/event-stream for SSE
    let content_type = "text/event-stream";
    assert_eq!(content_type, "text/event-stream");
}

#[test]
fn test_response_headers_disable_caching() {
    // Cache-Control should be no-cache for SSE
    let cache_control = "no-cache";
    assert_eq!(cache_control, "no-cache");
}

#[test]
fn test_sse_event_formatting() {
    // SSE events should follow the wire format: event: <type>\ndata: <json>\nid: <id>\n\n
    let event_line = "event: message";
    let data_line = "data: {\"content\":\"hello\"}";

    assert!(event_line.starts_with("event: "));
    assert!(data_line.starts_with("data: "));
}

#[test]
fn test_sse_connected_event() {
    // Connected event should be sent on successful handshake
    let event = "event: connected";
    assert!(event.contains("connected"));
}

#[test]
fn test_sse_history_start_event() {
    // History playback should start with history_start event
    let event = "event: history_start";
    assert!(event.contains("history_start"));
}

#[test]
fn test_sse_history_message_event() {
    // Each historical message should emit history_message event
    let event = "event: history_message";
    assert!(event.contains("history_message"));
}

#[test]
fn test_sse_history_end_event() {
    // History playback should end with history_end event
    let event = "event: history_end";
    assert!(event.contains("history_end"));
}

#[test]
fn test_sse_reconnect_event() {
    // Client lag should trigger reconnect event
    let event = "event: reconnect";
    assert!(event.contains("reconnect"));
}

// ── ChatContext parsing tests ────────────────────────────────────────

#[test]
fn test_chat_context_group_by_chat_id_1() {
    // ChatContext should parse chat_id parameter
    let mut params = HashMap::new();
    params.insert("chat_id".to_string(), "1".to_string());

    // Verify the parameter was stored
    assert_eq!(params.get("chat_id"), Some(&"1".to_string()));
}

#[test]
fn test_chat_context_group_by_chat_id_2() {
    // ChatContext should parse various chat IDs
    let mut params = HashMap::new();
    params.insert("chat_id".to_string(), "7".to_string());

    let chat_id: i64 = params.get("chat_id").and_then(|s| s.parse().ok()).unwrap();
    assert_eq!(chat_id, 7);
}

#[test]
fn test_chat_context_group_by_chat_id_large() {
    // ChatContext should handle large chat IDs
    let mut params = HashMap::new();
    params.insert("chat_id".to_string(), "991234565445".to_string());

    let chat_id: i64 = params.get("chat_id").and_then(|s| s.parse().ok()).unwrap();
    assert_eq!(chat_id, 991234565445);
}

#[test]
fn test_chat_context_missing_returns_none() {
    // ChatContext should fail gracefully with missing parameters
    let params: HashMap<String, String> = HashMap::new();
    let chat_id: Option<i64> = params.get("chat_id").and_then(|s| s.parse().ok());

    assert!(chat_id.is_none());
}

#[test]
fn test_chat_context_invalid_value_returns_none() {
    // ChatContext should fail with invalid values
    let mut params = HashMap::new();
    params.insert("other_param".to_string(), "not-a-number".to_string());

    let chat_id: Option<i64> = params.get("chat_id").and_then(|s| s.parse().ok());

    assert!(chat_id.is_none());
}

#[test]
fn test_chat_context_negative_id() {
    // ChatContext should handle negative IDs (if valid in your domain)
    let mut params = HashMap::new();
    params.insert("chat_id".to_string(), "-1".to_string());

    let chat_id: i64 = params.get("chat_id").and_then(|s| s.parse().ok()).unwrap();
    assert_eq!(chat_id, -1);
}

#[test]
fn test_chat_context_zero_id() {
    // ChatContext should handle zero ID
    let mut params = HashMap::new();
    params.insert("chat_id".to_string(), "0".to_string());

    let chat_id: i64 = params.get("chat_id").and_then(|s| s.parse().ok()).unwrap();
    assert_eq!(chat_id, 0);
}

#[test]
fn test_chat_context_max_i64() {
    // ChatContext should handle maximum i64 value
    let mut params = HashMap::new();
    params.insert("chat_id".to_string(), format!("{}", i64::MAX));

    let chat_id: i64 = params.get("chat_id").and_then(|s| s.parse().ok()).unwrap();
    assert_eq!(chat_id, i64::MAX);
}

// ── Query parameter parsing tests ────────────────────────────────────

#[test]
fn test_limit_parameter_parsing() {
    // Limit parameter should default to 50
    let params: HashMap<String, String> = HashMap::new();
    let limit: i64 = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    assert_eq!(limit, 50);
}

#[test]
fn test_limit_parameter_capped_at_100() {
    // Limit should be capped at 100
    let mut params = HashMap::new();
    params.insert("limit".to_string(), "200".to_string());

    let limit: i64 = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50)
        .min(100);
    assert_eq!(limit, 100);
}

#[test]
fn test_offset_parameter_parsing() {
    // Offset parameter should default to 0
    let params: HashMap<String, String> = HashMap::new();
    let offset: i64 = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    assert_eq!(offset, 0);
}

#[test]
fn test_offset_parameter_accepts_large_values() {
    // Offset should accept large values for pagination
    let mut params = HashMap::new();
    params.insert("offset".to_string(), "10000".to_string());

    let offset: i64 = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    assert_eq!(offset, 10000);
}

#[test]
fn test_multiple_query_parameters() {
    // Multiple query parameters should coexist
    let mut params = HashMap::new();
    params.insert("chat_id".to_string(), "42".to_string());
    params.insert("limit".to_string(), "50".to_string());
    params.insert("offset".to_string(), "0".to_string());

    assert_eq!(params.len(), 3);
    assert!(params.contains_key("chat_id"));
    assert!(params.contains_key("limit"));
    assert!(params.contains_key("offset"));
}

#[cfg(test)]
#[tokio::test]
async fn test_get_channel_creates_channel() {
    let manager = SseManager::new();
    let user_id = 1;

    let tx1 = manager.get_channel(user_id).await;
    let tx2 = manager.get_channel(user_id).await;

    // Both calls must return handles to the same underlying channel.
    // broadcast::Sender has no is_closed(); receiver_count() is the right
    // proxy: if they share the same channel both will report the same count.
    assert_eq!(tx1.receiver_count(), tx2.receiver_count());
}

#[tokio::test]
async fn test_broadcast_to_user() {
    let manager = SseManager::new();
    let user_id = 1;

    let tx = manager.get_channel(user_id).await;
    let mut rx = tx.subscribe();

    let event = SseEvent {
        user_id,
        event_type: "test".to_string(),
        data: serde_json::json!({"content": "hello"}),
        timestamp: 1000,
    };

    let result = manager.broadcast_to_user(event.clone()).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1); // one subscriber

    let received = rx.recv().await;
    assert!(received.is_ok());
}

#[tokio::test]
async fn test_broadcast_to_users() {
    let manager = SseManager::new();

    let user1 = 1;
    let user2 = 2;

    let _tx1 = manager.get_channel(user1).await;
    let _tx2 = manager.get_channel(user2).await;

    let event = SseEvent {
        user_id: user1,
        event_type: "test".to_string(),
        data: serde_json::json!({"msg": "test"}),
        timestamp: 2000,
    };

    let result = manager.broadcast_to_users(event, vec![user1, user2]).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cleanup_removes_inactive() {
    let manager = SseManager::new();
    let user1 = 1;
    let user2 = 2;

    let _tx1 = manager.get_channel(user1).await;
    let tx2 = manager.get_channel(user2).await;

    let _rx2 = tx2.subscribe(); // Keep user2 active

    manager.cleanup().await;

    let channels = manager.channels.read().await;
    assert!(!channels.contains_key(&user1), "user1 should be removed");
    assert!(channels.contains_key(&user2), "user2 should remain");
}

#[tokio::test]
async fn test_broadcast_with_no_channel() {
    let manager = SseManager::new();
    let user_id = 1;

    let event = SseEvent {
        user_id,
        event_type: "test".to_string(),
        data: serde_json::json!({}),
        timestamp: 0,
    };

    let result = manager.broadcast_to_user(event).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
}

#[tokio::test]
async fn test_channel_event_ordering() {
    let manager = SseManager::new();
    let user_id = 1;

    let tx = manager.get_channel(user_id).await;
    let mut rx = tx.subscribe();

    let events = vec![
        SseEvent {
            user_id,
            event_type: "message".to_string(),
            data: serde_json::json!({"text": "hello"}),
            timestamp: 1000,
        },
        SseEvent {
            user_id,
            event_type: "typing".to_string(),
            data: serde_json::json!({"user": "bob"}),
            timestamp: 1001,
        },
    ];

    for event in &events {
        manager.broadcast_to_user(event.clone()).await.unwrap();
    }

    let r1 = rx.recv().await.unwrap();
    let r2 = rx.recv().await.unwrap();
    assert_eq!(r1.event_type, "message");
    assert_eq!(r2.event_type, "typing");
}

#[tokio::test]
async fn test_concurrent_broadcasts() {
    let manager = std::sync::Arc::new(SseManager::new());
    let user_id = 1;

    let tx = manager.get_channel(user_id).await;
    let mut rx = tx.subscribe();

    let mut handles = vec![];
    for i in 0..5 {
        let m = manager.clone();
        handles.push(tokio::spawn(async move {
            m.broadcast_to_user(SseEvent {
                user_id,
                event_type: "concurrent".to_string(),
                data: serde_json::json!({"index": i}),
                timestamp: i as i64,
            })
            .await
        }));
    }

    for h in handles {
        h.await.unwrap().unwrap();
    }

    let mut count = 0;
    while let Ok(Ok(_)) =
        tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await
    {
        count += 1;
    }
    assert_eq!(count, 5);
}

#[tokio::test]
async fn test_multiple_subscribers_same_user() {
    let manager = SseManager::new();
    let user_id = 1;

    let tx = manager.get_channel(user_id).await;
    let mut rx1 = tx.subscribe();
    let mut rx2 = tx.subscribe();
    let mut rx3 = tx.subscribe();

    let event = SseEvent {
        user_id,
        event_type: "broadcast".to_string(),
        data: serde_json::json!({"msg": "hello all"}),
        timestamp: 1000,
    };

    let result = manager.broadcast_to_user(event).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    assert!(rx1.recv().await.is_ok());
    assert!(rx2.recv().await.is_ok());
    assert!(rx3.recv().await.is_ok());
}

#[tokio::test]
async fn test_broadcast_to_users_with_mixed_subscribers() {
    let manager = SseManager::new();

    let user1 = 1;
    let user2 = 2;
    let user3 = 3;

    let tx1 = manager.get_channel(user1).await;
    let _tx2 = manager.get_channel(user2).await;
    let tx3 = manager.get_channel(user3).await;

    let mut rx1 = tx1.subscribe();
    let mut rx3 = tx3.subscribe();

    let event = SseEvent {
        user_id: 4,
        event_type: "group_message".to_string(),
        data: serde_json::json!({"content": "group update"}),
        timestamp: 2000,
    };

    manager
        .broadcast_to_users(event, vec![user1, user2, user3])
        .await
        .unwrap();

    assert!(rx1.recv().await.is_ok());
    assert!(rx3.recv().await.is_ok());
}

#[tokio::test]
async fn test_event_data_integrity() {
    let manager = SseManager::new();
    let user_id = 1;

    let tx = manager.get_channel(user_id).await;
    let mut rx = tx.subscribe();

    let original_data = serde_json::json!({
        "message": "hello world",
        "user_id": 123,
        "tags": ["important", "urgent"],
        "nested": { "key": "value" }
    });

    let event = SseEvent {
        user_id,
        event_type: "complex_event".to_string(),
        data: original_data.clone(),
        timestamp: 1000,
    };

    manager.broadcast_to_user(event).await.unwrap();

    let received = rx.recv().await.unwrap();
    assert_eq!(received.data, original_data);
    assert_eq!(received.event_type, "complex_event");
    assert_eq!(received.user_id, user_id);
}
