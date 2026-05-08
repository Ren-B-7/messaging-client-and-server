use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::sqlite::SqlitePool;

/// Change user password (when user knows current password)
pub async fn change_password(
    pool: &SqlitePool,
    user_id: i64,
    new_password_hash: String,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(new_password_hash)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Get current password hash for verification
pub async fn get_password_hash(pool: &SqlitePool, user_id: i64) -> anyhow::Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT password_hash FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

/// Create a password reset token
pub async fn create_reset_token(
    pool: &SqlitePool,
    user_id: i64,
    token: String,
    valid_duration_secs: i64,
) -> anyhow::Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let expires_at = now + valid_duration_secs;

    let res = sqlx::query(
        "INSERT INTO password_reset_tokens (user_id, token, created_at, expires_at) 
         VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(token)
    .bind(now)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(res.last_insert_rowid())
}

/// Validate and use a password reset token
pub async fn validate_reset_token(pool: &SqlitePool, token: String) -> anyhow::Result<Option<i64>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut tx = pool.begin().await?;

    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT user_id, expires_at, used FROM password_reset_tokens WHERE token = ?",
    )
    .bind(&token)
    .fetch_optional(&mut *tx)
    .await?;

    match row {
        Some((user_id, expires_at, used)) => {
            if used != 0 || expires_at < now {
                Ok(None)
            } else {
                // Mark token as used
                sqlx::query("UPDATE password_reset_tokens SET used = 1 WHERE token = ?")
                    .bind(token)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                Ok(Some(user_id))
            }
        }
        None => Ok(None),
    }
}

/// Reset password using a valid token
pub async fn reset_password_with_token(
    pool: &SqlitePool,
    token: String,
    new_password_hash: String,
) -> anyhow::Result<bool> {
    // Validate token and get user_id
    let user_id = match validate_reset_token(pool, token).await? {
        Some(id) => id,
        None => return Ok(false),
    };

    // Update password
    change_password(pool, user_id, new_password_hash).await?;

    Ok(true)
}

/// Delete expired reset tokens
pub async fn cleanup_expired_reset_tokens(pool: &SqlitePool) -> anyhow::Result<usize> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let res = sqlx::query("DELETE FROM password_reset_tokens WHERE expires_at < ? OR used = 1")
        .bind(now)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() as usize)
}

/// Delete all reset tokens for a user (e.g., after successful password change)
pub async fn delete_user_reset_tokens(pool: &SqlitePool, user_id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM password_reset_tokens WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Get user ID by email (for password reset)
pub async fn get_user_id_by_email(pool: &SqlitePool, email: String) -> anyhow::Result<Option<i64>> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE email = ?")
        .bind(email)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}
