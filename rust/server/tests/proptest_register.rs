use proptest::prelude::*;
use server::handlers::http::auth::register::*;

// ── Property-based tests for username validation ───────────────────────────

proptest! {
    #[test]
    fn prop_username_length_boundary(s in "\\PC*") {
        // Test that usernames 3-32 chars are valid if they only contain allowed chars
        let valid_chars = s.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>();

        if valid_chars.len() >= 3 && valid_chars.len() <= 32 {
            prop_assert!(validate_username(&valid_chars).is_ok());
        }
    }

    #[test]
    fn prop_username_invalid_chars(s in "[^a-zA-Z0-9-_]{3,32}") {
        // Any string composed entirely of invalid characters (length 3-32) must fail
        // Using ASCII-specific invalid characters for the property test.
        let is_ascii_invalid = s.chars().all(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
        if is_ascii_invalid {
            prop_assert!(validate_username(&s).is_err());
        }
    }
}

// ── Property-based tests for password validation ───────────────────────────

proptest! {
    #[test]
    fn prop_password_length_boundary(s in "[a-zA-Z0-9]{8,128}") {
        // Ensure at least one letter and one digit to pass
        let has_digit = s.chars().any(|c| c.is_ascii_digit());
        let has_letter = s.chars().any(|c| c.is_ascii_alphabetic());

        if has_digit && has_letter {
            prop_assert!(validate_password(&s).is_ok());
        }
    }
}

// ── Property-based tests for email validation ──────────────────────────────

proptest! {
    #[test]
    fn prop_email_basic_regex_check(user in "[a-z0-9]{1,10}", domain in "[a-z]{3,10}", tld in "[a-z]{2,4}") {
        let email = format!("{}@{}.{}", user, domain, tld);
        prop_assert!(is_valid_email(&email));
    }
}
