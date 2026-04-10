use proptest::prelude::*;
use server::database::utils::*;

proptest! {
    // 1. Compression Round-trip Property
    #[test]
    fn prop_compression_roundtrip(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let compressed = compress_data(&data).expect("Compression failed");
        let decompressed = decompress_data(&compressed).expect("Decompression failed");
        prop_assert_eq!(data, decompressed);
    }

    // 2. Password Hashing/Verification Property
    #[test]
    fn prop_password_hashing_roundtrip(password in "\\PC{8,64}") {
        let hash = hash_password(&password).expect("Hashing failed");
        let valid = verify_password(&hash, &password).expect("Verification failed");
        prop_assert!(valid);
    }

    // 3. Filename Sanitization Property
    #[test]
    fn prop_sanitize_filename_safe(name in "\\PC{1,64}") {
        let sanitized = sanitize_filename(&name);
        // Ensure no path traversal characters or invalid control chars
        prop_assert!(!sanitized.contains('/'));
        prop_assert!(!sanitized.contains('\\'));
        prop_assert!(!sanitized.contains('\0'));
    }
}
