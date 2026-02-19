use std::time::{SystemTime, UNIX_EPOCH};

use tokio_rusqlite::{Connection, OptionalExtension, Result, params, rusqlite};

#[derive(Debug, Clone)]
pub struct PasswordResetToken {
    pub id: i64,
    pub user_id: i64,
    pub token: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub used: bool,
}

/// Change user password (when user knows current password)
pub async fn change_password(
    conn: &Connection,
    user_id: i64,
    new_password_hash: String,
) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE users SET password_hash = ?1 WHERE id = ?2",
            params![new_password_hash, user_id],
        )?;
        Ok(())
    })
    .await
}

/// Get current password hash for verification
pub async fn get_password_hash(conn: &Connection, user_id: i64) -> Result<Option<String>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare("SELECT password_hash FROM users WHERE id = ?1")?;
        let hash = stmt
            .query_row(params![user_id], |row: &rusqlite::Row| row.get(0))
            .optional()?;
        Ok(hash)
    })
    .await
}

/// Create a password reset token
pub async fn create_reset_token(
    conn: &Connection,
    user_id: i64,
    token: String,
    valid_duration_secs: i64,
) -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let expires_at = now + valid_duration_secs;

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "INSERT INTO password_reset_tokens (user_id, token, created_at, expires_at) 
             VALUES (?1, ?2, ?3, ?4)",
            params![user_id, token, now, expires_at],
        )?;

        Ok(conn.last_insert_rowid())
    })
    .await
}

/// Validate and use a password reset token
pub async fn validate_reset_token(conn: &Connection, token: String) -> Result<Option<i64>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT user_id, expires_at, used FROM password_reset_tokens WHERE token = ?1",
        )?;

        let result = stmt
            .query_row(params![token.clone()], |row: &rusqlite::Row| {
                let user_id: i64 = row.get(0)?;
                let expires_at: i64 = row.get(1)?;
                let used: i64 = row.get(2)?;
                Ok((user_id, expires_at, used))
            })
            .optional()?;

        match result {
            Some((user_id, expires_at, used)) => {
                if used != 0 {
                    // Token already used
                    Ok(None)
                } else if expires_at < now {
                    // Token expired
                    Ok(None)
                } else {
                    // Mark token as used
                    conn.execute(
                        "UPDATE password_reset_tokens SET used = 1 WHERE token = ?1",
                        params![token],
                    )?;
                    Ok(Some(user_id))
                }
            }
            None => Ok(None),
        }
    })
    .await
}

/// Reset password using a valid token
pub async fn reset_password_with_token(
    conn: &Connection,
    token: String,
    new_password_hash: String,
) -> Result<bool> {
    // Validate token and get user_id
    let user_id = match validate_reset_token(conn, token).await? {
        Some(id) => id,
        None => return Ok(false),
    };

    // Update password
    change_password(conn, user_id, new_password_hash).await?;

    Ok(true)
}

/// Delete expired reset tokens
pub async fn cleanup_expired_reset_tokens(conn: &Connection) -> Result<usize> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let count = conn.execute(
            "DELETE FROM password_reset_tokens WHERE expires_at < ?1 OR used = 1",
            params![now],
        )?;
        Ok(count)
    })
    .await
}

/// Delete all reset tokens for a user (e.g., after successful password change)
pub async fn delete_user_reset_tokens(conn: &Connection, user_id: i64) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "DELETE FROM password_reset_tokens WHERE user_id = ?1",
            params![user_id],
        )?;
        Ok(())
    })
    .await
}

/// Get user ID by email (for password reset)
pub async fn get_user_id_by_email(conn: &Connection, email: String) -> Result<Option<i64>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare("SELECT id FROM users WHERE email = ?1")?;
        let user_id = stmt
            .query_row(params![email], |row: &rusqlite::Row| row.get(0))
            .optional()?;
        Ok(user_id)
    })
    .await
}
