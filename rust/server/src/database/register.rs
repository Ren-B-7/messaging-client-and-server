use std::time::{SystemTime, UNIX_EPOCH};

use tokio_rusqlite::{Connection, OptionalExtension, Result, params, rusqlite};
use tracing::info;

use shared::types::user::*;

/// Register a new user. The very first user registered is automatically made admin.
pub async fn register_user(conn: &Connection, new_user: NewUser) -> Result<i64> {
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        let is_admin = if count == 0 { 1i64 } else { 0i64 };

        conn.execute(
            "INSERT INTO users (username, password_hash, email, created_at, is_admin)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                new_user.username,
                new_user.password_hash,
                new_user.email,
                created_at,
                is_admin,
            ],
        )?;
        info!("New user made! {}", new_user.username);
        Ok(conn.last_insert_rowid())
    })
    .await
}

/// Promote a user to admin.
pub async fn promote_user(conn: &Connection, user_id: i64) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute("UPDATE users SET is_admin = 1 WHERE id = ?1", params![user_id])?;
        info!("User promoted! {}", user_id);
        Ok(())
    })
    .await
}

/// Demote an admin back to a regular user.
pub async fn demote_user(conn: &Connection, user_id: i64) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute("UPDATE users SET is_admin = 0 WHERE id = ?1", params![user_id])?;
        info!("User demoted! {}", user_id);
        Ok(())
    })
    .await
}

/// Check if a username is already taken.
pub async fn username_exists(conn: &Connection, username: String) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM users WHERE username = ?1")?;
        let count: i64 = stmt.query_row(params![username], |row: &rusqlite::Row| row.get(0))?;
        Ok(count > 0)
    })
    .await
}

/// Check if an email is already taken.
pub async fn email_exists(conn: &Connection, email: String) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM users WHERE email = ?1")?;
        let count: i64 = stmt.query_row(params![email], |row: &rusqlite::Row| row.get(0))?;
        Ok(count > 0)
    })
    .await
}

/// Get user by ID.
pub async fn get_user_by_id(conn: &Connection, user_id: i64) -> Result<Option<User>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, username, email, created_at, is_banned FROM users WHERE id = ?1",
        )?;
        let user = stmt
            .query_row(params![user_id], |row: &rusqlite::Row| {
                Ok(User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    email: row.get(2)?,
                    created_at: row.get(3)?,
                    is_banned: row.get::<_, i64>(4)? != 0,
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
            "SELECT id, username, email, created_at, is_banned FROM users WHERE username = ?1",
        )?;
        let user = stmt
            .query_row(params![username], |row: &rusqlite::Row| {
                Ok(User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    email: row.get(2)?,
                    created_at: row.get(3)?,
                    is_banned: row.get::<_, i64>(4)? != 0,
                })
            })
            .optional()?;
        Ok(user)
    })
    .await
}

/// Update a user's username.
pub async fn update_username(conn: &Connection, user_id: i64, new_username: String) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE users SET username = ?1 WHERE id = ?2",
            params![new_username, user_id],
        )?;
        info!("Username updated! username:{} userid:{}", new_username, user_id);
        Ok(())
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
            "SELECT id, username, email, created_at, is_banned
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
                })
            })?
            .collect::<std::result::Result<Vec<User>, rusqlite::Error>>()?;
        Ok(users)
    })
    .await
}

// ---------------------------------------------------------------------------
// Avatar
// ---------------------------------------------------------------------------

/// Return the on-disk path of a user's avatar, or `None` if none has been set.
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

/// Write the on-disk path of a newly uploaded avatar for `user_id`.
pub async fn set_user_avatar(conn: &Connection, user_id: i64, path: String) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE users SET avatar_path = ?1 WHERE id = ?2",
            params![path, user_id],
        )?;
        Ok(())
    })
    .await
}

/// Clear the stored avatar path (does **not** remove the file from disk).
/// Call this after the file has already been deleted.
pub async fn clear_user_avatar(conn: &Connection, user_id: i64) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE users SET avatar_path = NULL WHERE id = ?1",
            params![user_id],
        )?;
        Ok(())
    })
    .await
}
