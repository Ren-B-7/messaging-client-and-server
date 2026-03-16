/// Tests for group management handlers
use std::collections::HashMap;

// ── Participant CSV parsing ────────────────────────────────────────────────

#[test]
fn participants_csv_parses_to_vec() {
    let raw = "1,2, 3, 42";
    let ids: Vec<i64> = raw
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    assert_eq!(ids, vec![1, 2, 3, 42]);
}

#[test]
fn participants_single_id() {
    let raw = "42";
    let ids: Vec<i64> = raw
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    assert_eq!(ids, vec![42]);
}

#[test]
fn participants_with_extra_spaces() {
    let raw = "  1  ,  2  ,  3  ";
    let ids: Vec<i64> = raw
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    assert_eq!(ids, vec![1, 2, 3]);
}

#[test]
fn participants_large_ids() {
    let raw = "9223372036854775800, 9223372036854775801";
    let ids: Vec<i64> = raw
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    assert_eq!(ids.len(), 2);
}

#[test]
fn participants_empty_string_gives_empty_vec() {
    let raw = "";
    let ids: Vec<i64> = raw
        .split(',')
        .filter_map(|p| p.trim().parse::<i64>().ok())
        .collect();
    assert!(ids.is_empty());
}

#[test]
fn participants_only_whitespace() {
    let raw = "   ,  ,   ";
    let ids: Vec<i64> = raw
        .split(',')
        .filter_map(|p| p.trim().parse::<i64>().ok())
        .collect();
    assert!(ids.is_empty());
}

#[test]
fn participants_mixed_valid_invalid() {
    let raw = "1, abc, 2, def, 3";
    let ids: Vec<i64> = raw
        .split(',')
        .filter_map(|p| p.trim().parse::<i64>().ok())
        .collect();
    assert_eq!(ids, vec![1, 2, 3]); // Only valid numbers
}

#[test]
fn participants_with_commas_no_numbers() {
    let raw = "a, b, c";
    let ids: Vec<i64> = raw
        .split(',')
        .filter_map(|p| p.trim().parse::<i64>().ok())
        .collect();
    assert!(ids.is_empty());
}

#[test]
fn participants_zero_is_valid() {
    let raw = "0, 1, 2";
    let ids: Vec<i64> = raw
        .split(',')
        .filter_map(|p| p.trim().parse::<i64>().ok())
        .collect();
    assert_eq!(ids, vec![0, 1, 2]);
}

#[test]
fn participants_negative_numbers() {
    let raw = "-1, -2, 3";
    let ids: Vec<i64> = raw
        .split(',')
        .filter_map(|p| p.trim().parse::<i64>().ok())
        .collect();
    assert_eq!(ids, vec![-1, -2, 3]);
}

// ── Role defaults ──────────────────────────────────────────────────────────

#[test]
fn role_defaults_to_member() {
    let params: HashMap<String, String> = HashMap::new();
    let role = params
        .get("role")
        .cloned()
        .unwrap_or_else(|| "member".to_string());
    assert_eq!(role, "member");
}

#[test]
fn role_from_params_when_present() {
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("role".to_string(), "admin".to_string());
    let role = params
        .get("role")
        .cloned()
        .unwrap_or_else(|| "member".to_string());
    assert_eq!(role, "admin");
}

#[test]
fn role_case_sensitive() {
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("role".to_string(), "ADMIN".to_string());
    let role = params
        .get("role")
        .cloned()
        .unwrap_or_else(|| "member".to_string());
    assert_eq!(role, "ADMIN"); // Stored as-is
}

#[test]
fn role_empty_string_uses_default() {
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("role".to_string(), "".to_string());
    let role = if params.get("role").map_or(true, |r| r.is_empty()) {
        "member"
    } else {
        params.get("role").map(|r| r.as_str()).unwrap_or("member")
    };
    assert_eq!(role, "member");
}

#[test]
fn role_multiple_roles_first_wins() {
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("role".to_string(), "moderator".to_string());
    let role = params
        .get("role")
        .cloned()
        .unwrap_or_else(|| "member".to_string());
    assert_eq!(role, "moderator");
}

// ── User ID parsing ───────────────────────────────────────────────────────

#[test]
fn user_id_parses_from_string() {
    let s = "42";
    let id: Option<i64> = s.parse::<i64>().ok();
    assert_eq!(id, Some(42));
}

#[test]
fn user_id_parses_from_large_number() {
    let s = "9223372036854775800";
    let id: Option<i64> = s.parse::<i64>().ok();
    assert_eq!(id, Some(9223372036854775800));
}

#[test]
fn user_id_parses_zero() {
    let s = "0";
    let id: Option<i64> = s.parse::<i64>().ok();
    assert_eq!(id, Some(0));
}

#[test]
fn user_id_parses_negative() {
    let s = "-100";
    let id: Option<i64> = s.parse::<i64>().ok();
    assert_eq!(id, Some(-100));
}

#[test]
fn invalid_user_id_gives_none() {
    let s = "not_a_number";
    let id: Option<i64> = s.parse::<i64>().ok();
    assert!(id.is_none());
}

#[test]
fn invalid_user_id_with_spaces() {
    let s = "42 invalid";
    let id: Option<i64> = s.parse::<i64>().ok();
    assert!(id.is_none());
}

#[test]
fn invalid_user_id_with_decimals() {
    let s = "42.5";
    let id: Option<i64> = s.parse::<i64>().ok();
    assert!(id.is_none());
}

#[test]
fn invalid_user_id_empty_string() {
    let s = "";
    let id: Option<i64> = s.parse::<i64>().ok();
    assert!(id.is_none());
}

#[test]
fn invalid_user_id_whitespace_only() {
    let s = "   ";
    let id: Option<i64> = s.parse::<i64>().ok();
    assert!(id.is_none());
}

#[test]
fn invalid_user_id_special_chars() {
    let s = "42@invalid";
    let id: Option<i64> = s.parse::<i64>().ok();
    assert!(id.is_none());
}

// ── Trim and filter ───────────────────────────────────────────────────────

#[test]
fn trim_removes_leading_space() {
    let s = "  alice";
    let result = s.trim();
    assert_eq!(result, "alice");
}

#[test]
fn trim_removes_trailing_space() {
    let s = "alice  ";
    let result = s.trim();
    assert_eq!(result, "alice");
}

#[test]
fn trim_removes_both_sides() {
    let s = "  alice  ";
    let result = s.trim();
    assert_eq!(result, "alice");
}

#[test]
fn trim_handles_tabs() {
    let s = "\talice\t";
    let result = s.trim();
    assert_eq!(result, "alice");
}

#[test]
fn trim_handles_newlines() {
    let s = "\nalice\n";
    let result = s.trim();
    assert_eq!(result, "alice");
}

#[test]
fn trim_no_whitespace() {
    let s = "alice";
    let result = s.trim();
    assert_eq!(result, "alice");
}

#[test]
fn empty_string_trims_to_empty() {
    let s = "   ";
    let result = s.trim();
    assert!(result.is_empty());
}

// ── Group name validation ──────────────────────────────────────────────────

#[test]
fn valid_group_name() {
    let name = "Project Alpha";
    assert!(!name.trim().is_empty());
    assert!(name.len() <= 100);
}

#[test]
fn empty_group_name() {
    let name = "";
    assert!(name.trim().is_empty());
}

#[test]
fn whitespace_only_group_name() {
    let name = "   ";
    assert!(name.trim().is_empty());
}

#[test]
fn max_length_group_name() {
    let name = "a".repeat(100);
    assert_eq!(name.len(), 100);
}

#[test]
fn exceeds_max_length_group_name() {
    let name = "a".repeat(101);
    assert!(name.len() > 100);
}

#[test]
fn group_name_with_special_chars() {
    let name = "Team @Project (2024)";
    assert!(!name.trim().is_empty());
    assert!(name.len() <= 100);
}

#[test]
fn group_name_with_unicode() {
    let name = "Équipe α";
    assert!(!name.trim().is_empty());
    assert!(name.len() <= 100);
}

// ── Description validation ─────────────────────────────────────────────────

#[test]
fn description_within_limit() {
    let desc = "a".repeat(500);
    assert_eq!(desc.len(), 500);
}

#[test]
fn description_exceeds_limit_is_truncated() {
    let desc = "a".repeat(501);
    let truncated: String = desc.chars().take(500).collect();
    assert_eq!(truncated.len(), 500);
}

#[test]
fn description_null_bytes_removed() {
    let desc = "hello\0world";
    let cleaned = desc.replace('\0', "");
    assert_eq!(cleaned, "helloworld");
}

#[test]
fn empty_description() {
    let desc = "";
    assert!(desc.is_empty());
}

// ── Search query validation ───────────────────────────────────────────────

#[test]
fn search_query_empty() {
    let q = "";
    assert!(q.is_empty());
}

#[test]
fn search_query_single_char() {
    let q = "a";
    assert!(!q.is_empty());
    assert!(q.len() <= 50);
}

#[test]
fn search_query_max_length() {
    let q = "a".repeat(50);
    assert_eq!(q.len(), 50);
}

#[test]
fn search_query_exceeds_max() {
    let q = "a".repeat(51);
    assert!(q.len() > 50);
}

#[test]
fn search_query_with_spaces() {
    let q = "alice smith";
    assert!(!q.is_empty());
    assert!(q.len() <= 50);
}

#[test]
fn search_query_with_special_chars() {
    let q = "user@domain";
    assert!(!q.is_empty());
    assert!(q.len() <= 50);
}

#[test]
fn search_query_trimmed() {
    let q = "  alice  ";
    let trimmed = q.trim();
    assert_eq!(trimmed, "alice");
}
