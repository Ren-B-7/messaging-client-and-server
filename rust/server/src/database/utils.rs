use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

/// Get current Unix timestamp in seconds
pub fn get_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Generate a UUID-based session token
pub fn generate_uuid_token() -> String {
    Uuid::new_v4().to_string()
}

/// Hash a password using Argon2id (recommended for production)
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    use argon2::{
        Argon2,
        password_hash::{PasswordHasher, SaltString},
    };
    use rand::rngs::OsRng;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| anyhow::anyhow!("Password hashing failed: {}", e))
}

/// Verify a password against its hash
pub fn verify_password(hash: &str, password: &str) -> anyhow::Result<bool> {
    use argon2::{
        Argon2,
        password_hash::{PasswordHash, PasswordVerifier},
    };

    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| anyhow::anyhow!("Failed to parse password hash: {}", e))?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Compress data using gzip
pub fn compress_data(data: &[u8]) -> std::io::Result<Vec<u8>> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}

/// Decompress gzipped data
pub fn decompress_data(data: &[u8]) -> std::io::Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

/// Validate email format (basic validation)
pub fn is_valid_email(email: &str) -> bool {
    email.contains('@') && email.contains('.') && email.len() > 3
}

/// Validate username (alphanumeric, underscore, 3-20 chars)
pub fn is_valid_username(username: &str) -> bool {
    if username.len() < 3 || username.len() > 32 {
        return false;
    }

    username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Validate password strength (min 8 chars, at least one number, one letter)
pub fn is_strong_password(password: &str) -> bool {
    if password.len() < 8 {
        return false;
    }

    let has_letter = password.chars().any(|c| c.is_alphabetic());
    let has_number = password.chars().any(|c| c.is_numeric());

    has_letter && has_number
}

/// Calculate session expiry (current time + duration in seconds)
pub fn calculate_expiry(duration_secs: i64) -> i64 {
    get_timestamp() + duration_secs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp() {
        let ts = get_timestamp();
        assert!(ts > 0);
    }

    #[test]
    fn test_session_token() {
        let token1 = generate_uuid_token();
        let token2 = generate_uuid_token();
        assert_ne!(token1, token2);
        assert_eq!(token1.len(), 36); // 32 bytes as hex + 4 delims
    }

    #[test]
    fn test_password_hashing() {
        let password = "test_password_123";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(&hash, password).unwrap());
        assert!(!verify_password(&hash, "wrong_password").unwrap());
    }

    #[test]
    fn test_compression() {
        let data = b"Hello, World! This is test data.";
        let compressed = compress_data(data).unwrap();
        let decompressed = decompress_data(&compressed).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_email_validation() {
        assert!(is_valid_email("test@example.com"));
        assert!(!is_valid_email("invalid"));
        assert!(!is_valid_email("@."));
    }

    #[test]
    fn test_username_validation() {
        assert!(is_valid_username("alice"));
        assert!(is_valid_username("user_123"));
        assert!(is_valid_username("bob-smith")); // hyphen now valid
        assert!(is_valid_username(&"a".repeat(32))); // max length
        assert!(!is_valid_username("ab")); // too short
        assert!(!is_valid_username(&"a".repeat(33))); // too long
        assert!(!is_valid_username("user@name")); // invalid char
    }

    #[test]
    fn test_password_strength() {
        assert!(is_strong_password("password123"));
        assert!(!is_strong_password("short1"));
        assert!(!is_strong_password("nodigits"));
        assert!(!is_strong_password("12345678"));
    }
}
