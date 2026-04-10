use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio_rusqlite::{Connection, OptionalExtension, Result, params, rusqlite};

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
    filename.replace(|c: char| !c.is_alphanumeric() && c != '.' && c != '_', "_")
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

/// Get user by ID.
pub async fn get_user_by_id(conn: &Connection, user_id: i64) -> Result<Option<User>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, username, email, created_at, is_banned, first_name, last_name FROM users WHERE id = ?1",
        )?;
        let user = stmt
            .query_row(params![user_id], |row: &rusqlite::Row| {
                Ok(User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    email: row.get(2)?,
                    created_at: row.get(3)?,
                    is_banned: row.get::<_, i64>(4)? != 0,
                    name: Some(NameSurname{first_name: row.get(5)?, last_name: row.get(6)?}),
                })
            })
            .optional()?;
        Ok(user)
    })
    .await
}

/// Get user by username.
pub async fn get_user_by_username(conn: &Connection, username: String) -> Result<Option<User>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, username, email, created_at, is_banned, first_name, last_name FROM users WHERE username = ?1",
        )?;
        let user = stmt
            .query_row(params![username], |row: &rusqlite::Row| {
                Ok(User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    email: row.get(2)?,
                    created_at: row.get(3)?,
                    is_banned: row.get::<_, i64>(4)? != 0,
                    name: Some(NameSurname{first_name: row.get(5)?, last_name: row.get(6)?}),
                })
            })
            .optional()?;
        Ok(user)
    })
    .await
}

pub async fn get_user_avatar(conn: &Connection, user_id: i64) -> Result<Option<String>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare("SELECT avatar_path FROM users WHERE id = ?1")?;
        let path = stmt
            .query_row(params![user_id], |row: &rusqlite::Row| row.get(0))
            .optional()?;
        Ok(path)
    })
    .await
}

/// Search users whose username starts with `prefix` (case-insensitive).
/// Returns at most `limit` results.
pub async fn search_users_by_username(
    conn: &Connection,
    prefix: &str,
    limit: i64,
) -> Result<Vec<User>> {
    let pattern = format!("{}%", prefix.to_lowercase());
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, username, email, created_at, is_banned, first_name, last_name
             FROM users
             WHERE lower(username) LIKE ?1
             ORDER BY username ASC
             LIMIT ?2",
        )?;
        let users = stmt
            .query_map(params![pattern, limit], |row: &rusqlite::Row| {
                Ok(User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    email: row.get(2)?,
                    created_at: row.get(3)?,
                    is_banned: row.get::<_, i64>(4)? != 0,
                    name: Some(NameSurname {
                        first_name: row.get(5)?,
                        last_name: row.get(6)?,
                    }),
                })
            })?
            .collect::<std::result::Result<Vec<User>, rusqlite::Error>>()?;
        Ok(users)
    })
    .await
}

/// Update `first_name` and `last_name` for a user.
pub async fn update_user_names(conn: &Connection, user_id: i64, name: NameSurname) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE users SET first_name = ?1, last_name = ?2 WHERE id = ?3",
            params![
                name.first_name.unwrap_or_default(),
                name.last_name.unwrap_or_default(),
                user_id
            ],
        )?;
        Ok(())
    })
    .await
}
