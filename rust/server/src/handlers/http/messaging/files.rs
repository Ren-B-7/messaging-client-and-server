//! File-sharing handlers

use std::convert::Infallible;

use anyhow::Context;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response, StatusCode, header};
use multer::Multipart;
use tracing::{error, info, warn};

use crate::AppState;
use crate::database::{files, groups, utils};
use crate::handlers::http::utils::{deliver_error_json, deliver_serialized_json};
use shared::types::jwt::JwtClaims;
use shared::types::sse::SseEvent;

pub const MAX_FILE_SIZE: usize = 50 * 1024 * 1024;
pub const DEFAULT_PAGE_SIZE: i64 = 50;

/// Accept a `multipart/form-data` upload, persist the bytes to disk, record
/// metadata in the DB, create a companion `file` message, and broadcast a
/// `file_shared` SSE event to all chat members.
pub async fn handle_upload_file(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("File upload request from user {}", user_id);

    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let boundary = multer::parse_boundary(content_type)
        .map_err(|e| anyhow::anyhow!("Invalid multipart boundary: {}", e))?;

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
                while field
                    .chunk()
                    .await
                    .map_err(|e| anyhow::anyhow!("drain error: {}", e))?
                    .is_some()
                {}
            }
        }
    }

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

    let filename = utils::sanitize_filename(original_filename.as_deref().unwrap_or("unnamed"));
    let mime_type = mime_type.unwrap_or_else(|| "application/octet-stream".to_string());

    if !utils::is_allowed_mime_type(&mime_type) {
        return deliver_error_json(
            "INVALID_FILE_TYPE",
            "File type not allowed",
            StatusCode::BAD_REQUEST,
        );
    }

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

    let storage_dir = state.config.read().await.paths.uploads_dir.clone();
    let storage_path = utils::build_storage_path(&storage_dir, &filename);

    tokio::fs::create_dir_all(&storage_dir)
        .await
        .context("Failed to create uploads directory")?;

    tokio::fs::write(&storage_path, &file_bytes)
        .await
        .with_context(|| format!("Failed to write file to {:?}", storage_path))?;

    let file_size = file_bytes.len() as i64;
    let storage_path_str = storage_path.to_string_lossy().to_string();

    let msg_content_json = serde_json::json!({
        "filename":  filename,
        "mime_type": mime_type,
        "size":      file_size,
    })
    .to_string();

    let compressed = utils::compress_data(msg_content_json.as_bytes())
        .context("Failed to compress message content")?;

    let sent_at = utils::get_timestamp();

    let mut tx = state.db.begin().await?;

    let res_msg = sqlx::query(
        "INSERT INTO messages (sender_id, chat_id, content, sent_at, is_encrypted, message_type)
         VALUES (?, ?, ?, ?, 1, 'file')",
    )
    .bind(user_id)
    .bind(chat_id)
    .bind(compressed)
    .bind(sent_at)
    .execute(&mut *tx)
    .await?;
    let message_id = res_msg.last_insert_rowid();

    let res_file = sqlx::query(
        "INSERT INTO files
             (uploader_id, chat_id, filename, mime_type, size,
              storage_path, uploaded_at, message_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(chat_id)
    .bind(&filename)
    .bind(&mime_type)
    .bind(file_size)
    .bind(storage_path_str)
    .bind(sent_at)
    .bind(message_id)
    .execute(&mut *tx)
    .await?;
    let file_id = res_file.last_insert_rowid();

    tx.commit().await?;

    info!(
        "File {} uploaded by user {} to chat {} (message {})",
        file_id, user_id, chat_id, message_id
    );

    sse_broadcast_file_shared(
        &state, file_id, user_id, chat_id, &filename, &mime_type, file_size,
    )
    .await;

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

/// Stream a file to the client.
pub async fn handle_download_file(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    claims: JwtClaims,
    file_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    let user_id = claims.user_id;
    info!("File download: file={} user={}", file_id, user_id);

    let rec = match files::get_file(&state.db, file_id).await? {
        Some(r) => r,
        None => {
            return deliver_error_json("NOT_FOUND", "File not found", StatusCode::NOT_FOUND);
        }
    };

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

    let inline = req
        .uri()
        .query()
        .unwrap_or("")
        .split('&')
        .any(|p| p == "inline=1" || p == "inline=true");

    let file_bytes = tokio::fs::read(&rec.storage_path)
        .await
        .with_context(|| format!("Failed to read file from {:?}", rec.storage_path))?;

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

/// List files shared in a chat, newest first.
pub async fn handle_get_chat_files(
    req: Request<hyper::body::Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
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

/// Delete a file and its companion message atomically.
pub async fn handle_delete_file(
    _req: Request<hyper::body::Incoming>,
    state: AppState,
    user_id: i64,
    file_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
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

    let companion_message_id = rec.message_id;
    let storage_path = rec.storage_path.clone();

    let mut tx = state.db.begin().await?;

    let res = sqlx::query("DELETE FROM files WHERE id = ?")
        .bind(file_id)
        .execute(&mut *tx)
        .await?;

    if let Some(msg_id) = companion_message_id {
        sqlx::query("DELETE FROM messages WHERE id = ?")
            .bind(msg_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    let deleted = res.rows_affected() > 0;

    if !deleted {
        return deliver_error_json("NOT_FOUND", "File not found", StatusCode::NOT_FOUND);
    }

    if let Err(e) = tokio::fs::remove_file(&storage_path).await {
        warn!("Could not remove file {:?} from disk: {}", storage_path, e);
    } else {
        info!("Deleted file {:?} from disk", storage_path);
    }

    deliver_serialized_json(
        &serde_json::json!({ "status": "success", "message": "File deleted" }),
        StatusCode::OK,
    )
}

async fn sse_broadcast_file_shared(
    state: &AppState,
    file_id: i64,
    uploader_id: i64,
    chat_id: i64,
    filename: &str,
    mime_type: &str,
    size: i64,
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

    let recipients: Vec<i64> = members.iter().map(|m| m.user_id).collect();

    let event = SseEvent {
        user_id: 0,
        event_type: "file_shared".to_string(),
        data: serde_json::json!({
            "file_id":     file_id,
            "chat_id":     chat_id,
            "uploader_id": uploader_id,
            "filename":    filename,
            "mime_type":   mime_type,
            "size":        size,
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
