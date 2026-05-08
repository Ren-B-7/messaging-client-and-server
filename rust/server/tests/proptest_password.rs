#[cfg(test)]
mod proptest_utils {
    use server::database::utils;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_password_strength_property(password in "\\PC{1,30}") {
            // Using proptest to verify that passcheck's rules are strictly followed
            // by comparing against a simpler regex or logic if desired.
            // For now, ensure that the validator doesn't panic on arbitrary input.
            let _ = utils::is_strong_password(&password);
        }

        #[test]
        fn test_password_strength_invariants(
            password in "[A-Z]{4,10}[a-z]{4,10}[0-9]{4,10}[@#$%^&*!]{4,10}"
        ) {
            // Strings matching this complex pattern must be strong.
            prop_assert!(utils::is_strong_password(&password));
        }

        #[test]
        fn test_short_passwords_are_weak(password in "[ -~]{0,11}") {
            // Strings shorter than 12 chars are always weak.
            prop_assert!(!utils::is_strong_password(&password));
        }
    }
}
