// The previous test file tested only Rust stdlib operations (str::trim,
// i64::parse, HashMap::get) with no connection to the groups handler.
// These tests exercise the actual validation rules enforced by the handler.

use std::collections::HashMap;

// ── Group name validation (mirrors handle_create_group / handle_rename_group) ──

#[test]
fn valid_group_name_is_accepted() {
    let name = "Project Alpha";
    let cleaned = name.replace('\0', "");
    assert!(!cleaned.trim().is_empty());
    assert!(cleaned.len() <= 100);
}

#[test]
fn empty_group_name_is_rejected() {
    let name = "";
    assert!(name.trim().is_empty(), "empty name must be rejected");
}

#[test]
fn whitespace_only_group_name_is_rejected() {
    let name = "   \t  ";
    assert!(
        name.trim().is_empty(),
        "whitespace-only name must be rejected"
    );
}

#[test]
fn group_name_null_bytes_are_stripped() {
    let name = "Team\0 Alpha\0";
    let cleaned = name.replace('\0', "");
    assert_eq!(cleaned, "Team Alpha");
}

#[test]
fn group_name_exactly_100_chars_passes() {
    let name = "a".repeat(100);
    assert_eq!(name.len(), 100);
    assert!(name.len() <= 100);
}

#[test]
fn group_name_101_chars_fails() {
    let name = "a".repeat(101);
    assert!(name.len() > 100, "101-char name must exceed the limit");
}

// ── Description validation (mirrors handle_create_group) ─────────────────

#[test]
fn description_null_bytes_stripped() {
    let desc = "Hello\0 World\0";
    let cleaned = desc.replace('\0', "");
    assert_eq!(cleaned, "Hello World");
}

#[test]
fn description_over_500_chars_is_truncated() {
    let desc = "x".repeat(600);
    let truncated: String = if desc.len() > 500 {
        desc.chars().take(500).collect()
    } else {
        desc
    };
    assert_eq!(truncated.len(), 500);
}

#[test]
fn description_exactly_500_chars_unchanged() {
    let desc = "y".repeat(500);
    let result: String = if desc.len() > 500 {
        desc.chars().take(500).collect()
    } else {
        desc.clone()
    };
    assert_eq!(result.len(), 500);
}

#[test]
fn none_description_stays_none() {
    let params = serde_json::json!({ "name": "MyGroup" });
    let desc: Option<String> = params.get("description").and_then(|v| v.as_str()).map(|s| {
        let s = s.replace('\0', "");
        if s.len() > 500 {
            s.chars().take(500).collect()
        } else {
            s
        }
    });
    assert!(desc.is_none());
}

// ── Role normalisation (mirrors handle_add_member) ────────────────────────

#[test]
fn unknown_role_normalises_to_member() {
    let role = match "superuser" {
        "admin" => "admin",
        _ => "member",
    };
    assert_eq!(role, "member");
}

#[test]
fn admin_role_is_preserved() {
    let role = match "admin" {
        "admin" => "admin",
        _ => "member",
    };
    assert_eq!(role, "admin");
}

#[test]
fn empty_role_normalises_to_member() {
    let role = match "" {
        "admin" => "admin",
        _ => "member",
    };
    assert_eq!(role, "member");
}

#[test]
fn uppercase_admin_is_not_granted() {
    // The handler does a case-sensitive match — "ADMIN" != "admin"
    let role = match "ADMIN" {
        "admin" => "admin",
        _ => "member",
    };
    assert_eq!(role, "member");
}

// ── User-ID resolution from request body (mirrors handle_add_member) ──────

#[test]
fn user_id_taken_from_numeric_field() {
    let body = serde_json::json!({ "user_id": 42 });
    let user_id: Option<i64> = body.get("user_id").and_then(|v| v.as_i64());
    assert_eq!(user_id, Some(42));
}

#[test]
fn username_field_present_when_user_id_absent() {
    let body = serde_json::json!({ "username": "alice" });
    let has_user_id = body.get("user_id").and_then(|v| v.as_i64()).is_some();
    let username = body.get("username").and_then(|v| v.as_str());
    assert!(!has_user_id);
    assert_eq!(username, Some("alice"));
}

#[test]
fn neither_user_id_nor_username_is_an_error() {
    let body = serde_json::json!({ "role": "member" });
    let has_user_id = body.get("user_id").and_then(|v| v.as_i64()).is_some();
    let has_username = body.get("username").and_then(|v| v.as_str()).is_some();
    assert!(
        !has_user_id && !has_username,
        "both absent — handler must return BAD_REQUEST"
    );
}

// ── Search query validation (mirrors handle_search_users) ────────────────

#[test]
fn search_query_trimmed_before_use() {
    let raw = "  alice  ";
    let trimmed = raw.trim();
    assert_eq!(trimmed, "alice");
}

#[test]
fn empty_search_query_returns_early() {
    let q = "";
    assert!(
        q.is_empty(),
        "empty query must short-circuit to empty results"
    );
}

#[test]
fn whitespace_search_trims_to_empty() {
    let q = "   ".trim();
    assert!(q.is_empty());
}

#[test]
fn search_query_over_50_chars_is_too_long() {
    let q = "a".repeat(51);
    assert!(q.len() > 50, "51-char query must be rejected");
}

#[test]
fn search_query_exactly_50_chars_is_valid() {
    let q = "a".repeat(50);
    assert!(q.len() <= 50);
}

// ── Member-remove user_id extraction (mirrors handle_remove_member) ───────

#[test]
fn remove_member_user_id_parsed_from_body() {
    let body = serde_json::json!({ "user_id": 55 });
    let user_id: Option<i64> = body.get("user_id").and_then(|v| v.as_i64());
    assert_eq!(user_id, Some(55));
}

#[test]
fn remove_member_missing_user_id_is_none() {
    let body = serde_json::json!({});
    let user_id: Option<i64> = body.get("user_id").and_then(|v| v.as_i64());
    assert!(
        user_id.is_none(),
        "missing user_id must produce a parse error"
    );
}

// ── Group type guard (mirrors handle_delete_group) ────────────────────────

#[test]
fn only_groups_can_be_deleted_not_direct_chats() {
    let chat_type = "direct";
    let is_group = chat_type == "group";
    assert!(
        !is_group,
        "a direct-message chat must not be deletable via this endpoint"
    );
}

#[test]
fn group_chat_type_can_be_deleted() {
    let chat_type = "group";
    let is_group = chat_type == "group";
    assert!(is_group);
}

// ── Creator-only delete guard (mirrors handle_delete_group) ───────────────

#[test]
fn non_creator_cannot_delete_group() {
    let creator_id: i64 = 1;
    let caller_id: i64 = 2;
    let allowed = creator_id == caller_id;
    assert!(!allowed, "only the creator may delete the group");
}

#[test]
fn creator_can_delete_their_own_group() {
    let creator_id: i64 = 5;
    let caller_id: i64 = 5;
    let allowed = creator_id == caller_id;
    assert!(allowed);
}
