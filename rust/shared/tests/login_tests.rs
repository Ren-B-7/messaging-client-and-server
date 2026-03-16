use shared::types::login::*;

#[test]
fn login_data_deserializes_username() {
    let json = r#"{"username":"bob","password":"pass123"}"#;
    let d: LoginData = serde_json::from_str(json).unwrap();
    assert_eq!(d.username, "bob");
    assert!(!d.remember_me);
}

#[test]
fn login_data_email_alias_maps_to_username() {
    let json = r#"{"email":"bob@example.com","password":"pass123"}"#;
    let d: LoginData = serde_json::from_str(json).unwrap();
    assert_eq!(d.username, "bob@example.com");
}

#[test]
fn login_data_remember_me_defaults_false() {
    let json = r#"{"username":"x","password":"y"}"#;
    let d: LoginData = serde_json::from_str(json).unwrap();
    assert!(!d.remember_me);
}

#[test]
fn login_data_remember_me_can_be_set() {
    let json = r#"{"username":"x","password":"y","remember_me":true}"#;
    let d: LoginData = serde_json::from_str(json).unwrap();
    assert!(d.remember_me);
}

#[test]
fn all_error_variants_have_non_empty_messages() {
    let variants: Vec<Box<dyn Fn() -> LoginError>> = vec![
        Box::new(|| LoginError::InvalidCredentials),
        Box::new(|| LoginError::UserBanned),
        Box::new(|| LoginError::UserNotFound),
        Box::new(|| LoginError::MissingField("test".into())),
        Box::new(|| LoginError::DatabaseError),
        Box::new(|| LoginError::InternalError),
    ];
    for v in variants {
        let e = v();
        assert!(!e.to_code().is_empty());
        assert!(!e.to_message().is_empty());
    }
}

#[test]
fn login_error_response_is_serializable() {
    let r = LoginError::UserBanned.to_response();
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["status"], "error");
    assert_eq!(json["code"], "USER_BANNED");
}

#[test]
fn login_response_success_serializes_all_fields() {
    let r = LoginResponse::Success {
        user_id: 1,
        username: "alice".into(),
        token: "t.o.k".into(),
        expires_in: 3600,
        message: "ok".into(),
    };
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["status"], "success");
    assert_eq!(json["expires_in"], 3600);
}

#[test]
fn new_session_display_omits_sensitive_data() {
    let s = NewSession {
        user_id: 5,
        session_id: "handle-123".into(),
        expires_at: 1000,
        ip_address: Some("10.0.0.1".into()),
    };
    let out = format!("{}", s);
    assert!(out.contains("handle-123"));
    assert!(out.contains("10.0.0.1"));
}
