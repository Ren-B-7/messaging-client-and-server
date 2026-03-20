//! File-sharing handlers
//!
//! Routes
//! ──────
//!   POST   /api/files/upload      (hard auth) — multipart upload
//!   GET    /api/files/:id         (light auth) — download / stream a file
//!   GET    /api/files?chat_id=N   (light auth) — list files in a chat
//!   DELETE /api/files/:id         (hard auth)  — delete own file

use std::convert::Infallible;
use std::path::PathBuf;

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response, StatusCode, header};
use multer::Multipart;
use tokio_rusqlite::rusqlite;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::AppState;
use crate::database::{groups, utils};
use crate::handlers::http::utils::{deliver_error_json, deliver_serialized_json};
use shared::types::jwt::JwtClaims;
use shared::types::sse::SseEvent;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Hard cap on a single uploaded file (50 MiB).
pub const MAX_FILE_SIZE: usize = 50 * 1024 * 1024;

/// How many files to return per page by default.
pub const DEFAULT_PAGE_SIZE: i64 = 50;

// ---------------------------------------------------------------------------
// POST /api/files/upload
// ---------------------------------------------------------------------------

/// Accept a `multipart/form-data` upload, persist the bytes to disk, record
/// metadata in the DB, create a companion `file` message, and broadcast a
/// `file_shared` SSE event to all chat members.
///
/// Expected form fields:
///   - `file`     — the binary payload (required)
///   - `chat_id`  — destination chat (required)
///   - `filename` — original filename override (optional; falls back to the
///                  `filename` from the `Content-Disposition` header)
///
/// # Atomicity
///
/// The `files` row and the companion `messages` row are inserted inside a
/// single SQLite transaction.  Previously these were separate inserts with a
/// back-fill step, which left an orphaned `files` row (and bytes on disk) if
/// the message insert failed.  Now both inserts succeed or fail together.
///
/// Note: the disk write still happens before the transaction.  If the process
/// crashes between the disk write and the transaction commit, unreferenced
/// bytes may accumulate under `uploads_dir`.  A periodic scrub of files not
/// referenced in the `files` table is the correct mitigation for that edge
/// case, but is not yet implemented.
pub async fn handle_upload_file(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("File upload request from user {}", user_id);

    // ── 1. Parse Content-Type boundary ──────────────────────────────────────
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let boundary = multer::parse_boundary(content_type)
        .map_err(|e| anyhow::anyhow!("Invalid multipart boundary: {}", e))?;

    // ── 2. Stream the multipart body ─────────────────────────────────────────
    let body_stream = req.into_body().into_data_stream();
    let mut multipart = Multipart::new(body_stream, boundary);

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut original_filename: Option<String> = None;
    let mut mime_type: Option<String> = None;
    let mut chat_id: Option<i64> = None;

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| anyhow::anyhow!("Multipart read error: {}", e))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                original_filename = field.file_name().map(|s| s.to_string());
                mime_type = field.content_type().map(|m| m.to_string());

                let mut buf = Vec::new();
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|e| anyhow::anyhow!("Read chunk error: {}", e))?
                {
                    buf.extend_from_slice(&chunk);
                    if buf.len() > MAX_FILE_SIZE {
                        return deliver_error_json(
                            "FILE_TOO_LARGE",
                            &format!("File exceeds the {} MiB limit", MAX_FILE_SIZE / 1024 / 1024),
                            StatusCode::PAYLOAD_TOO_LARGE,
                        );
                    }
                }
                file_bytes = Some(buf);
            }
            "chat_id" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| anyhow::anyhow!("chat_id read error: {}", e))?;
                chat_id = text.trim().parse::<i64>().ok();
            }
            "filename" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| anyhow::anyhow!("filename read error: {}", e))?;
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    original_filename = Some(trimmed);
                }
            }
            _ => {
                // Drain unknown fields silently.
                while field
                    .chunk()
                    .await
                    .map_err(|e| anyhow::anyhow!("drain error: {}", e))?
                    .is_some()
                {}
            }
        }
    }

    // ── 3. Validate required fields ──────────────────────────────────────────
    let file_bytes = match file_bytes {
        Some(b) if !b.is_empty() => b,
        _ => {
            return deliver_error_json(
                "MISSING_FILE",
                "No file data received",
                StatusCode::BAD_REQUEST,
            );
        }
    };

    let chat_id = match chat_id {
        Some(id) => id,
        None => {
            return deliver_error_json(
                "MISSING_CHAT_ID",
                "chat_id is required",
                StatusCode::BAD_REQUEST,
            );
        }
    };

    let filename = sanitize_filename(original_filename.as_deref().unwrap_or("unnamed"));
    let mime_type = mime_type.unwrap_or_else(|| "application/octet-stream".to_string());

    // ── 4. Authorization — must be a chat member ─────────────────────────────
    let is_member = groups::is_group_member(&state.db, chat_id, user_id)
        .await
        .context("DB error checking membership")?;

    if !is_member {
        return deliver_error_json(
            "FORBIDDEN",
            "You are not a member of this chat",
            StatusCode::FORBIDDEN,
        );
    }

    // ── 5. Write bytes to disk ───────────────────────────────────────────────
    let storage_dir = state.config.read().await.paths.uploads_dir.clone();
    let storage_path = build_storage_path(&storage_dir, &filename);

    tokio::fs::create_dir_all(&storage_dir)
        .await
        .context("Failed to create uploads directory")?;

    tokio::fs::write(&storage_path, &file_bytes)
        .await
        .with_context(|| format!("Failed to write file to {:?}", storage_path))?;

    let file_size = file_bytes.len() as i64;
    let storage_path_str = storage_path.to_string_lossy().to_string();

    // ── 6. Build compressed message content ──────────────────────────────────
    //
    // Note: `file_id` is not yet known at this point — it is assigned by
    // SQLite's auto-increment during the transaction below.  The message
    // content therefore carries filename/mime/size metadata only.  Clients
    // that need the `file_id` can use the `file_id` field in the HTTP
    // response or the `file_shared` SSE event, both of which include it.
    let msg_content_json = serde_json::json!({
        "filename":  filename,
        "mime_type": mime_type,
        "size":      file_size,
    })
    .to_string();

    let compressed = utils::compress_data(msg_content_json.as_bytes())
        .context("Failed to compress message content")?;

    let sent_at = utils::get_timestamp();

    // ── 7. Atomic DB transaction: insert message + file together ─────────────
    //
    // Previously these were two separate async DB calls with a back-fill step:
    //   store_file_record(message_id: None)  →  file_id
    //   send_message(...)                    →  message_id
    //   set_file_message_id(file_id, ...)    (back-fill)
    //
    // If `send_message` failed, the `files` row was orphaned (no message_id,
    // unreachable via the API).  Combining both INSERTs in a single
    // `conn.transaction()` ensures they succeed or fail atomically.
    let filename_c = filename.clone();
    let mime_type_c = mime_type.clone();

    let (file_id, message_id) = state
        .db
        .call(move |conn| {
            let tx = conn.transaction()?;

            // Insert the companion message first.
            tx.execute(
                "INSERT INTO messages (sender_id, chat_id, content, sent_at, is_encrypted, message_type)
                 VALUES (?1, ?2, ?3, ?4, 1, 'file')",
                rusqlite::params![user_id, chat_id, compressed, sent_at],
            )?;
            let message_id = tx.last_insert_rowid();

            // Insert the file record referencing the message.
            tx.execute(
                "INSERT INTO files
                     (uploader_id, chat_id, filename, mime_type, size,
                      storage_path, uploaded_at, message_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    user_id,
                    chat_id,
                    filename_c,
                    mime_type_c,
                    file_size,
                    storage_path_str,
                    sent_at,
                    message_id,
                ],
            )?;
            let file_id = tx.last_insert_rowid();

            tx.commit()?;
            Ok::<_, rusqlite::Error>((file_id, message_id))
        })
        .await
        .context("Failed to complete file upload transaction")?;

    info!(
        "File {} uploaded by user {} to chat {} (message {})",
        file_id, user_id, chat_id, message_id
    );

    // ── 8. SSE broadcast ─────────────────────────────────────────────────────
    sse_broadcast_file_shared(
        &state, file_id, user_id, chat_id, &filename, &mime_type, file_size, message_id,
    )
    .await;

    // ── 9. Respond ───────────────────────────────────────────────────────────
    deliver_serialized_json(
        &serde_json::json!({
            "status":     "success",
            "file_id":    file_id,
            "message_id": message_id,
            "filename":   filename,
            "mime_type":  mime_type,
            "size":       file_size,
        }),
        StatusCode::CREATED,
    )
}

// ---------------------------------------------------------------------------
// GET /api/files/:id
// ---------------------------------------------------------------------------

/// Stream a file to the client.
///
/// Access control: the requesting user must be a member of the chat the file
/// belongs to.  The original filename and MIME type are restored from the DB
/// so the browser handles the download correctly.
pub async fn handle_download_file(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    claims: JwtClaims,
    file_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::files;

    let user_id = claims.user_id;
    info!("File download: file={} user={}", file_id, user_id);

    // ── 1. Load metadata ─────────────────────────────────────────────────────
    let rec = match files::get_file(&state.db, file_id).await? {
        Some(r) => r,
        None => {
            return deliver_error_json("NOT_FOUND", "File not found", StatusCode::NOT_FOUND);
        }
    };

    // ── 2. Auth — must be a chat member ──────────────────────────────────────
    let is_member = groups::is_group_member(&state.db, rec.chat_id, user_id)
        .await
        .context("DB error checking membership")?;

    if !is_member {
        return deliver_error_json(
            "FORBIDDEN",
            "You do not have access to this file",
            StatusCode::FORBIDDEN,
        );
    }

    // ── 3. Check for inline vs attachment preference ──────────────────────────
    let inline = req
        .uri()
        .query()
        .unwrap_or("")
        .split('&')
        .any(|p| p == "inline=1" || p == "inline=true");

    // ── 4. Read from disk ────────────────────────────────────────────────────
    let file_bytes = tokio::fs::read(&rec.storage_path)
        .await
        .with_context(|| format!("Failed to read file from {:?}", rec.storage_path))?;

    // ── 5. Build response ────────────────────────────────────────────────────
    let disposition = if inline {
        format!("inline; filename=\"{}\"", rec.filename)
    } else {
        format!("attachment; filename=\"{}\"", rec.filename)
    };

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, rec.mime_type)
        .header(header::CONTENT_LENGTH, file_bytes.len())
        .header(header::CONTENT_DISPOSITION, disposition)
        .header("cache-control", "private, max-age=86400")
        .body(Full::new(Bytes::from(file_bytes)).boxed())
        .context("Failed to build file response")?;

    Ok(response)
}

// ---------------------------------------------------------------------------
// GET /api/files?chat_id=N
// ---------------------------------------------------------------------------

/// List files shared in a chat, newest first.
///
/// Query params:
///   - `chat_id` (required)
///   - `limit`   (optional, default 50, max 100)
///   - `offset`  (optional, default 0)
pub async fn handle_get_chat_files(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::files;

    let user_id = claims.user_id;

    let params: std::collections::HashMap<String, String> =
        form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
            .into_owned()
            .collect();

    let chat_id = match params.get("chat_id").and_then(|s| s.parse::<i64>().ok()) {
        Some(id) => id,
        None => {
            return deliver_error_json(
                "MISSING_CHAT_ID",
                "chat_id query parameter is required",
                StatusCode::BAD_REQUEST,
            );
        }
    };

    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .min(100);

    let offset = params
        .get("offset")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let is_member = groups::is_group_member(&state.db, chat_id, user_id)
        .await
        .context("DB error checking membership")?;

    if !is_member {
        return deliver_error_json(
            "FORBIDDEN",
            "You are not a member of this chat",
            StatusCode::FORBIDDEN,
        );
    }

    let file_list = files::get_files_for_chat(&state.db, chat_id, limit, offset)
        .await
        .context("Failed to fetch files")?;

    let files_json: Vec<serde_json::Value> = file_list
        .into_iter()
        .map(|f| {
            serde_json::json!({
                "id":          f.id,
                "uploader_id": f.uploader_id,
                "chat_id":     f.chat_id,
                "filename":    f.filename,
                "mime_type":   f.mime_type,
                "size":        f.size,
                "uploaded_at": f.uploaded_at,
                "message_id":  f.message_id,
            })
        })
        .collect();

    deliver_serialized_json(
        &serde_json::json!({
            "status": "success",
            "data": {
                "files": files_json,
                "limit":  limit,
                "offset": offset,
            }
        }),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// DELETE /api/files/:id
// ---------------------------------------------------------------------------

/// Delete a file.  Only the uploader (or an admin) may delete their files.
/// Removes both the DB record and the bytes on disk.
pub async fn handle_delete_file(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
    file_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::files;

    info!("Delete file {} requested by user {}", file_id, user_id);

    let rec = match files::get_file(&state.db, file_id).await? {
        Some(r) => r,
        None => {
            return deliver_error_json("NOT_FOUND", "File not found", StatusCode::NOT_FOUND);
        }
    };

    if rec.uploader_id != user_id {
        return deliver_error_json(
            "FORBIDDEN",
            "You can only delete your own files",
            StatusCode::FORBIDDEN,
        );
    }

    // Remove DB record first so a crash between DB and disk doesn't leak access.
    let deleted = files::delete_file_record(&state.db, file_id)
        .await
        .context("Failed to delete file record")?;

    if !deleted {
        return deliver_error_json("NOT_FOUND", "File not found", StatusCode::NOT_FOUND);
    }

    // Best-effort disk removal — log but don't fail the request if it errors.
    if let Err(e) = tokio::fs::remove_file(&rec.storage_path).await {
        warn!(
            "Could not remove file {:?} from disk: {}",
            rec.storage_path, e
        );
    } else {
        info!("Deleted file {:?} from disk", rec.storage_path);
    }

    deliver_serialized_json(
        &serde_json::json!({ "status": "success", "message": "File deleted" }),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// SSE helper
// ---------------------------------------------------------------------------

async fn sse_broadcast_file_shared(
    state: &AppState,
    file_id: i64,
    uploader_id: i64,
    chat_id: i64,
    filename: &str,
    mime_type: &str,
    size: i64,
    message_id: i64,
) {
    let now = utils::get_timestamp();

    let members = match groups::get_group_members(&state.db, chat_id).await {
        Ok(m) => m,
        Err(e) => {
            error!(
                "SSE file_shared: failed to fetch members for chat {}: {}",
                chat_id, e
            );
            return;
        }
    };

    let recipients: Vec<String> = members.iter().map(|m| m.user_id.to_string()).collect();

    let event = SseEvent {
        user_id: String::new(),
        event_type: "file_shared".to_string(),
        data: serde_json::json!({
            "file_id":     file_id,
            "chat_id":     chat_id,
            "uploader_id": uploader_id,
            "filename":    filename,
            "mime_type":   mime_type,
            "size":        size,
            "message_id":  message_id,
            "uploaded_at": now,
        }),
        timestamp: now,
    };

    if let Err(e) = state
        .sse_manager
        .broadcast_to_users(event, recipients)
        .await
    {
        error!("SSE file_shared broadcast failed: {:?}", e);
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Derive a collision-free on-disk path for a new upload.
///
/// Layout: `<uploads_dir>/<uuid>_<sanitized-filename>`
pub fn build_storage_path(uploads_dir: &str, filename: &str) -> PathBuf {
    let uuid = Uuid::new_v4().to_string();
    let stored_name = format!("{}_{}", uuid, filename);
    PathBuf::from(uploads_dir).join(stored_name)
}

/// Strip directory traversal, null bytes, and other hazardous characters
/// from a filename supplied by the client.
pub fn sanitize_filename(name: &str) -> String {
    let base = name
        .replace('\\', "/")
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".." && *s != ".")
        .last()
        .unwrap_or("unnamed")
        .to_string();

    base.chars()
        .filter(|c| !c.is_control() && *c != '\0')
        .collect()
}

// needed for handle_get_chat_files
use form_urlencoded;
