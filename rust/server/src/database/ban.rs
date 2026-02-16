use std::time::{SystemTime, UNIX_EPOCH};
use tokio_rusqlite::{Connection, OptionalExtension, Result, params, rusqlite};

#[derive(Debug, Clone)]
pub struct BanInfo {
    pub user_id: i64,
    pub username: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
    pub banned_at: Option<i64>,
    pub banned_by: Option<i64>,
}

/// Ban a user
pub async fn ban_user(
    conn: &Connection,
    user_id: i64,
    banned_by: i64,
    reason: Option<String>,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn| {
        conn.execute(
            "UPDATE users SET is_banned = 1, ban_reason = ?1, banned_at = ?2, banned_by = ?3 WHERE id = ?4",
            params![reason, now, banned_by, user_id],
        )?;

        // Also delete all active sessions
        conn.execute(
            "DELETE FROM sessions WHERE user_id = ?1",
            params![user_id],
        )?;

        Ok(())
    })
    .await
}

/// Unban a user
pub async fn unban_user(conn: &Connection, user_id: i64) -> Result<()> {
    conn.call(move |conn| {
        conn.execute(
            "UPDATE users SET is_banned = 0, ban_reason = NULL, banned_at = NULL, banned_by = NULL WHERE id = ?1",
            params![user_id],
        )?;
        Ok(())
    })
    .await
}

/// Check if a user is banned
pub async fn is_user_banned(conn: &Connection, user_id: i64) -> Result<bool> {
    conn.call(move |conn| {
        let mut stmt = conn.prepare("SELECT is_banned FROM users WHERE id = ?1")?;
        let is_banned: i64 = stmt.query_row(params![user_id], |row| row.get(0))?;
        Ok(is_banned != 0)
    })
    .await
}

/// Get ban information for a user
pub async fn get_ban_info(conn: &Connection, user_id: i64) -> Result<Option<BanInfo>> {
    conn.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, username, is_banned, ban_reason, banned_at, banned_by FROM users WHERE id = ?1"
        )?;

        let info = stmt.query_row(params![user_id], |row| {
            Ok(BanInfo {
                user_id: row.get(0)?,
                username: row.get(1)?,
                is_banned: row.get::<_, i64>(2)? != 0,
                ban_reason: row.get(3)?,
                banned_at: row.get(4)?,
                banned_by: row.get(5)?,
            })
        }).optional()?;

        Ok(info)
    })
    .await
}

/// Get all banned users
pub async fn get_banned_users(conn: &Connection) -> Result<Vec<BanInfo>> {
    conn.call(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, username, is_banned, ban_reason, banned_at, banned_by 
             FROM users 
             WHERE is_banned = 1
             ORDER BY banned_at DESC",
        )?;

        let users = stmt
            .query_map([], |row| {
                Ok(BanInfo {
                    user_id: row.get(0)?,
                    username: row.get(1)?,
                    is_banned: row.get::<_, i64>(2)? != 0,
                    ban_reason: row.get(3)?,
                    banned_at: row.get(4)?,
                    banned_by: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<BanInfo>, rusqlite::Error>>()?;

        Ok(users)
    })
    .await
}

/// Update ban reason
pub async fn update_ban_reason(conn: &Connection, user_id: i64, new_reason: String) -> Result<()> {
    conn.call(move |conn| {
        conn.execute(
            "UPDATE users SET ban_reason = ?1 WHERE id = ?2 AND is_banned = 1",
            params![new_reason, user_id],
        )?;
        Ok(())
    })
    .await
}
