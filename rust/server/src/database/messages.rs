use std::time::{SystemTime, UNIX_EPOCH};

use tokio_rusqlite::{Connection, Result, params, rusqlite};

use shared::types::message::*;

/// Send a message to a chat (group or direct).
pub async fn send_message(conn: &Connection, new_message: NewMessage) -> Result<i64> {
    let sent_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "INSERT INTO messages (sender_id, chat_id, content, sent_at, is_encrypted, message_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                new_message.sender_id,
                new_message.chat_id,
                new_message.content,
                sent_at,
                if new_message.is_encrypted { 1 } else { 0 },
                new_message.message_type,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    })
    .await
}

/// Get messages for any chat (group or direct) by chat_id.
pub async fn get_chat_messages(
    conn: &Connection,
    chat_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<Message>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, sender_id, chat_id, content, sent_at, delivered_at, read_at, is_encrypted, message_type
             FROM messages
             WHERE chat_id = ?1
             ORDER BY sent_at DESC
             LIMIT ?2 OFFSET ?3",
        )?;

        let messages = stmt
            .query_map(params![chat_id, limit, offset], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    sender_id: row.get(1)?,
                    chat_id: row.get(2)?,
                    content: row.get(3)?,
                    sent_at: row.get(4)?,
                    delivered_at: row.get(5)?,
                    read_at: row.get(6)?,
                    is_encrypted: row.get::<_, i64>(7)? != 0,
                    message_type: row.get(8)?,
                })
            })?
            .collect::<std::result::Result<Vec<Message>, rusqlite::Error>>()?;

        Ok(messages)
    })
    .await
}

/// Mark a message as delivered.
pub async fn mark_delivered(conn: &Connection, message_id: i64) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE messages SET delivered_at = ?1 WHERE id = ?2",
            params![now, message_id],
        )?;
        Ok(())
    })
    .await
}

/// Mark a message as read.
pub async fn mark_read(conn: &Connection, message_id: i64) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE messages SET read_at = ?1 WHERE id = ?2",
            params![now, message_id],
        )?;
        Ok(())
    })
    .await
}

/// Get total unread message count for a user across all their chats.
///
/// A message is unread when `read_at IS NULL` and the sender is not the user
/// themselves.  Uses `group_members` to find all chats the user belongs to.
pub async fn get_unread_count(conn: &Connection, user_id: i64) -> Result<i64> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT COUNT(*)
             FROM messages m
             INNER JOIN group_members gm ON gm.chat_id = m.chat_id
             WHERE gm.user_id = ?1
               AND m.sender_id != ?1
               AND m.read_at IS NULL",
        )?;
        let count: i64 = stmt.query_row(params![user_id], |row| row.get(0))?;
        Ok(count)
    })
    .await
}

/// Get unread message count for a user in a specific chat.
pub async fn get_unread_count_for_chat(
    conn: &Connection,
    chat_id: i64,
    user_id: i64,
) -> Result<i64> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT COUNT(*)
             FROM messages
             WHERE chat_id = ?1
               AND sender_id != ?2
               AND read_at IS NULL",
        )?;
        let count: i64 = stmt.query_row(params![chat_id, user_id], |row| row.get(0))?;
        Ok(count)
    })
    .await
}

/// Delete a message (only the sender may delete their own messages).
pub async fn delete_message(conn: &Connection, message_id: i64, user_id: i64) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let count = conn.execute(
            "DELETE FROM messages WHERE id = ?1 AND sender_id = ?2",
            params![message_id, user_id],
        )?;
        Ok(count > 0)
    })
    .await
}

/// Get all chats a user is in, ordered by the most recent message.
///
/// Returns `(chat_id, last_message_time)` pairs.  Replaces the old
/// `get_recent_conversations` which relied on the removed `recipient_id` logic.
pub async fn get_recent_chats(
    conn: &Connection,
    user_id: i64,
    limit: i64,
) -> Result<Vec<(i64, i64)>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT gm.chat_id, MAX(m.sent_at) as last_message_time
             FROM group_members gm
             LEFT JOIN messages m ON m.chat_id = gm.chat_id
             WHERE gm.user_id = ?1
             GROUP BY gm.chat_id
             ORDER BY last_message_time DESC
             LIMIT ?2",
        )?;

        let chats = stmt
            .query_map(params![user_id, limit], |row| {
                Ok((row.get(0)?, row.get(1).unwrap_or(0)))
            })?
            .collect::<std::result::Result<Vec<(i64, i64)>, rusqlite::Error>>()?;

        Ok(chats)
    })
    .await
}
