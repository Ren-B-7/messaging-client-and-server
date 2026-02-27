use tokio_rusqlite::{Connection, Result, rusqlite};
use tracing::{info, warn};

/// Current schema version.  Bump this whenever the schema changes and add a
/// corresponding migration arm in `run_migrations`.
const SCHEMA_VERSION: u32 = 4;

/// Initialize the database schema and run any pending migrations.
pub async fn create_tables(conn: &Connection) -> Result<()> {
    create_schema(conn).await?;
    run_migrations(conn).await?;
    Ok(())
}

/// Create all tables for a brand-new database (version 4 schema).
async fn create_schema(conn: &Connection) -> Result<()> {
    conn.call(|conn: &mut rusqlite::Connection| {
        // Users table — is_admin = 1 marks admin accounts (same table, separate server)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                username      TEXT    NOT NULL UNIQUE,
                password_hash TEXT    NOT NULL,
                email         TEXT    UNIQUE,
                created_at    INTEGER NOT NULL,
                last_login    INTEGER,
                is_admin      INTEGER NOT NULL DEFAULT 0,
                is_banned     INTEGER NOT NULL DEFAULT 0,
                ban_reason    TEXT,
                banned_at     INTEGER,
                banned_by     INTEGER,
                FOREIGN KEY (banned_by) REFERENCES users(id)
            )",
            [],
        )?;

        // Sessions table (v2):
        //   - `session_id`  replaces the old `session_token` column
        //   - `user_agent`  column removed (now stored only in the JWT claims)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id       INTEGER NOT NULL,
                session_id    TEXT    NOT NULL UNIQUE,
                created_at    INTEGER NOT NULL,
                expires_at    INTEGER NOT NULL,
                last_activity INTEGER NOT NULL,
                ip_address    TEXT,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Messages table (v4): unified under chat_id; recipient_id retained for
        // potential legacy data but should be NULL for all new rows.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                sender_id    INTEGER NOT NULL,
                recipient_id INTEGER,
                chat_id      INTEGER,
                content      BLOB    NOT NULL,
                sent_at      INTEGER NOT NULL,
                delivered_at INTEGER,
                read_at      INTEGER,
                is_encrypted INTEGER NOT NULL DEFAULT 1,
                message_type TEXT    NOT NULL DEFAULT 'text',
                FOREIGN KEY (sender_id)    REFERENCES users(id),
                FOREIGN KEY (recipient_id) REFERENCES users(id),
                FOREIGN KEY (chat_id)      REFERENCES groups(id)
            )",
            [],
        )?;

        // Groups table (v3): `chat_type` distinguishes DMs from group chats.
        //   'direct' — a DM; all members are admins, no meaningful hierarchy
        //   'group'  — a group chat; creator is admin, others are members
        conn.execute(
            "CREATE TABLE IF NOT EXISTS groups (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT    NOT NULL,
                created_by  INTEGER NOT NULL,
                created_at  INTEGER NOT NULL,
                description TEXT,
                chat_type   TEXT    NOT NULL DEFAULT 'group',
                FOREIGN KEY (created_by) REFERENCES users(id)
            )",
            [],
        )?;

        // Group members table (v4): group_id renamed to chat_id.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS group_members (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_id   INTEGER NOT NULL,
                user_id   INTEGER NOT NULL,
                joined_at INTEGER NOT NULL,
                role      TEXT    NOT NULL DEFAULT 'member',
                FOREIGN KEY (chat_id) REFERENCES groups(id)  ON DELETE CASCADE,
                FOREIGN KEY (user_id) REFERENCES users(id)   ON DELETE CASCADE,
                UNIQUE(chat_id, user_id)
            )",
            [],
        )?;

        // Password reset tokens
        conn.execute(
            "CREATE TABLE IF NOT EXISTS password_reset_tokens (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id    INTEGER NOT NULL,
                token      TEXT    NOT NULL UNIQUE,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                used       INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // --- Indexes --------------------------------------------------------
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_users_username      ON users(username)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_users_email         ON users(email)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_users_is_admin      ON users(is_admin)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_session_id ON sessions(session_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_user_id    ON sessions(user_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_sender     ON messages(sender_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_chat       ON messages(chat_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_group_members_chat  ON group_members(chat_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_group_members_user  ON group_members(user_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_groups_chat_type    ON groups(chat_type)",
            [],
        )?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;

        Ok(())
    })
    .await
}

/// Apply any schema migrations required to reach `SCHEMA_VERSION`.
///
/// Uses `PRAGMA user_version` as the migration counter.
/// Each migration arm is idempotent — safe to run on a DB that was created
/// at any earlier version.
async fn run_migrations(conn: &Connection) -> Result<()> {
    let current_version: u32 = conn
        .call(|conn| {
            let v: u32 = conn
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap_or(0);
            Ok::<_, rusqlite::Error>(v)
        })
        .await?;

    if current_version >= SCHEMA_VERSION {
        return Ok(());
    }

    info!(
        "Database schema at version {}; target version {}. Running migrations…",
        current_version, SCHEMA_VERSION
    );

    // ── v1 → v2: rename session_token → session_id, drop user_agent ──────
    if current_version < 2 {
        let needs_migration: bool = conn
            .call(|conn| {
                let mut stmt = conn.prepare("PRAGMA table_info(sessions)")?;
                let old_col_exists = stmt
                    .query_map([], |row| {
                        let col_name: String = row.get(1)?;
                        Ok(col_name)
                    })?
                    .flatten()
                    .any(|name| name == "session_token");
                Ok::<_, rusqlite::Error>(old_col_exists)
            })
            .await?;

        if needs_migration {
            warn!(
                "Migrating sessions table from v1 to v2 (session_token → session_id, drop user_agent)…"
            );

            conn.call(|conn| {
                conn.execute_batch("
                    BEGIN;

                    CREATE TABLE sessions_v2 (
                        id            INTEGER PRIMARY KEY AUTOINCREMENT,
                        user_id       INTEGER NOT NULL,
                        session_id    TEXT    NOT NULL UNIQUE,
                        created_at    INTEGER NOT NULL,
                        expires_at    INTEGER NOT NULL,
                        last_activity INTEGER NOT NULL,
                        ip_address    TEXT,
                        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
                    );

                    INSERT INTO sessions_v2
                        (id, user_id, session_id, created_at, expires_at, last_activity, ip_address)
                    SELECT
                        id, user_id, session_token, created_at, expires_at, last_activity, ip_address
                    FROM sessions;

                    DROP TABLE sessions;

                    ALTER TABLE sessions_v2 RENAME TO sessions;

                    CREATE INDEX IF NOT EXISTS idx_sessions_session_id ON sessions(session_id);
                    CREATE INDEX IF NOT EXISTS idx_sessions_user_id    ON sessions(user_id);

                    COMMIT;
                ")?;
                Ok::<_, rusqlite::Error>(())
            })
            .await?;

            info!("Sessions table migration complete.");
        }

        conn.call(|conn| {
            conn.execute_batch("PRAGMA user_version = 2")?;
            Ok::<_, rusqlite::Error>(())
        })
        .await?;

        info!("Schema version set to 2.");
    }

    // ── v2 → v3: add chat_type column to groups ───────────────────────────
    if current_version < 3 {
        let needs_migration: bool = conn
            .call(|conn| {
                let mut stmt = conn.prepare("PRAGMA table_info(groups)")?;
                let col_missing = stmt
                    .query_map([], |row| {
                        let col_name: String = row.get(1)?;
                        Ok(col_name)
                    })?
                    .flatten()
                    .all(|name| name != "chat_type");
                Ok::<_, rusqlite::Error>(col_missing)
            })
            .await?;

        if needs_migration {
            warn!("Migrating groups table from v2 to v3 (add chat_type)…");

            conn.call(|conn| {
                conn.execute_batch(
                    "
                    BEGIN;
                    ALTER TABLE groups ADD COLUMN chat_type TEXT NOT NULL DEFAULT 'group';
                    CREATE INDEX IF NOT EXISTS idx_groups_chat_type ON groups(chat_type);
                    COMMIT;
                ",
                )?;
                Ok::<_, rusqlite::Error>(())
            })
            .await?;

            info!(
                "Groups table migration complete (chat_type backfilled as 'group' for all existing rows)."
            );
        }

        conn.call(|conn| {
            conn.execute_batch("PRAGMA user_version = 3")?;
            Ok::<_, rusqlite::Error>(())
        })
        .await?;

        info!("Schema version set to 3.");
    }

    // ── v3 → v4: rename group_members.group_id → chat_id,
    //             rename messages.group_id → chat_id ────────────────────────
    //
    // SQLite doesn't support RENAME COLUMN before 3.25.0, so we do a full
    // table rebuild for group_members.  For messages we use ALTER TABLE ADD
    // COLUMN + UPDATE + a note that the old group_id column is left in place
    // (SQLite cannot DROP columns before 3.35.0 — we just stop using it).
    if current_version < 4 {
        // ── group_members rebuild ──
        let needs_members_migration: bool = conn
            .call(|conn| {
                let mut stmt = conn.prepare("PRAGMA table_info(group_members)")?;
                let has_old_col = stmt
                    .query_map([], |row| {
                        let col_name: String = row.get(1)?;
                        Ok(col_name)
                    })?
                    .flatten()
                    .any(|name| name == "group_id");
                Ok::<_, rusqlite::Error>(has_old_col)
            })
            .await?;

        if needs_members_migration {
            warn!("Migrating group_members table from v3 to v4 (group_id → chat_id)…");

            conn.call(|conn| {
                conn.execute_batch(
                    "
                    BEGIN;

                    CREATE TABLE group_members_v4 (
                        id        INTEGER PRIMARY KEY AUTOINCREMENT,
                        chat_id   INTEGER NOT NULL,
                        user_id   INTEGER NOT NULL,
                        joined_at INTEGER NOT NULL,
                        role      TEXT    NOT NULL DEFAULT 'member',
                        FOREIGN KEY (chat_id) REFERENCES groups(id)  ON DELETE CASCADE,
                        FOREIGN KEY (user_id) REFERENCES users(id)   ON DELETE CASCADE,
                        UNIQUE(chat_id, user_id)
                    );

                    INSERT INTO group_members_v4 (id, chat_id, user_id, joined_at, role)
                    SELECT id, group_id, user_id, joined_at, role
                    FROM group_members;

                    DROP TABLE group_members;

                    ALTER TABLE group_members_v4 RENAME TO group_members;

                    CREATE INDEX IF NOT EXISTS idx_group_members_chat ON group_members(chat_id);
                    CREATE INDEX IF NOT EXISTS idx_group_members_user ON group_members(user_id);

                    COMMIT;
                ",
                )?;
                Ok::<_, rusqlite::Error>(())
            })
            .await?;

            info!("group_members migration complete.");
        }

        // ── messages: add chat_id, backfill from group_id ──
        let needs_messages_migration: bool = conn
            .call(|conn| {
                let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
                let has_chat_id = stmt
                    .query_map([], |row| {
                        let col_name: String = row.get(1)?;
                        Ok(col_name)
                    })?
                    .flatten()
                    .any(|name| name == "chat_id");
                Ok::<_, rusqlite::Error>(!has_chat_id)
            })
            .await?;

        if needs_messages_migration {
            warn!("Migrating messages table from v3 to v4 (group_id → chat_id)…");

            conn.call(|conn| {
                conn.execute_batch(
                    "
                    BEGIN;
                    ALTER TABLE messages ADD COLUMN chat_id INTEGER REFERENCES groups(id);
                    UPDATE messages SET chat_id = group_id WHERE group_id IS NOT NULL;
                    CREATE INDEX IF NOT EXISTS idx_messages_chat ON messages(chat_id);
                    COMMIT;
                ",
                )?;
                Ok::<_, rusqlite::Error>(())
            })
            .await?;

            info!("messages migration complete (chat_id backfilled from group_id).");
        }

        conn.call(|conn| {
            conn.execute_batch("PRAGMA user_version = 4")?;
            Ok::<_, rusqlite::Error>(())
        })
        .await?;

        info!("Schema version set to 4.");
    }

    // Add future migration arms here:
    // if current_version < 5 { ... }

    Ok(())
}

/// Open or create the database and ensure the schema is up to date.
pub async fn open_database(path: &str) -> Result<Connection> {
    let conn = Connection::open(path).await?;
    create_tables(&conn).await?;
    Ok(conn)
}
