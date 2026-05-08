use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::FromRow;
use sqlx::sqlite::SqlitePool;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow)]
pub struct FileRecord {
    pub id: i64,
    pub uploader_id: i64,
    pub chat_id: i64,
    /// Original filename as provided by the client.
    pub filename: String,
    pub mime_type: String,
    /// Size in bytes.
    pub size: i64,
    /// Path on disk where the file is stored (UUID-based to avoid collisions).
    pub storage_path: String,
    pub uploaded_at: i64,
    /// The message row that was created alongside this file upload (may be NULL
    /// for legacy rows but should always be set for new uploads).
    pub message_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewFileRecord {
    pub uploader_id: i64,
    pub chat_id: i64,
    pub filename: String,
    pub mime_type: String,
    pub size: i64,
    pub storage_path: String,
    pub message_id: Option<i64>,
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

/// Persist metadata for a newly uploaded file.
/// Returns the auto-generated `files.id`.
pub async fn store_file_record(pool: &SqlitePool, rec: NewFileRecord) -> anyhow::Result<i64> {
    let uploaded_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let res = sqlx::query(
        "INSERT INTO files
             (uploader_id, chat_id, filename, mime_type, size,
              storage_path, uploaded_at, message_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(rec.uploader_id)
    .bind(rec.chat_id)
    .bind(rec.filename)
    .bind(rec.mime_type)
    .bind(rec.size)
    .bind(rec.storage_path)
    .bind(uploaded_at)
    .bind(rec.message_id)
    .execute(pool)
    .await?;

    Ok(res.last_insert_rowid())
}

/// Back-fill `message_id` after the companion message row has been inserted.
pub async fn set_file_message_id(
    pool: &SqlitePool,
    file_id: i64,
    message_id: i64,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE files SET message_id = ? WHERE id = ?")
        .bind(message_id)
        .bind(file_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete a file record.  Callers are responsible for removing the file from
/// disk afterwards (so that a DB error doesn't leave orphaned bytes).
pub async fn delete_file_record(pool: &SqlitePool, file_id: i64) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM files WHERE id = ?")
        .bind(file_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

/// Fetch a single file record by its primary key.
pub async fn get_file(pool: &SqlitePool, file_id: i64) -> anyhow::Result<Option<FileRecord>> {
    let rec = sqlx::query_as::<_, FileRecord>(
        "SELECT id, uploader_id, chat_id, filename, mime_type, size,
                storage_path, uploaded_at, message_id
         FROM   files
         WHERE  id = ?",
    )
    .bind(file_id)
    .fetch_optional(pool)
    .await?;

    Ok(rec)
}

/// Fetch all files shared in a chat, newest first.
pub async fn get_files_for_chat(
    pool: &SqlitePool,
    chat_id: i64,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<FileRecord>> {
    let recs = sqlx::query_as::<_, FileRecord>(
        "SELECT id, uploader_id, chat_id, filename, mime_type, size,
                storage_path, uploaded_at, message_id
         FROM   files
         WHERE  chat_id = ?
         ORDER  BY uploaded_at DESC
         LIMIT  ? OFFSET ?",
    )
    .bind(chat_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(recs)
}

/// Check whether a file belongs to a given chat (used for access control).
pub async fn file_belongs_to_chat(
    pool: &SqlitePool,
    file_id: i64,
    chat_id: i64,
) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files WHERE id = ? AND chat_id = ?")
        .bind(file_id)
        .bind(chat_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0 > 0)
}
