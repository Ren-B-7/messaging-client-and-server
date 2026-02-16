use std::time::{SystemTime, UNIX_EPOCH};
use tokio_rusqlite::{Connection, OptionalExtension, Result, params};

#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: String,
    pub password_hash: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub created_at: i64,
    pub is_banned: bool,
}

/// Register a new user
pub async fn register_user(conn: &Connection, new_user: NewUser) -> Result<i64> {
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn| {
        conn.execute(
            "INSERT INTO users (username, password_hash, email, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                new_user.username,
                new_user.password_hash,
                new_user.email,
                created_at
            ],
        )?;

        Ok(conn.last_insert_rowid())
    })
    .await
}

/// Check if username exists
pub async fn username_exists(conn: &Connection, username: String) -> Result<bool> {
    conn.call(move |conn| {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM users WHERE username = ?1")?;
        let count: i64 = stmt.query_row(params![username], |row| row.get(0))?;
        Ok(count > 0)
    })
    .await
}

/// Check if email exists
pub async fn email_exists(conn: &Connection, email: String) -> Result<bool> {
    conn.call(move |conn| {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM users WHERE email = ?1")?;
        let count: i64 = stmt.query_row(params![email], |row| row.get(0))?;
        Ok(count > 0)
    })
    .await
}

/// Get user by ID
pub async fn get_user_by_id(conn: &Connection, user_id: i64) -> Result<Option<User>> {
    conn.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, username, email, created_at, is_banned FROM users WHERE id = ?1",
        )?;

        let user = stmt
            .query_row(params![user_id], |row| {
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

/// Get user by username
pub async fn get_user_by_username(conn: &Connection, username: String) -> Result<Option<User>> {
    conn.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, username, email, created_at, is_banned FROM users WHERE username = ?1",
        )?;

        let user = stmt
            .query_row(params![username], |row| {
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

/// Update username
pub async fn update_username(conn: &Connection, user_id: i64, new_username: String) -> Result<()> {
    conn.call(move |conn| {
        conn.execute(
            "UPDATE users SET username = ?1 WHERE id = ?2",
            params![new_username, user_id],
        )?;
        Ok(())
    })
    .await
}
