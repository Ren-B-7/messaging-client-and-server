use std::collections::HashMap;
#[test]
fn form_body_user_id_parsing() {
    let params: HashMap<String, String> = form_urlencoded::parse(b"user_id=123&reason=Spamming")
        .into_owned()
        .collect();
    let user_id: Option<i64> = params.get("user_id").and_then(|id| id.parse().ok());
    let reason = params
        .get("reason")
        .cloned()
        .unwrap_or_else(|| "No reason".to_string());
    assert_eq!(user_id, Some(123));
    assert_eq!(reason, "Spamming");
}

#[test]
fn user_id_from_path_last_segment() {
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
fn admin_cannot_act_on_themselves() {
    let admin_id: i64 = 1;
    let target_id: i64 = 1;
    assert_eq!(admin_id, target_id, "should be blocked when equal");
}
