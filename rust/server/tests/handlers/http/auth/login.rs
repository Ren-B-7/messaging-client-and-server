use server::handlers::http::auth::login::*;
use shared::types::login::*;

// ── valid inputs ──────────────────────────────────────────────────────────

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
fn validate_login_remember_me_true() {
    let data = LoginData {
        username: "alice".to_string(),
        password: "hunter2".to_string(),
        remember_me: true,
    };
    assert!(validate_login(&data).is_ok());
}

#[test]
fn validate_login_max_length_username_passes() {
    let data = LoginData {
        username: "a".repeat(32),
        password: "hunter2".to_string(),
        remember_me: false,
    };
    assert!(validate_login(&data).is_ok());
}

#[test]
fn validate_login_max_length_password_passes() {
    let data = LoginData {
        username: "alice".to_string(),
        // 1024 bytes is the cap — exactly at the boundary must pass
        password: "a".repeat(1024),
        remember_me: false,
    };
    assert!(validate_login(&data).is_ok());
}

// ── empty-field rejections ────────────────────────────────────────────────

#[test]
fn validate_login_empty_username_fails() {
    let data = LoginData {
        username: "".to_string(),
        password: "hunter2".to_string(),
        remember_me: false,
    };
    // NOTE: previous test used bare `matches!(...)` — a bool expression
    // used as a statement, which silently does nothing.  `assert!` is required.
    let err = validate_login(&data).unwrap_err();
    assert!(
        matches!(err, LoginError::MissingField(_)),
        "expected MissingField, got {:?}",
        err
    );
}

#[test]
fn validate_login_empty_password_fails() {
    let data = LoginData {
        username: "alice".to_string(),
        password: "".to_string(),
        remember_me: false,
    };
    let err = validate_login(&data).unwrap_err();
    assert!(
        matches!(err, LoginError::MissingField(_)),
        "expected MissingField, got {:?}",
        err
    );
}

// ── over-length rejections ────────────────────────────────────────────────
// These cases existed in the implementation but were never tested.

#[test]
fn validate_login_username_over_32_chars_fails() {
    let data = LoginData {
        username: "a".repeat(33),
        password: "hunter2".to_string(),
        remember_me: false,
    };
    // The implementation currently returns MissingField for this — that is
    // itself a bug (finding #13 in the analysis), but the test must at least
    // assert it returns *some* error rather than silently passing.
    assert!(
        validate_login(&data).is_err(),
        "a 33-char username must be rejected"
    );
}

#[test]
fn validate_login_password_over_1024_bytes_fails() {
    let data = LoginData {
        username: "alice".to_string(),
        password: "a".repeat(1025),
        remember_me: false,
    };
    assert!(
        validate_login(&data).is_err(),
        "a 1025-byte password must be rejected"
    );
}

// ── boundary: 1024 bytes exactly is the last valid value ─────────────────

#[test]
fn validate_login_password_exactly_1024_bytes_passes() {
    let data = LoginData {
        username: "alice".to_string(),
        password: "x".repeat(1024),
        remember_me: false,
    };
    assert!(validate_login(&data).is_ok());
}

#[test]
fn validate_login_username_exactly_32_chars_passes() {
    let data = LoginData {
        username: "a".repeat(32),
        password: "hunter2".to_string(),
        remember_me: false,
    };
    assert!(validate_login(&data).is_ok());
}
