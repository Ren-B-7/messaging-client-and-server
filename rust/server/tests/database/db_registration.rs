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
    assert!(utils::is_valid_username("renier_7"));
    assert!(!utils::is_valid_username("a")); // too short
    assert!(!utils::is_valid_username("user!name")); // invalid chars
}

#[test]
fn test_data_compression() {
    let original = b"repeat repeat repeat repeat repeat data";
    let compressed = utils::compress_data(original).unwrap();
    let decompressed = utils::decompress_data(&compressed).unwrap();

    assert_eq!(original, decompressed.as_slice());
    assert!(compressed.len() < original.len());
}
