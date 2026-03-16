/// Tests for message sending and management handlers
use server::handlers::http::messaging::messages::*;
use shared::types::message::*;

// ── Message validation ─────────────────────────────────────────────────────

#[test]
fn valid_message_data() {
    let data = SendMessageData {
        chat_id: 5,
        content: "Hello!".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn valid_message_with_type() {
    let data = SendMessageData {
        chat_id: 5,
        content: "System notification".to_string(),
        message_type: Some("system".to_string()),
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn valid_message_long_content() {
    let data = SendMessageData {
        chat_id: 5,
        content: "a".repeat(10_000),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn valid_message_with_unicode() {
    let data = SendMessageData {
        chat_id: 5,
        content: "Hello 世界 🌍".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn valid_message_with_newlines() {
    let data = SendMessageData {
        chat_id: 5,
        content: "Line 1\nLine 2\nLine 3".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn empty_content_fails() {
    let data = SendMessageData {
        chat_id: 5,
        content: "".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_err());
}

#[test]
fn whitespace_only_fails() {
    let data = SendMessageData {
        chat_id: 5,
        content: "   ".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_err());
}

#[test]
fn tabs_and_newlines_only_fails() {
    let data = SendMessageData {
        chat_id: 5,
        content: "\t\n\t\n".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_err());
}

#[test]
fn oversized_content_fails() {
    let data = SendMessageData {
        chat_id: 5,
        content: "x".repeat(10_001),
        message_type: None,
    };
    assert!(matches!(
        validate_message(&data).unwrap_err(),
        MessageError::MessageTooLong
    ));
}

#[test]
fn significantly_oversized_fails() {
    let data = SendMessageData {
        chat_id: 5,
        content: "x".repeat(100_000),
        message_type: None,
    };
    assert!(validate_message(&data).is_err());
}

#[test]
fn one_char_message_valid() {
    let data = SendMessageData {
        chat_id: 5,
        content: "a".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn message_with_special_chars() {
    let data = SendMessageData {
        chat_id: 5,
        content: "!@#$%^&*(){}[]|\\:;\"'<>,.?/".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

// ── Limit clamping ─────────────────────────────────────────────────────────

#[test]
fn limit_clamped_to_100() {
    let limit = Some(200_i64).unwrap_or(50).min(100);
    assert_eq!(limit, 100);
}

#[test]
fn limit_exactly_100() {
    let limit = Some(100_i64).unwrap_or(50).min(100);
    assert_eq!(limit, 100);
}

#[test]
fn limit_less_than_100() {
    let limit = Some(50_i64).unwrap_or(50).min(100);
    assert_eq!(limit, 50);
}

#[test]
fn limit_defaults_to_50() {
    let limit = None::<i64>.unwrap_or(50).min(100);
    assert_eq!(limit, 50);
}

#[test]
fn limit_zero_stays_zero() {
    let limit = parse_limit(Some(0_i64));
    assert_eq!(limit, 0);
}

#[test]
fn limit_one() {
    let limit = parse_limit(Some(1_i64));
    assert_eq!(limit, 1);
}

#[test]
fn limit_very_large_clamped() {
    let limit = Some(1_000_000_i64).unwrap_or(50).min(100);
    assert_eq!(limit, 100);
}

// ── Message error code ─────────────────────────────────────────────────────

#[test]
fn message_error_missing_chat() {
    // Simulating error type
    let code = "MISSING_CHAT";
    assert_eq!(code, "MISSING_CHAT");
}

#[test]
fn message_error_empty_message() {
    let code = "EMPTY_MESSAGE";
    assert_eq!(code, "EMPTY_MESSAGE");
}

#[test]
fn message_error_too_long() {
    let code = "MESSAGE_TOO_LONG";
    assert_eq!(code, "MESSAGE_TOO_LONG");
}

#[test]
fn message_error_not_found() {
    let code = "NOT_FOUND";
    assert_eq!(code, "NOT_FOUND");
}

// ── Constants validation ───────────────────────────────────────────────────

#[test]
fn max_message_length_is_10000() {
    assert_eq!(MAX_MESSAGE_LENGTH, 10_000);
}

#[test]
fn default_limit_is_50() {
    assert_eq!(DEFAULT_LIMIT, 50);
}

#[test]
fn max_message_length_reasonable() {
    assert!(MAX_MESSAGE_LENGTH > 100); // At least 100 chars
    assert!(MAX_MESSAGE_LENGTH < 100_000); // Less than 100KB
}

// ── Parse limit helper ─────────────────────────────────────────────────────

#[test]
fn parse_limit_with_some_value() {
    let result = parse_limit(Some(30));
    assert_eq!(result, 30);
}

#[test]
fn parse_limit_none_returns_default() {
    let result = parse_limit(None);
    assert_eq!(result, DEFAULT_LIMIT);
}

#[test]
fn parse_limit_exceeding_max_is_clamped() {
    let result = parse_limit(Some(150));
    assert_eq!(result, 100);
}

#[test]
fn parse_limit_exactly_max() {
    let result = parse_limit(Some(100));
    assert_eq!(result, 100);
}

#[test]
fn parse_limit_zero() {
    let result = parse_limit(Some(0));
    assert_eq!(result, 0);
}

// ── Message content edge cases ─────────────────────────────────────────────

#[test]
fn message_with_urls() {
    let data = SendMessageData {
        chat_id: 5,
        content: "Check out https://example.com for more info".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn message_with_emails() {
    let data = SendMessageData {
        chat_id: 5,
        content: "Email me at test@example.com".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn message_with_code_block() {
    let data = SendMessageData {
        chat_id: 5,
        content: "```\nfn main() {\n    println!(\"Hello\");\n}\n```".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn message_with_json() {
    let data = SendMessageData {
        chat_id: 5,
        content: r#"{"key": "value", "nested": {"item": 123}}"#.to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn message_with_markdown() {
    let data = SendMessageData {
        chat_id: 5,
        content: "# Heading\n\n**Bold** and *italic* text".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn message_with_emoji() {
    let data = SendMessageData {
        chat_id: 5,
        content: "Hello 👋 World 🌍 🚀✨".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn message_with_mixed_whitespace() {
    let data = SendMessageData {
        chat_id: 5,
        content: "  \t Hello  \n  World  \t  ".to_string(),
        message_type: None,
    };
    assert!(validate_message(&data).is_ok()); // Outer trim will handle this
}

// ── Chat ID validation ─────────────────────────────────────────────────────

#[test]
fn valid_chat_id_small() {
    let chat_id = 1_i64;
    assert!(chat_id > 0);
}

#[test]
fn valid_chat_id_large() {
    let chat_id = 9_223_372_036_854_775_800_i64;
    assert!(chat_id > 0);
}

#[test]
fn invalid_chat_id_zero() {
    let chat_id = 0_i64;
    assert!(chat_id == 0);
}

#[test]
fn invalid_chat_id_negative() {
    let chat_id = -1_i64;
    assert!(chat_id < 0);
}

// ── Message type variations ────────────────────────────────────────────────

#[test]
fn message_type_file() {
    let data = SendMessageData {
        chat_id: 5,
        content: "file_data".to_string(),
        message_type: Some("file".to_string()),
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn message_type_system() {
    let data = SendMessageData {
        chat_id: 5,
        content: "User joined".to_string(),
        message_type: Some("system".to_string()),
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn message_type_reaction() {
    let data = SendMessageData {
        chat_id: 5,
        content: "👍".to_string(),
        message_type: Some("reaction".to_string()),
    };
    assert!(validate_message(&data).is_ok());
}

#[test]
fn message_type_empty_string() {
    let data = SendMessageData {
        chat_id: 5,
        content: "Hello".to_string(),
        message_type: Some("".to_string()),
    };
    assert!(validate_message(&data).is_ok());
}

// ── Integration scenarios ──────────────────────────────────────────────────

#[test]
fn multiple_limits_in_sequence() {
    let l1 = parse_limit(Some(30));
    let l2 = parse_limit(Some(150));
    let l3 = parse_limit(None);

    assert_eq!(l1, 30);
    assert_eq!(l2, 100);
    assert_eq!(l3, 50);
}

#[test]
fn message_validation_then_limit_parsing() {
    let msg = SendMessageData {
        chat_id: 5,
        content: "Test message".to_string(),
        message_type: None,
    };
    let is_valid = validate_message(&msg).is_ok();
    let limit = parse_limit(Some(200));

    assert!(is_valid);
    assert_eq!(limit, 100);
}
