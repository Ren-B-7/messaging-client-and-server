use shared::types::update::*;

#[test]
fn profile_error_codes_are_unique() {
    let codes = [
        ProfileError::Unauthorized.to_code(),
        ProfileError::UserNotFound.to_code(),
        ProfileError::DatabaseError.to_code(),
    ];
    let unique: std::collections::HashSet<_> = codes.iter().collect();
    assert_eq!(codes.len(), unique.len());
}

#[test]
fn profile_response_success_has_profile_data() {
    let r = ProfileResponse::Success {
        profile: ProfileData {
            user_id: 1,
            username: "alice".into(),
            email: None,
            created_at: 0,
            last_login: None,
        },
        message: "ok".into(),
    };
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["profile"]["username"], "alice");
}
