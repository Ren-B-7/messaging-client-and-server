use server::handlers::http::auth::register::*;

#[test]
fn valid_username_passes() {
    assert!(validate_username("alice_123").is_ok());
    assert!(validate_username("Bob-Smith").is_ok());
    assert!(validate_username("abc").is_ok());
}

#[test]
fn username_too_short_fails() {
    assert!(validate_username("ab").is_err());
    assert!(validate_username("").is_err());
}

#[test]
fn username_too_long_fails() {
    let long: String = "a".repeat(33);
    assert!(validate_username(&long).is_err());
}

#[test]
fn username_invalid_chars_fails() {
    assert!(validate_username("alice!").is_err());
    assert!(validate_username("bob@mail").is_err());
    assert!(validate_username("eve space").is_err());
}

#[test]
fn username_max_length_passes() {
    let max: String = "a".repeat(32);
    assert!(validate_username(&max).is_ok());
}

#[test]
fn valid_password_passes() {
    assert!(validate_password("Password1").is_ok());
    assert!(validate_password("abc12345").is_ok());
}

#[test]
fn password_too_short_fails() {
    assert!(validate_password("Abc1").is_err());
    assert!(validate_password("").is_err());
}

#[test]
fn password_no_digit_fails() {
    assert!(validate_password("onlyletters").is_err());
}

#[test]
fn password_no_letter_fails() {
    assert!(validate_password("12345678").is_err());
}

#[test]
fn valid_email_passes() {
    assert!(is_valid_email("user@example.com"));
    assert!(is_valid_email("a.b+tag@sub.domain.org"));
}

#[test]
fn email_missing_at_fails() {
    assert!(!is_valid_email("notanemail.com"));
}

#[test]
fn email_empty_local_part_fails() {
    assert!(!is_valid_email("@example.com"));
}

#[test]
fn email_multiple_at_signs_fails() {
    assert!(!is_valid_email("a@b@c.com"));
}
