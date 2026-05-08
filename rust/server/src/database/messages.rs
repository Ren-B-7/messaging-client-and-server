use std::time::{SystemTime, UNIX_EPOCH};

use shared::types::message::*;
use sqlx::sqlite::SqlitePool;

/// Send a message to a chat (group or direct).
pub async fn send_message(pool: &SqlitePool, new_message: NewMessage) -> anyhow::Result<i64> {
    let sent_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let res = sqlx::query(
        "INSERT INTO messages (sender_id, chat_id, content, sent_at, is_encrypted, message_type)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(new_message.sender_id)
    .bind(new_message.chat_id)
    .bind(new_message.content)
    .bind(sent_at)
    .bind(if new_message.is_encrypted { 1 } else { 0 })
    .bind(new_message.message_type)
    .execute(pool)
    .await?;

    Ok(res.last_insert_rowid())
}

/// Get messages for any chat (group or direct) by chat_id.
pub async fn get_chat_messages(
    pool: &SqlitePool,
    chat_id: i64,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<Message>> {
    let rows = sqlx::query_as::<_, (i64, i64, i64, Vec<u8>, i64, Option<i64>, Option<i64>, i64, String)>(
        "SELECT id, sender_id, chat_id, content, sent_at, delivered_at, read_at, is_encrypted, message_type
         FROM messages
         WHERE chat_id = ?
         ORDER BY sent_at DESC, id DESC
         LIMIT ? OFFSET ?"
    )
    .bind(chat_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Message {
            id: r.0,
            sender_id: r.1,
            chat_id: r.2,
            content: r.3,
            sent_at: r.4,
            delivered_at: r.5,
            read_at: r.6,
            is_encrypted: r.7 != 0,
            message_type: r.8,
        })
        .collect())
}

/// Fetch a single message by its primary key.
pub async fn get_message_by_id(
    pool: &SqlitePool,
    message_id: i64,
) -> anyhow::Result<Option<Message>> {
    let row = sqlx::query_as::<
        _,
        (
            i64,
            i64,
            i64,
            Vec<u8>,
            i64,
            Option<i64>,
            Option<i64>,
            i64,
            String,
        ),
    >(
        "SELECT id, sender_id, chat_id, content, sent_at, delivered_at, read_at,
                is_encrypted, message_type
         FROM   messages
         WHERE  id = ?",
    )
    .bind(message_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| Message {
        id: r.0,
        sender_id: r.1,
        chat_id: r.2,
        content: r.3,
        sent_at: r.4,
        delivered_at: r.5,
        read_at: r.6,
        is_encrypted: r.7 != 0,
        message_type: r.8,
    }))
}

/// Mark a message as delivered.
pub async fn mark_delivered(pool: &SqlitePool, message_id: i64) -> anyhow::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    sqlx::query("UPDATE messages SET delivered_at = ? WHERE id = ?")
        .bind(now)
        .bind(message_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Mark a message as read.
pub async fn mark_read(pool: &SqlitePool, message_id: i64) -> anyhow::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    sqlx::query("UPDATE messages SET read_at = ? WHERE id = ?")
        .bind(now)
        .bind(message_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Get total unread message count for a user across all their chats.
pub async fn get_unread_count(pool: &SqlitePool, user_id: i64) -> anyhow::Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM messages m
         INNER JOIN chat_members gm ON gm.chat_id = m.chat_id
         WHERE gm.user_id = ?
           AND m.sender_id != ?
           AND m.read_at IS NULL",
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Get unread message count for a user in a specific chat.
pub async fn get_unread_count_for_chat(
    pool: &SqlitePool,
    chat_id: i64,
    user_id: i64,
) -> anyhow::Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM messages
         WHERE chat_id = ?
           AND sender_id != ?
           AND read_at IS NULL",
    )
    .bind(chat_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Delete a message (only the sender may delete their own messages).
pub async fn delete_message(
    pool: &SqlitePool,
    message_id: i64,
    user_id: i64,
) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM messages WHERE id = ? AND sender_id = ?")
        .bind(message_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

/// Get all chats a user is in, ordered by the most recent message.
pub async fn get_recent_chats(
    pool: &SqlitePool,
    user_id: i64,
    limit: i64,
) -> anyhow::Result<Vec<(i64, i64)>> {
    let rows = sqlx::query_as::<_, (i64, Option<i64>)>(
        "SELECT gm.chat_id, MAX(m.sent_at) as last_message_time
         FROM chat_members gm
         LEFT JOIN messages m ON m.chat_id = gm.chat_id
         WHERE gm.user_id = ?
         GROUP BY gm.chat_id
         ORDER BY last_message_time DESC
         LIMIT ?",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| (r.0, r.1.unwrap_or(0))).collect())
}
