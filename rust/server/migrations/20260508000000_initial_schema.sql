-- Initial schema migration
-- Users table
CREATE TABLE IF NOT EXISTS users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT    NOT NULL UNIQUE,
    password_hash TEXT    NOT NULL,
    email         TEXT    UNIQUE,
    first_name    TEXT,
    last_name     TEXT,
    avatar_path   TEXT,
    is_admin      BOOLEAN NOT NULL DEFAULT 0,
    is_banned     BOOLEAN NOT NULL DEFAULT 0,
    ban_reason    TEXT,
    banned_at     INTEGER,
    banned_by     INTEGER,
    created_at    INTEGER NOT NULL,
    last_login    INTEGER,
    FOREIGN KEY (banned_by) REFERENCES users(id)
);

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id       INTEGER NOT NULL,
    session_id    TEXT    NOT NULL UNIQUE,
    created_at    INTEGER NOT NULL,
    expires_at    INTEGER NOT NULL,
    last_activity INTEGER NOT NULL,
    ip_address    TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Chats table (replaces groups)
CREATE TABLE IF NOT EXISTS chats (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL,
    description TEXT,
    chat_type   TEXT    NOT NULL DEFAULT 'group', -- 'direct' or 'group'
    created_by  INTEGER NOT NULL,
    created_at  INTEGER NOT NULL,
    FOREIGN KEY (created_by) REFERENCES users(id)
);

-- Chat members table (replaces group_members)
CREATE TABLE IF NOT EXISTS chat_members (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id   INTEGER NOT NULL,
    user_id   INTEGER NOT NULL,
    joined_at INTEGER NOT NULL,
    role      TEXT    NOT NULL DEFAULT 'member', -- 'admin' or 'member'
    FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE(chat_id, user_id)
);

-- Messages table (recipient_id removed)
CREATE TABLE IF NOT EXISTS messages (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    sender_id    INTEGER NOT NULL,
    chat_id      INTEGER NOT NULL,
    content      BLOB    NOT NULL,
    message_type TEXT    NOT NULL DEFAULT 'text',
    is_encrypted BOOLEAN NOT NULL DEFAULT 1,
    sent_at      INTEGER NOT NULL,
    delivered_at INTEGER,
    read_at      INTEGER,
    FOREIGN KEY (sender_id) REFERENCES users(id),
    FOREIGN KEY (chat_id)   REFERENCES chats(id)
);

-- Password reset tokens
CREATE TABLE IF NOT EXISTS password_reset_tokens (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    INTEGER NOT NULL,
    token      TEXT    NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    used       BOOLEAN NOT NULL DEFAULT 0,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Files
CREATE TABLE IF NOT EXISTS files (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    uploader_id  INTEGER NOT NULL,
    chat_id      INTEGER NOT NULL,
    message_id   INTEGER,
    filename     TEXT    NOT NULL,
    mime_type    TEXT    NOT NULL DEFAULT 'application/octet-stream',
    size         INTEGER NOT NULL,
    storage_path TEXT    NOT NULL UNIQUE,
    uploaded_at  INTEGER NOT NULL,
    FOREIGN KEY (uploader_id) REFERENCES users(id)    ON DELETE CASCADE,
    FOREIGN KEY (chat_id)     REFERENCES chats(id)    ON DELETE CASCADE,
    FOREIGN KEY (message_id)  REFERENCES messages(id) ON DELETE SET NULL
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_users_username      ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_email         ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_is_admin      ON users(is_admin);
CREATE INDEX IF NOT EXISTS idx_sessions_session_id ON sessions(session_id);
CREATE INDEX IF NOT EXISTS idx_sessions_user_id    ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_messages_sender     ON messages(sender_id);
CREATE INDEX IF NOT EXISTS idx_messages_chat       ON messages(chat_id);
CREATE INDEX IF NOT EXISTS idx_chat_members_chat   ON chat_members(chat_id);
CREATE INDEX IF NOT EXISTS idx_chat_members_user   ON chat_members(user_id);
CREATE INDEX IF NOT EXISTS idx_chats_chat_type     ON chats(chat_type);
CREATE INDEX IF NOT EXISTS idx_files_chat          ON files(chat_id);
CREATE INDEX IF NOT EXISTS idx_files_uploader      ON files(uploader_id);
