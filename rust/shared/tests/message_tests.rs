use shared::types::message::*;

#[test]
fn all_message_error_codes_are_unique() {
    let codes = [
        MessageError::Unauthorized.to_code(),
        MessageError::MessageTooLong.to_code(),
        MessageError::EmptyMessage.to_code(),
        MessageError::DatabaseError.to_code(),
        MessageError::InternalError.to_code(),
    ];
    let unique: std::collections::HashSet<_> = codes.iter().collect();
    assert_eq!(codes.len(), unique.len(), "duplicate message error codes");
}

#[test]
fn send_message_data_deserialize() {
    let json = r#"{"chat_id": 5, "content": "hello"}"#;
    let data: SendMessageData = serde_json::from_str(json).unwrap();
    assert_eq!(data.chat_id, 5);
    assert_eq!(data.content, "hello");
}

#[test]
fn message_error_codes() {
    assert_eq!(MessageError::MissingChat.to_code(), "MISSING_CHAT");
    assert_eq!(
        MessageError::NotMemberOfChat.to_code(),
        "NOT_MEMBER_OF_CHAT"
    );
    assert_eq!(MessageError::SenderBanned.to_code(), "SENDER_BANNED");
}

#[test]
fn message_response_serialization() {
    let msg = MessageResponse {
        id: 1,
        sender_id: 42,
        chat_id: 5,
        content: "hello".to_string(),
        sent_at: 1709038211,
        delivered_at: None,
        read_at: None,
        message_type: "text".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""id":1"#));
    assert!(!json.contains("delivered_at"));
}
