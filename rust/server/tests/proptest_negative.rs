use proptest::prelude::*;
use server::database::utils::*;
use server::handlers::http::auth::register::*;

proptest! {
    // Negative test for username: Ensure obvious invalid formats are rejected
    #[test]
    fn prop_username_negative_cases(s in "\\PC{0,32}") {
        // Test patterns that MUST fail
        let invalid_patterns = vec![
            "!!", "@@", "  ", "..", "/", "\\", "a!", "user name",
        ];
        
        // Specifically check known bad patterns if generated
        if invalid_patterns.iter().any(|&p| s.contains(p)) || s.len() < 3 || s.len() > 32 {
            prop_assert!(validate_username(&s).is_err());
        }
    }

    // Negative test for filename: Ensure path traversal and control chars are rejected or sanitized
    #[test]
    fn prop_filename_negative_cases(s in "\\PC{0,64}") {
        let sanitized = sanitize_filename(&s);
        
        // Property: Sanitized name must NEVER contain dangerous path chars
        prop_assert!(!sanitized.contains('/'));
        prop_assert!(!sanitized.contains('\\'));
        prop_assert!(!sanitized.contains('\0'));
        
        // Property: If input contains ONLY dangerous chars, it MUST NOT return an empty string
        if !s.is_empty() && sanitized.is_empty() {
             panic!("Sanitization returned empty string for input: {}", s);
        }
    }
}
