use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::sqlite::SqlitePool;
use tracing::info;

use shared::types::user::*;

/// Register a new user. The very first user registered is automatically made admin.
pub async fn register_user(pool: &SqlitePool, new_user: NewUser) -> anyhow::Result<i64> {
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut tx = pool.begin().await?;

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&mut *tx)
        .await?;
    let is_admin = if count.0 == 0 { 1i64 } else { 0i64 };

    let res = sqlx::query(
        "INSERT INTO users (username, password_hash, email, created_at, is_admin)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&new_user.username)
    .bind(&new_user.password_hash)
    .bind(&new_user.email)
    .bind(created_at)
    .bind(is_admin)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    info!("New user made! {}", new_user.username);
    Ok(res.last_insert_rowid())
}

/// Promote a user to admin.
pub async fn promote_user(pool: &SqlitePool, user_id: i64) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    info!("User promoted! {}", user_id);
    Ok(())
}

/// Demote an admin back to a regular user.
pub async fn demote_user(pool: &SqlitePool, user_id: i64) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET is_admin = 0 WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    info!("User demoted! {}", user_id);
    Ok(())
}

/// Check if a username is already taken.
pub async fn username_exists(pool: &SqlitePool, username: String) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE username = ?")
        .bind(username)
        .fetch_one(pool)
        .await?;
    Ok(row.0 > 0)
}

/// Check if an email is already taken.
pub async fn email_exists(pool: &SqlitePool, email: String) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(email)
        .fetch_one(pool)
        .await?;
    Ok(row.0 > 0)
}

/// Update a user's username.
pub async fn update_username(
    pool: &SqlitePool,
    user_id: i64,
    new_username: String,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET username = ? WHERE id = ?")
        .bind(&new_username)
        .bind(user_id)
        .execute(pool)
        .await?;
    info!(
        "Username updated! username:{} userid:{}",
        new_username, user_id
    );
    Ok(())
}

/// Update a user's email.
pub async fn update_email(
    pool: &SqlitePool,
    user_id: i64,
    new_email: String,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET email = ? WHERE id = ?")
        .bind(&new_email)
        .bind(user_id)
        .execute(pool)
        .await?;
    info!("Email updated! email:{} userid:{}", new_email, user_id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Avatar
// ---------------------------------------------------------------------------

/// Return the on-disk path of a user's avatar, or `None` if none has been set.
/// Write the on-disk path of a newly uploaded avatar for `user_id`.
pub async fn set_user_avatar(pool: &SqlitePool, user_id: i64, path: String) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET avatar_path = ? WHERE id = ?")
        .bind(path)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Clear the stored avatar path (does **not** remove the file from disk).
/// Call this after the file has already been deleted.
pub async fn clear_user_avatar(pool: &SqlitePool, user_id: i64) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET avatar_path = NULL WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
