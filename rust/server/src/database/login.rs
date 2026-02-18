use std::time::{SystemTime, UNIX_EPOCH};
use tokio_rusqlite::{Connection, OptionalExtension, Result, params, rusqlite};

#[derive(Debug, Clone)]
pub struct LoginCredentials {
    pub username: String,
    pub password_hash: String,
}

#[derive(Debug, Clone)]
pub struct UserAuth {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
}

/// Auth record for admin accounts — same users table, filtered by is_admin = 1
#[derive(Debug, Clone)]
pub struct AdminAuth {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    pub session_token: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub last_activity: i64,
}

#[derive(Debug, Clone)]
pub struct NewSession {
    pub user_id: i64,
    pub session_token: String,
    pub expires_at: i64,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Get user authentication data by username
pub async fn get_user_auth(conn: &Connection, username: String) -> Result<Option<UserAuth>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, is_banned, ban_reason FROM users WHERE username = ?1"
        )?;

        let user = stmt.query_row(params![username], |row: &rusqlite::Row| {
            Ok(UserAuth {
                id: row.get(0)?,
                username: row.get(1)?,
                password_hash: row.get(2)?,
                is_banned: row.get::<_, i64>(3)? != 0,
                ban_reason: row.get(4)?,
            })
        }).optional()?;

        Ok(user)
    })
    .await
}

/// Get admin authentication data by username — only matches rows where is_admin = 1
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

/// Create an admin session (delegates to the shared create_session)
pub async fn create_admin_session(conn: &Connection, new_session: NewSession) -> Result<i64> {
    create_session(conn, new_session).await
}

/// Update last_login for an admin (delegates to the shared helper)
pub async fn update_admin_last_login(conn: &Connection, admin_id: i64) -> Result<()> {
    update_last_login(conn, admin_id).await
}

/// Create a new session
pub async fn create_session(conn: &Connection, new_session: NewSession) -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "INSERT INTO sessions (user_id, session_token, created_at, expires_at, last_activity, ip_address, user_agent) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new_session.user_id,
                new_session.session_token,
                now,
                new_session.expires_at,
                now,
                new_session.ip_address,
                new_session.user_agent,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    })
    .await
}

/// Validate session token and return user_id if valid
pub async fn validate_session(conn: &Connection, session_token: String) -> Result<Option<i64>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt =
            conn.prepare("SELECT user_id, expires_at FROM sessions WHERE session_token = ?1")?;

        let result = stmt
            .query_row(params![session_token.clone()], |row: &rusqlite::Row| {
                let user_id: i64 = row.get(0)?;
                let expires_at: i64 = row.get(1)?;
                Ok((user_id, expires_at))
            })
            .optional()?;

        match result {
            Some((user_id, expires_at)) => {
                if expires_at > now {
                    // Update last_activity
                    conn.execute(
                        "UPDATE sessions SET last_activity = ?1 WHERE session_token = ?2",
                        params![now, session_token],
                    )?;
                    Ok(Some(user_id))
                } else {
                    // Session expired
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    })
    .await
}

/// Delete a session (logout)
pub async fn delete_session(conn: &Connection, session_token: String) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "DELETE FROM sessions WHERE session_token = ?1",
            params![session_token],
        )?;
        Ok(())
    })
    .await
}

/// Delete all sessions for a user (logout everywhere)
pub async fn delete_all_user_sessions(conn: &Connection, user_id: i64) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute("DELETE FROM sessions WHERE user_id = ?1", params![user_id])?;
        Ok(())
    })
    .await
}

/// Clean up expired sessions
pub async fn cleanup_expired_sessions(conn: &Connection) -> Result<usize> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let count = conn.execute("DELETE FROM sessions WHERE expires_at < ?1", params![now])?;
        Ok(count)
    })
    .await
}

/// Update last login timestamp
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

/// Get all active sessions for a user
pub async fn get_user_sessions(conn: &Connection, user_id: i64) -> Result<Vec<Session>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, user_id, session_token, created_at, expires_at, last_activity 
             FROM sessions 
             WHERE user_id = ?1 AND expires_at > ?2
             ORDER BY last_activity DESC",
        )?;

        let sessions = stmt
            .query_map(params![user_id, now], |row| {
                Ok(Session {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    session_token: row.get(2)?,
                    created_at: row.get(3)?,
                    expires_at: row.get(4)?,
                    last_activity: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<Session>, rusqlite::Error>>()?;

        Ok(sessions)
    })
    .await
}
