use server::database::utils;

#[test]
fn test_password_hashing_cycle() {
    let password = "super_secure_password";
    let hash = utils::hash_password(password).unwrap();

    assert!(utils::verify_password(&hash, password).unwrap());
    assert!(!utils::verify_password(&hash, "wrong_password").unwrap());
}

#[test]
fn test_username_validation_logic() {
    assert!(utils::is_valid_name("renier_7"));
    assert!(!utils::is_valid_name("a")); // too short
    assert!(!utils::is_valid_name("user!name")); // invalid chars
}

#[test]
fn test_data_compression() {
    let original = b"repeat repeat repeat repeat repeat data";
    let compressed = utils::compress_data(original).unwrap();
    let decompressed = utils::decompress_data(&compressed).unwrap();

    assert_eq!(original, decompressed.as_slice());
    assert!(compressed.len() < original.len());
}

#[test]
fn test_timestamp() {
    let ts = utils::get_timestamp();
    assert!(ts > 0);
}

#[test]
fn test_session_token() {
    let token1 = utils::generate_uuid_token();
    let token2 = utils::generate_uuid_token();
    assert_ne!(token1, token2);
    assert_eq!(token1.len(), 36); // 32 bytes as hex + 4 delims
}

#[test]
fn test_password_hashing() {
    let password = "test_password_123";
    let hash = utils::hash_password(password).unwrap();
    assert!(utils::verify_password(&hash, password).unwrap());
    assert!(!utils::verify_password(&hash, "wrong_password").unwrap());
}

#[test]
fn test_compression() {
    let data = b"Hello, World! This is test data.";
    let compressed = utils::compress_data(data).unwrap();
    let decompressed = utils::decompress_data(&compressed).unwrap();
    assert_eq!(data, decompressed.as_slice());
}

#[test]
fn test_email_validation() {
    assert!(utils::is_valid_email("test@example.com"));
    assert!(!utils::is_valid_email("invalid"));
    assert!(!utils::is_valid_email("@."));
}

#[test]
fn test_username_validation() {
    assert!(utils::is_valid_name("alice"));
    assert!(utils::is_valid_name("user_123"));
    assert!(utils::is_valid_name("bob-smith")); // hyphen now valid
    assert!(utils::is_valid_name(&"a".repeat(32))); // max length
    assert!(!utils::is_valid_name("ab")); // too short
    assert!(!utils::is_valid_name(&"a".repeat(33))); // too long
    assert!(!utils::is_valid_name("user@name")); // invalid char
}

#[test]
fn test_password_strength() {
    // New policy: min 12 chars, upper, lower, digit, special
    assert!(utils::is_strong_password("SecurePass123!"));
    assert!(!utils::is_strong_password("short1")); // Too short
    assert!(!utils::is_strong_password("password123!")); // No uppercase
    assert!(!utils::is_strong_password("PASSWORD123!")); // No lowercase
    assert!(!utils::is_strong_password("SecurePass!!!")); // No digit
    assert!(!utils::is_strong_password("SecurePass123")); // No special
}
