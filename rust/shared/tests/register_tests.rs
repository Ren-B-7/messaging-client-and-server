use shared::types::register::*;

#[test]
fn all_register_error_codes_are_non_empty() {
    let errors: Vec<Box<dyn Fn() -> RegisterError>> = vec![
        Box::new(|| RegisterError::UsernameTaken),
        Box::new(|| RegisterError::EmailTaken),
        Box::new(|| RegisterError::DatabaseError),
    ];
    for e in errors {
        let err = e();
        assert!(!err.to_code().is_empty());
    }
}

#[test]
fn register_data_deserializes_from_json() {
    let json = r#"{
        "username": "bob",
        "password": "Pass1234",
        "confirm_password": "Pass1234",
        "email": "bob@example.com"
    }"#;
    let d: RegisterData = serde_json::from_str(json).unwrap();
    assert_eq!(d.username, "bob");
    assert_eq!(d.email, Some("bob@example.com".into()));
}
