use std::time::{SystemTime, UNIX_EPOCH};

use shared::types::login::*;
use sqlx::sqlite::SqlitePool;

/// Get user authentication data by username.
pub async fn get_user_auth(
    pool: &SqlitePool,
    username: String,
) -> anyhow::Result<Option<UserAuth>> {
    let row = sqlx::query_as::<_, (i64, String, String, i64, Option<String>, i64)>(
        "SELECT id, username, password_hash, is_banned, ban_reason, is_admin
         FROM users WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| UserAuth {
        id: r.0,
        username: r.1,
        password_hash: r.2,
        is_banned: r.3 != 0,
        ban_reason: r.4,
        is_admin: r.5 != 0,
    }))
}

/// Create an admin session (delegates to the shared `create_session`).
pub async fn create_admin_session(
    pool: &SqlitePool,
    new_session: NewSession,
) -> anyhow::Result<i64> {
    create_session(pool, new_session).await
}

/// Update last_login for an admin (delegates to the shared helper).
pub async fn update_admin_last_login(pool: &SqlitePool, admin_id: i64) -> anyhow::Result<()> {
    update_last_login(pool, admin_id).await
}

/// Persist a new session row.
pub async fn create_session(pool: &SqlitePool, new_session: NewSession) -> anyhow::Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let res = sqlx::query(
        "INSERT INTO sessions
             (user_id, session_id, created_at, expires_at, last_activity, ip_address)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(new_session.user_id)
    .bind(new_session.session_id)
    .bind(now)
    .bind(new_session.expires_at)
    .bind(now)
    .bind(new_session.ip_address)
    .execute(pool)
    .await?;

    Ok(res.last_insert_rowid())
}

/// Look up a session by its UUID and return the full row if it hasn't expired.
///
/// Bumps `last_activity` on every hit so idle-timeout logic works.
/// Returns `None` when the session doesn't exist or has expired.
pub async fn validate_session_id(
    pool: &SqlitePool,
    session_id: String,
) -> anyhow::Result<Option<Session>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let row = sqlx::query_as::<_, (i64, i64, String, i64, i64, i64, Option<String>)>(
        "SELECT id, user_id, session_id, created_at, expires_at, last_activity, ip_address
         FROM sessions
         WHERE session_id = ?",
    )
    .bind(&session_id)
    .fetch_optional(pool)
    .await?;

    if let Some(r) = row {
        let session = Session {
            id: r.0,
            user_id: r.1,
            session_id: r.2,
            created_at: r.3,
            expires_at: r.4,
            last_activity: r.5,
            ip_address: r.6,
        };

        if session.expires_at > now {
            sqlx::query("UPDATE sessions SET last_activity = ? WHERE session_id = ?")
                .bind(now)
                .bind(session_id)
                .execute(pool)
                .await?;
            return Ok(Some(session));
        }
    }

    Ok(None)
}

/// Delete a single session by its UUID (logout from one device).
pub async fn delete_session_by_id(pool: &SqlitePool, session_id: String) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM sessions WHERE session_id = ?")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete all sessions for a user (logout from all devices / post-ban / post-password-change).
pub async fn delete_all_user_sessions(pool: &SqlitePool, user_id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Remove all expired sessions.  Called by the 60-second background task.
pub async fn cleanup_expired_sessions(pool: &SqlitePool) -> anyhow::Result<usize> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let res = sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
        .bind(now)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() as usize)
}

/// Update the `last_login` column on the users table.
pub async fn update_last_login(pool: &SqlitePool, user_id: i64) -> anyhow::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    sqlx::query("UPDATE users SET last_login = ? WHERE id = ?")
        .bind(now)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Get all active (non-expired) sessions for a given user.
pub async fn get_user_sessions(pool: &SqlitePool, user_id: i64) -> anyhow::Result<Vec<Session>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let rows = sqlx::query_as::<_, (i64, i64, String, i64, i64, i64, Option<String>)>(
        "SELECT id, user_id, session_id, created_at, expires_at, last_activity, ip_address
         FROM   sessions
         WHERE  user_id = ? AND expires_at > ?
         ORDER  BY last_activity DESC",
    )
    .bind(user_id)
    .bind(now)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Session {
            id: r.0,
            user_id: r.1,
            session_id: r.2,
            created_at: r.3,
            expires_at: r.4,
            last_activity: r.5,
            ip_address: r.6,
        })
        .collect())
}
