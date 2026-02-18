use tokio_rusqlite::{Connection, Result, rusqlite};

/// Initialize the database schema for the messaging service
pub async fn create_tables(conn: &Connection) -> Result<()> {
    conn.call(|conn: &mut rusqlite::Connection| {
        // Users table — is_admin = 1 marks admin accounts (same table, separate server)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                email TEXT UNIQUE,
                created_at INTEGER NOT NULL,
                last_login INTEGER,
                is_admin INTEGER NOT NULL DEFAULT 0,
                is_banned INTEGER NOT NULL DEFAULT 0,
                ban_reason TEXT,
                banned_at INTEGER,
                banned_by INTEGER,
                FOREIGN KEY (banned_by) REFERENCES users(id)
            )",
            [],
        )?;

        // Sessions table for active logins
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                session_token TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                last_activity INTEGER NOT NULL,
                ip_address TEXT,
                user_agent TEXT,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Messages table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sender_id INTEGER NOT NULL,
                recipient_id INTEGER,
                group_id INTEGER,
                content BLOB NOT NULL,
                sent_at INTEGER NOT NULL,
                delivered_at INTEGER,
                read_at INTEGER,
                is_encrypted INTEGER NOT NULL DEFAULT 1,
                message_type TEXT NOT NULL DEFAULT 'text',
                FOREIGN KEY (sender_id) REFERENCES users(id),
                FOREIGN KEY (recipient_id) REFERENCES users(id),
                FOREIGN KEY (group_id) REFERENCES groups(id)
            )",
            [],
        )?;

        // Groups table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                created_by INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                description TEXT,
                FOREIGN KEY (created_by) REFERENCES users(id)
            )",
            [],
        )?;

        // Group members table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS group_members (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                group_id INTEGER NOT NULL,
                user_id INTEGER NOT NULL,
                joined_at INTEGER NOT NULL,
                role TEXT NOT NULL DEFAULT 'member',
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                UNIQUE(group_id, user_id)
            )",
            [],
        )?;

        // Password reset tokens
        conn.execute(
            "CREATE TABLE IF NOT EXISTS password_reset_tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                token TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                used INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create indexes for better query performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_users_is_admin ON users(is_admin)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(session_token)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_sender ON messages(sender_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_recipient ON messages(recipient_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_group ON messages(group_id)",
            [],
        )?;

        Ok(())
    })
    .await
}

/// Open or create the database
pub async fn open_database(path: &str) -> Result<Connection> {
    let conn = Connection::open(path).await?;
    create_tables(&conn).await?;
    Ok(conn)
}
