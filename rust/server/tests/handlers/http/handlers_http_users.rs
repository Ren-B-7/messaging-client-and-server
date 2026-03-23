use std::collections::HashMap;

// ── Body parsing helper (mirrors admin users.rs parse_body logic) ─────────

#[test]
fn form_body_user_id_and_reason_parsed() {
    let params: HashMap<String, String> = form_urlencoded::parse(b"user_id=123&reason=Spamming")
        .into_owned()
        .collect();

    let user_id: Option<i64> = params.get("user_id").and_then(|id| id.parse().ok());
    let reason = params
        .get("reason")
        .cloned()
        .unwrap_or_else(|| "No reason provided".to_string());

    assert_eq!(user_id, Some(123));
    assert_eq!(reason, "Spamming");
}

#[test]
fn form_body_missing_reason_uses_default() {
    let params: HashMap<String, String> =
        form_urlencoded::parse(b"user_id=42").into_owned().collect();

    let reason = params
        .get("reason")
        .cloned()
        .unwrap_or_else(|| "No reason provided".to_string());

    assert_eq!(reason, "No reason provided");
}

#[test]
fn form_body_invalid_user_id_is_none() {
    let params: HashMap<String, String> = form_urlencoded::parse(b"user_id=notanumber")
        .into_owned()
        .collect();

    let user_id: Option<i64> = params.get("user_id").and_then(|id| id.parse().ok());
    assert!(user_id.is_none());
}

// ── Path segment ID extraction (mirrors handle_delete_user logic) ─────────

#[test]
fn user_id_from_last_path_segment() {
    let path = "/admin/api/users/42";
    let user_id: Option<i64> = path
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .filter(|s| *s != ":id")
        .and_then(|s| s.parse().ok());
    assert_eq!(user_id, Some(42));
}

#[test]
fn literal_id_placeholder_in_path_returns_none() {
    // The router sometimes produces "/:id" literally — must not parse as a number
    let path = "/admin/api/users/:id";
    let user_id: Option<i64> = path
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .filter(|s| *s != ":id")
        .and_then(|s| s.parse().ok());
    assert!(user_id.is_none());
}

#[test]
fn trailing_slash_on_path_is_ignored() {
    let path = "/admin/api/users/99/";
    let user_id: Option<i64> = path
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .filter(|s| *s != ":id")
        .and_then(|s| s.parse().ok());
    assert_eq!(user_id, Some(99));
}

#[test]
fn non_numeric_last_segment_returns_none() {
    let path = "/admin/api/users/settings";
    let user_id: Option<i64> = path
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .filter(|s| *s != ":id")
        .and_then(|s| s.parse().ok());
    assert!(user_id.is_none());
}

// ── Self-action guard (admin cannot act on their own account) ─────────────

#[test]
fn admin_and_target_equal_must_be_blocked() {
    let admin_id: i64 = 1;
    let target_id: i64 = 1;
    // The guard condition used in delete/promote/demote handlers
    let blocked = admin_id == target_id;
    assert!(blocked, "admin acting on themselves must be rejected");
}

#[test]
fn admin_and_different_target_must_be_allowed() {
    let admin_id: i64 = 1;
    let target_id: i64 = 2;
    let blocked = admin_id == target_id;
    assert!(!blocked, "admin acting on a different user must proceed");
}

// ── Ban reason sanitisation (mirrors handle_ban_user logic) ───────────────

#[test]
fn null_bytes_stripped_from_ban_reason() {
    let raw = "Spamming\0 the\0 chat";
    let cleaned = raw.replace('\0', "");
    assert_eq!(cleaned, "Spamming the chat");
    assert!(!cleaned.contains('\0'));
}

#[test]
fn ban_reason_over_500_chars_is_truncated() {
    let raw = "x".repeat(600);
    let cleaned: String = if raw.len() > 500 {
        raw.chars().take(500).collect()
    } else {
        raw
    };
    assert_eq!(cleaned.len(), 500);
}

#[test]
fn ban_reason_exactly_500_chars_is_unchanged() {
    let raw = "x".repeat(500);
    let cleaned: String = if raw.len() > 500 {
        raw.chars().take(500).collect()
    } else {
        raw.clone()
    };
    assert_eq!(cleaned.len(), 500);
    assert_eq!(cleaned, raw);
}

#[test]
fn empty_ban_reason_stays_empty() {
    let raw = "";
    let cleaned = raw.replace('\0', "");
    assert_eq!(cleaned, "");
}

// ── JSON body user_id extraction ──────────────────────────────────────────

#[test]
fn json_body_user_id_as_number() {
    let body = serde_json::json!({ "user_id": 77 });
    let user_id: Option<i64> = body.get("user_id").and_then(|v| v.as_i64());
    assert_eq!(user_id, Some(77));
}

#[test]
fn json_body_user_id_missing_is_none() {
    let body = serde_json::json!({ "reason": "test" });
    let user_id: Option<i64> = body.get("user_id").and_then(|v| v.as_i64());
    assert!(user_id.is_none());
}

#[test]
fn json_body_user_id_as_string_is_none_via_as_i64() {
    // as_i64() only works for JSON number nodes, not string-encoded numbers
    let body = serde_json::json!({ "user_id": "42" });
    let user_id: Option<i64> = body.get("user_id").and_then(|v| v.as_i64());
    assert!(user_id.is_none());
}
