use std::time::{SystemTime, UNIX_EPOCH};

use shared::types::user::BanInfo;
use sqlx::sqlite::SqlitePool;

/// Ban a user
pub async fn ban_user(
    pool: &SqlitePool,
    user_id: i64,
    banned_by: i64,
    reason: Option<String>,
) -> anyhow::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE users SET is_banned = 1, ban_reason = ?, banned_at = ?, banned_by = ? WHERE id = ?",
    )
    .bind(reason)
    .bind(now)
    .bind(banned_by)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    // Also delete all active sessions
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(())
}

/// Unban a user
pub async fn unban_user(pool: &SqlitePool, user_id: i64) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE users SET is_banned = 0, ban_reason = NULL, banned_at = NULL, banned_by = NULL WHERE id = ?"
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Check if a user is banned
pub async fn is_user_banned(pool: &SqlitePool, user_id: i64) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as("SELECT is_banned FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0 != 0)
}

/// Get ban information for a user
pub async fn get_ban_info(pool: &SqlitePool, user_id: i64) -> anyhow::Result<Option<BanInfo>> {
    let row = sqlx::query_as::<_, (i64, String, i64, Option<String>, Option<i64>, Option<i64>)>(
        "SELECT id, username, is_banned, ban_reason, banned_at, banned_by FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| BanInfo {
        user_id: r.0,
        username: r.1,
        is_banned: r.2 != 0,
        ban_reason: r.3,
        banned_at: r.4,
        banned_by: r.5,
    }))
}

/// Get all banned users
pub async fn get_banned_users(pool: &SqlitePool) -> anyhow::Result<Vec<BanInfo>> {
    let rows = sqlx::query_as::<_, (i64, String, i64, Option<String>, Option<i64>, Option<i64>)>(
        "SELECT id, username, is_banned, ban_reason, banned_at, banned_by 
         FROM users 
         WHERE is_banned = 1
         ORDER BY banned_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| BanInfo {
            user_id: r.0,
            username: r.1,
            is_banned: r.2 != 0,
            ban_reason: r.3,
            banned_at: r.4,
            banned_by: r.5,
        })
        .collect())
}

/// Update ban reason
pub async fn update_ban_reason(
    pool: &SqlitePool,
    user_id: i64,
    new_reason: String,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE users SET ban_reason = ? WHERE id = ? AND is_banned = 1")
        .bind(new_reason)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
