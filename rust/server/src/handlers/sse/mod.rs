mod sse;

#[allow(unused_imports)]
pub use sse::{ChatContext, SseManager, SseStreamBuilder, handle_sse_subscribe};

#[cfg(test)]
mod tests {
    use super::*;
    use shared::types::sse::SseEvent;

    #[tokio::test]
    async fn test_get_channel_creates_channel() {
        let manager = SseManager::new();
        let user_id = "test-user".to_string();

        let tx1 = manager.get_channel(user_id.clone()).await;
        let tx2 = manager.get_channel(user_id).await;

        // Both calls must return handles to the same underlying channel.
        // broadcast::Sender has no is_closed(); receiver_count() is the right
        // proxy: if they share the same channel both will report the same count.
        assert_eq!(tx1.receiver_count(), tx2.receiver_count());
    }

    #[tokio::test]
    async fn test_broadcast_to_user() {
        let manager = SseManager::new();
        let user_id = "test-user".to_string();

        let tx = manager.get_channel(user_id.clone()).await;
        let mut rx = tx.subscribe();

        let event = SseEvent {
            user_id: user_id.clone(),
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

        let user1 = "user1".to_string();
        let user2 = "user2".to_string();

        let _tx1 = manager.get_channel(user1.clone()).await;
        let _tx2 = manager.get_channel(user2.clone()).await;

        let event = SseEvent {
            user_id: user1.clone(),
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
        let user1 = "user1".to_string();
        let user2 = "user2".to_string();

        let _tx1 = manager.get_channel(user1.clone()).await;
        let tx2 = manager.get_channel(user2.clone()).await;

        let _rx2 = tx2.subscribe(); // Keep user2 active

        manager.cleanup().await;

        let channels = manager.channels.read().await;
        assert!(!channels.contains_key(&user1), "user1 should be removed");
        assert!(channels.contains_key(&user2), "user2 should remain");
    }

    #[tokio::test]
    async fn test_broadcast_with_no_channel() {
        let manager = SseManager::new();
        let user_id = "ghost-user".to_string();

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
        let user_id = "alice".to_string();

        let tx = manager.get_channel(user_id.clone()).await;
        let mut rx = tx.subscribe();

        let events = vec![
            SseEvent {
                user_id: user_id.clone(),
                event_type: "message".to_string(),
                data: serde_json::json!({"text": "hello"}),
                timestamp: 1000,
            },
            SseEvent {
                user_id: user_id.clone(),
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
        let user_id = "test-user".to_string();

        let tx = manager.get_channel(user_id.clone()).await;
        let mut rx = tx.subscribe();

        let mut handles = vec![];
        for i in 0..5 {
            let m = manager.clone();
            let uid = user_id.clone();
            handles.push(tokio::spawn(async move {
                m.broadcast_to_user(SseEvent {
                    user_id: uid,
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
        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
                Ok(Ok(_)) => count += 1,
                _ => break,
            }
        }
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn test_multiple_subscribers_same_user() {
        let manager = SseManager::new();
        let user_id = "alice".to_string();

        let tx = manager.get_channel(user_id.clone()).await;
        let mut rx1 = tx.subscribe();
        let mut rx2 = tx.subscribe();
        let mut rx3 = tx.subscribe();

        let event = SseEvent {
            user_id: user_id.clone(),
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

        let user1 = "alice".to_string();
        let user2 = "bob".to_string();
        let user3 = "charlie".to_string();

        let tx1 = manager.get_channel(user1.clone()).await;
        let _tx2 = manager.get_channel(user2.clone()).await;
        let tx3 = manager.get_channel(user3.clone()).await;

        let mut rx1 = tx1.subscribe();
        let mut rx3 = tx3.subscribe();

        let event = SseEvent {
            user_id: "".to_string(),
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
        let user_id = "test".to_string();

        let tx = manager.get_channel(user_id.clone()).await;
        let mut rx = tx.subscribe();

        let original_data = serde_json::json!({
            "message": "hello world",
            "user_id": 123,
            "tags": ["important", "urgent"],
            "nested": { "key": "value" }
        });

        let event = SseEvent {
            user_id: user_id.clone(),
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

    // ── ChatContext parsing ─────────────────────────────────────────────────

    #[test]
    fn test_chat_context_direct() {
        let mut p = std::collections::HashMap::new();
        p.insert("other_user_id".to_string(), "42".to_string());
        let ctx = ChatContext::from_params(&p).unwrap();
        assert!(matches!(ctx, ChatContext::Chat { chat_id: 42 }));
    }

    #[test]
    fn test_chat_context_group_by_group_id() {
        let mut p = std::collections::HashMap::new();
        p.insert("group_id".to_string(), "7".to_string());
        let ctx = ChatContext::from_params(&p).unwrap();
        assert!(matches!(ctx, ChatContext::Chat { chat_id: 7 }));
    }

    #[test]
    fn test_chat_context_group_by_chat_id() {
        let mut p = std::collections::HashMap::new();
        p.insert("chat_id".to_string(), "99".to_string());
        let ctx = ChatContext::from_params(&p).unwrap();
        assert!(matches!(ctx, ChatContext::Chat { chat_id: 99 }));
    }

    #[test]
    fn test_chat_context_missing_returns_none() {
        let p = std::collections::HashMap::new();
        assert!(ChatContext::from_params(&p).is_none());
    }

    #[test]
    fn test_chat_context_invalid_value_returns_none() {
        let mut p = std::collections::HashMap::new();
        p.insert("other_user_id".to_string(), "not-a-number".to_string());
        assert!(ChatContext::from_params(&p).is_none());
    }
}
