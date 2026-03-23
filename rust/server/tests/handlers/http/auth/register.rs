use server::handlers::http::auth::register::*;

// ── username validation ───────────────────────────────────────────────────

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
    assert!(validate_username(&"a".repeat(33)).is_err());
}

#[test]
fn username_max_length_passes() {
    assert!(validate_username(&"a".repeat(32)).is_ok());
}

#[test]
fn username_min_length_passes() {
    assert!(validate_username("abc").is_ok());
}

#[test]
fn username_invalid_chars_fails() {
    assert!(validate_username("alice!").is_err());
    assert!(validate_username("bob@mail").is_err());
    assert!(validate_username("eve space").is_err());
    assert!(validate_username("dot.name").is_err());
    assert!(validate_username("slash/name").is_err());
}

#[test]
fn username_allows_hyphen_and_underscore() {
    assert!(validate_username("a-b_c").is_ok());
    assert!(validate_username("---").is_ok());
    assert!(validate_username("___").is_ok());
}

// ── password validation ───────────────────────────────────────────────────

#[test]
fn valid_password_passes() {
    assert!(validate_password("Password1").is_ok());
    assert!(validate_password("abc12345").is_ok());
}

#[test]
fn password_too_short_fails() {
    assert!(validate_password("Abc1").is_err());
    assert!(validate_password("").is_err());
    assert!(validate_password("1234567").is_err()); // 7 chars, one under limit
}

#[test]
fn password_exactly_8_chars_passes() {
    assert!(validate_password("abcde123").is_ok());
}

#[test]
fn password_no_digit_fails() {
    assert!(validate_password("onlyletters").is_err());
    assert!(validate_password("ALLCAPS!!").is_err());
}

#[test]
fn password_no_letter_fails() {
    assert!(validate_password("12345678").is_err());
    assert!(validate_password("99999999").is_err());
}

#[test]
fn password_max_length_passes() {
    // 128 is the upper boundary — must pass
    assert!(validate_password(&format!("a1{}", "x".repeat(126))).is_ok());
}

#[test]
fn password_over_max_length_fails() {
    // 129 chars — one over the boundary
    assert!(validate_password(&format!("a1{}", "x".repeat(127))).is_err());
}

// ── email validation ──────────────────────────────────────────────────────

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

#[test]
fn email_domain_no_dot_fails() {
    assert!(!is_valid_email("user@localhost"));
}

#[test]
fn email_empty_string_fails() {
    assert!(!is_valid_email(""));
}
