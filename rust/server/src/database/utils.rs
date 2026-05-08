use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::sqlite::SqlitePool;

use shared::types::user::*;
use uuid::Uuid;

pub fn is_allowed_mime_type(mime_type: &str) -> bool {
    let allowed = [
        "image/png",
        "image/jpeg",
        "image/gif",
        "image/webp",
        "application/pdf",
        "text/plain",
        "application/zip",
    ];
    allowed.contains(&mime_type)
}

pub fn sanitize_filename(filename: &str) -> String {
    // 1. Remove null bytes
    let clean = filename.replace('\0', "");

    // 2. Normalize separators: convert Windows backslashes to Unix slashes
    let normalized = clean.replace('\\', "/");

    // 3. Extract only the base name (prevents path traversal)
    let path = std::path::Path::new(&normalized);
    let base_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    // 4. If empty or just invalid, return "unnamed"
    if base_name.is_empty() || base_name == "." || base_name == ".." || base_name == "/" {
        return "unnamed".to_string();
    }

    // 5. Sanitize remaining characters: only allow alphanumeric, dots, underscores, hyphens, brackets, parentheses, and @
    let sanitized: String = base_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric()
                || c == '.'
                || c == '_'
                || c == '-'
                || c == '['
                || c == ']'
                || c == '('
                || c == ')'
                || c == '@'
                || c == ' '
            {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Remove leading/trailing underscores resulting from invalid chars
    let trimmed = sanitized.trim_matches('_');

    if trimmed.is_empty() {
        "unnamed".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn build_storage_path(uploads_dir: &str, filename: &str) -> PathBuf {
    let uuid = Uuid::new_v4().to_string();
    let stored_name = format!("{}_{}", uuid, filename);
    PathBuf::from(uploads_dir).join(stored_name)
}

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
pub fn is_valid_name(username: &str) -> bool {
    if username.len() < 3 || username.len() > 32 {
        return false;
    }

    username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Validate password strength (min 12 chars, at least one uppercase, one lowercase, one number, one special)
pub fn is_strong_password(password: &str) -> bool {
    use passcheck::PasswordChecker;
    let checker = PasswordChecker::new()
        .min_length(12)
        .require_upper_lower()
        .require_number()
        .require_special_char();

    checker.validate(password).is_ok()
}

/// Calculate session expiry (current time + duration in seconds)
pub fn calculate_expiry(duration_secs: i64) -> i64 {
    get_timestamp() + duration_secs
}

/// Get user by ID.
pub async fn get_user_by_id(pool: &SqlitePool, user_id: i64) -> anyhow::Result<Option<User>> {
    let row = sqlx::query_as::<_, (i64, String, Option<String>, i64, i64, Option<String>, Option<String>)>(
        "SELECT id, username, email, created_at, is_banned, first_name, last_name FROM users WHERE id = ?"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| User {
        id: r.0,
        username: r.1,
        email: r.2,
        created_at: r.3,
        is_banned: r.4 != 0,
        name: Some(NameSurname {
            first_name: r.5,
            last_name: r.6,
        }),
    }))
}

/// Get user by username.
pub async fn get_user_by_username(
    pool: &SqlitePool,
    username: String,
) -> anyhow::Result<Option<User>> {
    let row = sqlx::query_as::<_, (i64, String, Option<String>, i64, i64, Option<String>, Option<String>)>(
        "SELECT id, username, email, created_at, is_banned, first_name, last_name FROM users WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| User {
        id: r.0,
        username: r.1,
        email: r.2,
        created_at: r.3,
        is_banned: r.4 != 0,
        name: Some(NameSurname {
            first_name: r.5,
            last_name: r.6,
        }),
    }))
}

pub async fn get_user_avatar(pool: &SqlitePool, user_id: i64) -> anyhow::Result<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT avatar_path FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|r| r.0))
}

/// Search users whose username starts with `prefix` (case-insensitive).
pub async fn search_users_by_username(
    pool: &SqlitePool,
    prefix: &str,
    limit: i64,
) -> anyhow::Result<Vec<User>> {
    let pattern = format!("{}%", prefix.to_lowercase());
    let rows = sqlx::query_as::<
        _,
        (
            i64,
            String,
            Option<String>,
            i64,
            i64,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT id, username, email, created_at, is_banned, first_name, last_name
         FROM users
         WHERE lower(username) LIKE ?
         ORDER BY username ASC
         LIMIT ?",
    )
    .bind(pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| User {
            id: r.0,
            username: r.1,
            email: r.2,
            created_at: r.3,
            is_banned: r.4 != 0,
            name: Some(NameSurname {
                first_name: r.5,
                last_name: r.6,
            }),
        })
        .collect())
}

/// Update `first_name` and `last_name` for a user.
pub async fn update_user_names(
    pool: &SqlitePool,
    user_id: i64,
    name: NameSurname,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET first_name = ?, last_name = ? WHERE id = ?")
        .bind(name.first_name)
        .bind(name.last_name)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
