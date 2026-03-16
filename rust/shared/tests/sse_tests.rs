use shared::types::sse::*;

#[test]
fn sse_event_serializes_and_deserializes() {
    let e = SseEvent {
        user_id: "42".into(),
        event_type: "message_sent".into(),
        data: serde_json::json!({ "msg_id": 1 }),
        timestamp: 1234,
    };
    let json = serde_json::to_string(&e).unwrap();
    let back: SseEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(back.user_id, "42");
    assert_eq!(back.event_type, "message_sent");
}

#[test]
fn sse_error_channel_send_failed_display() {
    let e = SseError::ChannelSendFailed("test error".into());
    let out = format!("{}", e);
    assert!(out.contains("test error"));
}

#[test]
fn sse_error_channel_closed_display() {
    let e = SseError::ChannelClosed;
    let out = format!("{}", e);
    assert!(out.to_lowercase().contains("closed"));
}
