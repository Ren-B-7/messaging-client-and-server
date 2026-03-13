use std::time::{SystemTime, UNIX_EPOCH};

use tokio_rusqlite::{Connection, OptionalExtension, Result, params, rusqlite};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
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
pub async fn store_file_record(conn: &Connection, rec: NewFileRecord) -> Result<i64> {
    let uploaded_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "INSERT INTO files
                 (uploader_id, chat_id, filename, mime_type, size,
                  storage_path, uploaded_at, message_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                rec.uploader_id,
                rec.chat_id,
                rec.filename,
                rec.mime_type,
                rec.size,
                rec.storage_path,
                uploaded_at,
                rec.message_id,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    })
    .await
}

/// Back-fill `message_id` after the companion message row has been inserted.
pub async fn set_file_message_id(conn: &Connection, file_id: i64, message_id: i64) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE files SET message_id = ?1 WHERE id = ?2",
            params![message_id, file_id],
        )?;
        Ok(())
    })
    .await
}

/// Delete a file record.  Callers are responsible for removing the file from
/// disk afterwards (so that a DB error doesn't leave orphaned bytes).
pub async fn delete_file_record(conn: &Connection, file_id: i64) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let count = conn.execute("DELETE FROM files WHERE id = ?1", params![file_id])?;
        Ok(count > 0)
    })
    .await
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

/// Fetch a single file record by its primary key.
pub async fn get_file(conn: &Connection, file_id: i64) -> Result<Option<FileRecord>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, uploader_id, chat_id, filename, mime_type, size,
                    storage_path, uploaded_at, message_id
             FROM   files
             WHERE  id = ?1",
        )?;

        let rec = stmt
            .query_row(params![file_id], |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    uploader_id: row.get(1)?,
                    chat_id: row.get(2)?,
                    filename: row.get(3)?,
                    mime_type: row.get(4)?,
                    size: row.get(5)?,
                    storage_path: row.get(6)?,
                    uploaded_at: row.get(7)?,
                    message_id: row.get(8)?,
                })
            })
            .optional()?;

        Ok(rec)
    })
    .await
}

/// Fetch all files shared in a chat, newest first.
pub async fn get_files_for_chat(
    conn: &Connection,
    chat_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<FileRecord>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, uploader_id, chat_id, filename, mime_type, size,
                    storage_path, uploaded_at, message_id
             FROM   files
             WHERE  chat_id = ?1
             ORDER  BY uploaded_at DESC
             LIMIT  ?2 OFFSET ?3",
        )?;

        let recs = stmt
            .query_map(params![chat_id, limit, offset], |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    uploader_id: row.get(1)?,
                    chat_id: row.get(2)?,
                    filename: row.get(3)?,
                    mime_type: row.get(4)?,
                    size: row.get(5)?,
                    storage_path: row.get(6)?,
                    uploaded_at: row.get(7)?,
                    message_id: row.get(8)?,
                })
            })?
            .collect::<std::result::Result<Vec<FileRecord>, rusqlite::Error>>()?;

        Ok(recs)
    })
    .await
}

/// Check whether a file belongs to a given chat (used for access control).
pub async fn file_belongs_to_chat(conn: &Connection, file_id: i64, chat_id: i64) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM files WHERE id = ?1 AND chat_id = ?2")?;
        let count: i64 = stmt.query_row(params![file_id, chat_id], |row| row.get(0))?;
        Ok(count > 0)
    })
    .await
}
