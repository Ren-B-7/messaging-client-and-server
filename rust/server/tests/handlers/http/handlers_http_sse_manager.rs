use server::handlers::sse::sse_helper::{ChatContext, SseManager, SseStreamBuilder};
use shared::types::sse::SseEvent;
use std::collections::HashMap;

// ── ChatContext::from_params ──────────────────────────────────────────────

#[test]
fn chat_context_parses_chat_id() {
    let mut p = HashMap::new();
    p.insert("chat_id".to_string(), "42".to_string());
    let ctx = ChatContext::from_params(&p);
    assert!(ctx.is_some());
    match ctx.unwrap() {
        ChatContext::Chat { chat_id } => assert_eq!(chat_id, 42),
    }
}

#[test]
fn chat_context_parses_large_id() {
    let mut p = HashMap::new();
    p.insert("chat_id".to_string(), format!("{}", i64::MAX));
    let ctx = ChatContext::from_params(&p);
    assert!(ctx.is_some());
    match ctx.unwrap() {
        ChatContext::Chat { chat_id } => assert_eq!(chat_id, i64::MAX),
    }
}

#[test]
fn chat_context_missing_param_returns_none() {
    let p: HashMap<String, String> = HashMap::new();
    assert!(ChatContext::from_params(&p).is_none());
}

#[test]
fn chat_context_non_numeric_returns_none() {
    let mut p = HashMap::new();
    p.insert("chat_id".to_string(), "not-a-number".to_string());
    assert!(ChatContext::from_params(&p).is_none());
}

#[test]
fn chat_context_wrong_key_returns_none() {
    let mut p = HashMap::new();
    p.insert("group_id".to_string(), "5".to_string()); // wrong key
    assert!(ChatContext::from_params(&p).is_none());
}

// ── SseStreamBuilder::format_raw ─────────────────────────────────────────

#[test]
fn format_raw_contains_event_line() {
    let data = serde_json::json!({ "count": 3 });
    let frame = SseStreamBuilder::format_raw("history_start", &data);
    assert!(
        frame.starts_with("event: history_start\n"),
        "frame: {}",
        frame
    );
}

#[test]
fn format_raw_contains_data_line() {
    let data = serde_json::json!({ "count": 3 });
    let frame = SseStreamBuilder::format_raw("history_start", &data);
    assert!(frame.contains("data:"), "frame: {}", frame);
    assert!(frame.contains("\"count\":3"), "frame: {}", frame);
}

#[test]
fn format_raw_contains_id_line() {
    let data = serde_json::json!({});
    let frame = SseStreamBuilder::format_raw("history_end", &data);
    assert!(frame.contains("id:"), "frame: {}", frame);
}

#[test]
fn format_raw_ends_with_double_newline() {
    let data = serde_json::json!({});
    let frame = SseStreamBuilder::format_raw("connected", &data);
    assert!(frame.ends_with("\n\n"), "SSE frame must end with \\n\\n");
}

#[test]
fn format_raw_ids_are_unique() {
    let data = serde_json::json!({});
    let f1 = SseStreamBuilder::format_raw("evt", &data);
    let f2 = SseStreamBuilder::format_raw("evt", &data);
    // Extract the id lines and verify they differ (UUID v4 each time)
    let id1: &str = f1.lines().find(|l| l.starts_with("id:")).unwrap();
    let id2: &str = f2.lines().find(|l| l.starts_with("id:")).unwrap();
    assert_ne!(id1, id2, "each frame must carry a fresh UUID");
}

// ── SseStreamBuilder::format_event ───────────────────────────────────────

#[test]
fn format_event_embeds_event_type() {
    let event = SseEvent {
        user_id: 1,
        event_type: "new_message".to_string(),
        data: serde_json::json!({ "content": "hi" }),
        timestamp: 1000,
    };
    let frame = SseStreamBuilder::format_event(&event);
    assert!(frame.contains("event: new_message"), "frame: {}", frame);
}

#[test]
fn format_event_ends_with_double_newline() {
    let event = SseEvent {
        user_id: 1,
        event_type: "ping".to_string(),
        data: serde_json::json!({}),
        timestamp: 0,
    };
    let frame = SseStreamBuilder::format_event(&event);
    assert!(frame.ends_with("\n\n"));
}

// ── SseManager — channel creation and reuse ───────────────────────────────

#[tokio::test]
async fn get_channel_creates_channel_on_first_call() {
    let manager = SseManager::new();
    let _tx = manager.get_channel(1).await;
    let channels = manager.channels.read().await;
    assert!(channels.contains_key(&1));
}

#[tokio::test]
async fn get_channel_same_user_returns_same_channel() {
    let manager = SseManager::new();
    let tx1 = manager.get_channel(1).await;
    let tx2 = manager.get_channel(1).await;
    // If they share the same channel, subscribing on one is visible on the other.
    assert_eq!(
        tx1.receiver_count(),
        tx2.receiver_count(),
        "both handles must reflect the same underlying channel"
    );
}

#[tokio::test]
async fn get_channel_different_users_are_isolated() {
    let manager = SseManager::new();
    let tx1 = manager.get_channel(1).await;
    let tx2 = manager.get_channel(2).await;
    // Subscribe on user-1's channel; user-2's count should be unaffected.
    let _rx = tx1.subscribe();
    assert_eq!(tx1.receiver_count(), 1);
    assert_eq!(tx2.receiver_count(), 0);
}

// ── SseManager — broadcast_to_user ────────────────────────────────────────

#[tokio::test]
async fn broadcast_to_user_returns_subscriber_count() {
    let manager = SseManager::new();
    let tx = manager.get_channel(1).await;
    let _rx1 = tx.subscribe();
    let _rx2 = tx.subscribe();

    let event = SseEvent {
        user_id: 1,
        event_type: "test".to_string(),
        data: serde_json::json!({}),
        timestamp: 0,
    };
    let count = manager.broadcast_to_user(event).await.unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn broadcast_to_user_no_channel_returns_zero() {
    let manager = SseManager::new();
    let event = SseEvent {
        user_id: 99,
        event_type: "test".to_string(),
        data: serde_json::json!({}),
        timestamp: 0,
    };
    let count = manager.broadcast_to_user(event).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn broadcast_to_user_delivers_event_data() {
    let manager = SseManager::new();
    let tx = manager.get_channel(1).await;
    let mut rx = tx.subscribe();

    let payload = serde_json::json!({ "text": "hello", "id": 42 });
    let event = SseEvent {
        user_id: 1,
        event_type: "message".to_string(),
        data: payload.clone(),
        timestamp: 9999,
    };
    manager.broadcast_to_user(event).await.unwrap();

    let received = rx.recv().await.unwrap();
    assert_eq!(received.data, payload);
    assert_eq!(received.event_type, "message");
    assert_eq!(received.user_id, 1);
}

// ── SseManager — broadcast_to_users ──────────────────────────────────────

#[tokio::test]
async fn broadcast_to_users_stamps_recipient_user_id() {
    // broadcast_to_users must rewrite user_id on each copy to the recipient,
    // not leave the original sender's user_id on every frame.
    let manager = SseManager::new();

    let tx1 = manager.get_channel(10).await;
    let tx2 = manager.get_channel(20).await;
    let mut rx1 = tx1.subscribe();
    let mut rx2 = tx2.subscribe();

    let event = SseEvent {
        user_id: 99, // original sender — should be replaced on delivery
        event_type: "group_msg".to_string(),
        data: serde_json::json!({}),
        timestamp: 0,
    };
    manager
        .broadcast_to_users(event, vec![10, 20])
        .await
        .unwrap();

    let r1 = rx1.recv().await.unwrap();
    let r2 = rx2.recv().await.unwrap();
    assert_eq!(r1.user_id, 10, "recipient 10 must see their own user_id");
    assert_eq!(r2.user_id, 20, "recipient 20 must see their own user_id");
}

#[tokio::test]
async fn broadcast_to_users_skips_users_with_no_channel() {
    let manager = SseManager::new();
    let tx1 = manager.get_channel(1).await;
    let mut rx1 = tx1.subscribe();
    // user 2 has no channel

    let event = SseEvent {
        user_id: 0,
        event_type: "evt".to_string(),
        data: serde_json::json!({}),
        timestamp: 0,
    };
    // Should not panic or error when user 2 has no channel
    let result = manager.broadcast_to_users(event, vec![1, 2]).await;
    assert!(result.is_ok());
    assert!(rx1.recv().await.is_ok());
}

// ── SseManager — event ordering ───────────────────────────────────────────

#[tokio::test]
async fn events_delivered_in_broadcast_order() {
    let manager = SseManager::new();
    let tx = manager.get_channel(1).await;
    let mut rx = tx.subscribe();

    for i in 0u64..5 {
        manager
            .broadcast_to_user(SseEvent {
                user_id: 1,
                event_type: format!("evt_{}", i),
                data: serde_json::json!({ "seq": i }),
                timestamp: i as i64,
            })
            .await
            .unwrap();
    }

    for i in 0u64..5 {
        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_type, format!("evt_{}", i));
    }
}

// ── SseManager — cleanup ─────────────────────────────────────────────────

#[tokio::test]
async fn cleanup_removes_channels_with_no_receivers() {
    let manager = SseManager::new();

    // user 1: no subscriber → should be cleaned up
    let _tx1 = manager.get_channel(1).await;

    // user 2: keeps a live subscriber → must survive cleanup
    let tx2 = manager.get_channel(2).await;
    let _rx2 = tx2.subscribe();

    manager.cleanup().await;

    let channels = manager.channels.read().await;
    assert!(!channels.contains_key(&1), "user 1 should be removed");
    assert!(channels.contains_key(&2), "user 2 should remain");
}

#[tokio::test]
async fn cleanup_does_not_affect_active_channels() {
    let manager = SseManager::new();
    let tx = manager.get_channel(5).await;
    let _rx = tx.subscribe();

    manager.cleanup().await;
    manager.cleanup().await; // idempotent

    let channels = manager.channels.read().await;
    assert!(channels.contains_key(&5));
}

// ── SseManager — concurrent broadcasts ───────────────────────────────────

#[tokio::test]
async fn concurrent_broadcasts_all_delivered() {
    let manager = std::sync::Arc::new(SseManager::new());
    let tx = manager.get_channel(1).await;
    let mut rx = tx.subscribe();

    let handles: Vec<_> = (0..10u64)
        .map(|i| {
            let m = manager.clone();
            tokio::spawn(async move {
                m.broadcast_to_user(SseEvent {
                    user_id: 1,
                    event_type: "concurrent".to_string(),
                    data: serde_json::json!({ "i": i }),
                    timestamp: i as i64,
                })
                .await
                .unwrap();
            })
        })
        .collect();

    for h in handles {
        h.await.unwrap();
    }

    let mut count = 0;
    while let Ok(Ok(_)) =
        tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await
    {
        count += 1;
    }
    assert_eq!(count, 10);
}
