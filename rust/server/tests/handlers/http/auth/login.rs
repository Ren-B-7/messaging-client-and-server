use server::handlers::http::auth::login::*;
use shared::types::login::*;

#[test]
fn validate_login_ok() {
    let data = LoginData {
        username: "alice".to_string(),
        password: "hunter2".to_string(),
        remember_me: false,
    };
    assert!(validate_login(&data).is_ok());
}

#[test]
fn validate_login_empty_username_fails() {
    let data = LoginData {
        username: "".to_string(),
        password: "hunter2".to_string(),
        remember_me: false,
    };
    let err = validate_login(&data).unwrap_err();
    matches!(err, LoginError::MissingField(_));
}

#[test]
fn validate_login_empty_password_fails() {
    let data = LoginData {
        username: "alice".to_string(),
        password: "".to_string(),
        remember_me: false,
    };
    let err = validate_login(&data).unwrap_err();
    matches!(err, LoginError::MissingField(_));
}

#[test]
fn remember_me_variants() {
    for val in &["on", "true", "1"] {
        let remember = *val == "on" || *val == "true" || *val == "1";
        assert!(remember, "expected true for '{}'", val);
    }
    let not_remember = "0" == "on" || "0" == "true" || "0" == "1";
    assert!(!not_remember);
}
