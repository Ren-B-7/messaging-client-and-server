mod sse;

pub use sse::{SseManager, handle_sse_subscribe, SseStreamBuilder};

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

        // Both should reference same channel
        assert_eq!(tx1.is_closed(), tx2.is_closed());
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
        assert_eq!(result.unwrap(), 1); // One subscriber

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

        let _rx2 = tx2.subscribe(); // Keep user2's channel active

        manager.cleanup().await;

        // user1 should be removed, user2 should remain
        let channels = manager.channels.read().await;
        assert!(!channels.contains_key(&user1));
        assert!(channels.contains_key(&user2));
    }

    #[tokio::test]
    async fn test_broadcast_with_no_subscribers() {
        let manager = SseManager::new();
        let user_id = "test-user".to_string();

        let event = SseEvent {
            user_id,
            event_type: "test".to_string(),
            data: serde_json::json!({}),
            timestamp: 0,
        };

        let result = manager.broadcast_to_user(event).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // No subscribers
    }

    #[tokio::test]
    async fn test_channel_event_streaming() {
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

        // Send all events
        for event in events.iter() {
            let _ = manager.broadcast_to_user(event.clone()).await;
        }

        // Receive all events
        let received1 = rx.recv().await.unwrap();
        let received2 = rx.recv().await.unwrap();

        assert_eq!(received1.event_type, "message");
        assert_eq!(received2.event_type, "typing");
    }

    #[tokio::test]
    async fn test_concurrent_broadcasts() {
        let manager = std::sync::Arc::new(SseManager::new());
        let user_id = "test-user".to_string();

        let tx = manager.get_channel(user_id.clone()).await;
        let mut rx = tx.subscribe();

        // Spawn multiple tasks broadcasting events
        let mut handles = vec![];

        for i in 0..5 {
            let manager_clone = manager.clone();
            let user_id_clone = user_id.clone();

            let handle = tokio::spawn(async move {
                let event = SseEvent {
                    user_id: user_id_clone,
                    event_type: "concurrent".to_string(),
                    data: serde_json::json!({"index": i}),
                    timestamp: i as i64,
                };

                manager_clone.broadcast_to_user(event).await
            });

            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            let _ = handle.await;
        }

        // Should receive all 5 events
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

        let result = manager.broadcast_to_user(event.clone()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3); // Three subscribers

        // All subscribers should receive the event
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

        // Only subscribe user1 and user3
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

        // Broadcast to all three
        manager
            .broadcast_to_users(event, vec![user1, user2, user3])
            .await
            .unwrap();

        // user1 and user3 should receive (user2 has no subscribers, but that's ok)
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
            "nested": {
                "key": "value"
            }
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
}
