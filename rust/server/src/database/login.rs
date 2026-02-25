use std::time::{SystemTime, UNIX_EPOCH};

use tokio_rusqlite::{Connection, OptionalExtension, Result, params, rusqlite};

use shared::types::login::*;

/// Get user authentication data by username.
pub async fn get_user_auth(conn: &Connection, username: String) -> Result<Option<UserAuth>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, is_banned, ban_reason
             FROM users WHERE username = ?1",
        )?;

        let user = stmt
            .query_row(params![username], |row: &rusqlite::Row| {
                Ok(UserAuth {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    password_hash: row.get(2)?,
                    is_banned: row.get::<_, i64>(3)? != 0,
                    ban_reason: row.get(4)?,
                })
            })
            .optional()?;

        Ok(user)
    })
    .await
}

/// Get admin authentication data by username — only matches rows where `is_admin = 1`.
pub async fn get_admin_auth(conn: &Connection, username: String) -> Result<Option<AdminAuth>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, is_banned, ban_reason
             FROM users WHERE username = ?1 AND is_admin = 1",
        )?;

        let admin = stmt
            .query_row(params![username], |row: &rusqlite::Row| {
                Ok(AdminAuth {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    password_hash: row.get(2)?,
                    is_banned: row.get::<_, i64>(3)? != 0,
                    ban_reason: row.get(4)?,
                })
            })
            .optional()?;

        Ok(admin)
    })
    .await
}

/// Create an admin session (delegates to the shared `create_session`).
pub async fn create_admin_session(conn: &Connection, new_session: NewSession) -> Result<i64> {
    create_session(conn, new_session).await
}

/// Update last_login for an admin (delegates to the shared helper).
pub async fn update_admin_last_login(conn: &Connection, admin_id: i64) -> Result<()> {
    update_last_login(conn, admin_id).await
}

/// Persist a new session row.
///
/// `new_session.session_id` is the UUID embedded in the JWT claims.
/// `ip_address` is stored so the secure-path validator can reject requests
/// from a different IP (stolen-token protection).
/// `user_agent` is intentionally NOT stored here — it lives in the JWT.
pub async fn create_session(conn: &Connection, new_session: NewSession) -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "INSERT INTO sessions
                 (user_id, session_id, created_at, expires_at, last_activity, ip_address)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                new_session.user_id,
                new_session.session_id,
                now,
                new_session.expires_at,
                now,
                new_session.ip_address,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    })
    .await
}

/// Look up a session by its UUID and return the full row if it hasn't expired.
///
/// This is the **secure-path** DB call used by POST / PUT / DELETE handlers.
/// It also bumps `last_activity` on every hit so idle-timeout logic works.
///
/// Returns `None` when the `session_id` doesn't exist or has expired —
/// i.e. the user logged out or the JWT should be treated as revoked.
pub async fn validate_session_id(
    conn: &Connection,
    session_id: String,
) -> Result<Option<Session>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, user_id, session_id, created_at, expires_at, last_activity, ip_address
             FROM sessions
             WHERE session_id = ?1",
        )?;

        let result = stmt
            .query_row(params![session_id.clone()], |row: &rusqlite::Row| {
                Ok(Session {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    session_id: row.get(2)?,
                    created_at: row.get(3)?,
                    expires_at: row.get(4)?,
                    last_activity: row.get(5)?,
                    ip_address: row.get(6)?,
                })
            })
            .optional()?;

        match result {
            Some(session) if session.expires_at > now => {
                // Touch last_activity on every valid secure-path hit.
                conn.execute(
                    "UPDATE sessions SET last_activity = ?1 WHERE session_id = ?2",
                    params![now, session_id],
                )?;
                Ok(Some(session))
            }
            _ => Ok(None),
        }
    })
    .await
}

/// Delete a single session by its UUID (logout from one device).
pub async fn delete_session_by_id(conn: &Connection, session_id: String) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "DELETE FROM sessions WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    })
    .await
}

/// Delete all sessions for a user (logout from all devices).
pub async fn delete_all_user_sessions(conn: &Connection, user_id: i64) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "DELETE FROM sessions WHERE user_id = ?1",
            params![user_id],
        )?;
        Ok(())
    })
    .await
}

/// Remove all expired sessions.  Intended to be called by a periodic cleanup task.
pub async fn cleanup_expired_sessions(conn: &Connection) -> Result<usize> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let count =
            conn.execute("DELETE FROM sessions WHERE expires_at < ?1", params![now])?;
        Ok(count)
    })
    .await
}

/// Update the `last_login` column on the users table.
pub async fn update_last_login(conn: &Connection, user_id: i64) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE users SET last_login = ?1 WHERE id = ?2",
            params![now, user_id],
        )?;
        Ok(())
    })
    .await
}

/// Validate a session belonging to an admin account.
///
/// Used by the admin server's auth guard.  Returns the admin's `user_id`
/// only when both the session is active AND the user has `is_admin = 1`.
pub async fn validate_admin_session(
    conn: &Connection,
    session_id: String,
) -> Result<Option<i64>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT s.user_id, s.expires_at
             FROM   sessions s
             INNER JOIN users u ON u.id = s.user_id
             WHERE  s.session_id = ?1
               AND  u.is_admin  = 1
               AND  u.is_banned = 0",
        )?;

        let result = stmt
            .query_row(params![session_id.clone()], |row: &rusqlite::Row| {
                let user_id: i64 = row.get(0)?;
                let expires_at: i64 = row.get(1)?;
                Ok((user_id, expires_at))
            })
            .optional()?;

        match result {
            Some((user_id, expires_at)) if expires_at > now => {
                conn.execute(
                    "UPDATE sessions SET last_activity = ?1 WHERE session_id = ?2",
                    params![now, session_id],
                )?;
                Ok(Some(user_id))
            }
            _ => Ok(None),
        }
    })
    .await
}

/// Get all active (non-expired) sessions for a given user.
pub async fn get_user_sessions(conn: &Connection, user_id: i64) -> Result<Vec<Session>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, user_id, session_id, created_at, expires_at, last_activity, ip_address
             FROM   sessions
             WHERE  user_id = ?1 AND expires_at > ?2
             ORDER  BY last_activity DESC",
        )?;

        let sessions = stmt
            .query_map(params![user_id, now], |row| {
                Ok(Session {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    session_id: row.get(2)?,
                    created_at: row.get(3)?,
                    expires_at: row.get(4)?,
                    last_activity: row.get(5)?,
                    ip_address: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<Session>, rusqlite::Error>>()?;

        Ok(sessions)
    })
    .await
}
