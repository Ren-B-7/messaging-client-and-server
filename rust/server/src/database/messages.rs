use std::time::{SystemTime, UNIX_EPOCH};
use tokio_rusqlite::{Connection, Result, params, rusqlite};

#[derive(Debug, Clone)]
pub struct Message {
    pub id: i64,
    pub sender_id: i64,
    pub recipient_id: Option<i64>,
    pub group_id: Option<i64>,
    pub content: Vec<u8>, // Compressed/encrypted message data
    pub sent_at: i64,
    pub delivered_at: Option<i64>,
    pub read_at: Option<i64>,
    pub is_encrypted: bool,
    pub message_type: String,
}

#[derive(Debug, Clone)]
pub struct NewMessage {
    pub sender_id: i64,
    pub recipient_id: Option<i64>,
    pub group_id: Option<i64>,
    pub content: Vec<u8>,
    pub is_encrypted: bool,
    pub message_type: String,
}

/// Send a message
pub async fn send_message(conn: &Connection, new_message: NewMessage) -> Result<i64> {
    let sent_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "INSERT INTO messages (sender_id, recipient_id, group_id, content, sent_at, is_encrypted, message_type) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new_message.sender_id,
                new_message.recipient_id,
                new_message.group_id,
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

/// Get messages between two users
pub async fn get_direct_messages(
    conn: &Connection,
    user1_id: i64,
    user2_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<Message>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, sender_id, recipient_id, group_id, content, sent_at, delivered_at, read_at, is_encrypted, message_type
             FROM messages 
             WHERE (sender_id = ?1 AND recipient_id = ?2) OR (sender_id = ?2 AND recipient_id = ?1)
             ORDER BY sent_at DESC
             LIMIT ?3 OFFSET ?4"
        )?;

        let messages = stmt.query_map(params![user1_id, user2_id, limit, offset], |row| {
            Ok(Message {
                id: row.get(0)?,
                sender_id: row.get(1)?,
                recipient_id: row.get(2)?,
                group_id: row.get(3)?,
                content: row.get(4)?,
                sent_at: row.get(5)?,
                delivered_at: row.get(6)?,
                read_at: row.get(7)?,
                is_encrypted: row.get::<_, i64>(8)? != 0,
                message_type: row.get(9)?,
            })
        })?
        .collect::<std::result::Result<Vec<Message>, rusqlite::Error>>()?;

        Ok(messages)
    })
    .await
}

/// Get group messages
pub async fn get_group_messages(
    conn: &Connection,
    group_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<Message>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, sender_id, recipient_id, group_id, content, sent_at, delivered_at, read_at, is_encrypted, message_type
             FROM messages 
             WHERE group_id = ?1
             ORDER BY sent_at DESC
             LIMIT ?2 OFFSET ?3"
        )?;

        let messages = stmt.query_map(params![group_id, limit, offset], |row| {
            Ok(Message {
                id: row.get(0)?,
                sender_id: row.get(1)?,
                recipient_id: row.get(2)?,
                group_id: row.get(3)?,
                content: row.get(4)?,
                sent_at: row.get(5)?,
                delivered_at: row.get(6)?,
                read_at: row.get(7)?,
                is_encrypted: row.get::<_, i64>(8)? != 0,
                message_type: row.get(9)?,
            })
        })?
        .collect::<std::result::Result<Vec<Message>, rusqlite::Error>>()?;

        Ok(messages)
    })
    .await
}

/// Mark message as delivered
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

/// Mark message as read
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

/// Get unread message count for a user
pub async fn get_unread_count(conn: &Connection, user_id: i64) -> Result<i64> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM messages WHERE recipient_id = ?1 AND read_at IS NULL")?;
        let count: i64 = stmt.query_row(params![user_id], |row: &rusqlite::Row| row.get(0))?;
        Ok(count)
    })
    .await
}

/// Delete a message
pub async fn delete_message(conn: &Connection, message_id: i64, user_id: i64) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        // Only allow sender to delete
        let count = conn.execute(
            "DELETE FROM messages WHERE id = ?1 AND sender_id = ?2",
            params![message_id, user_id],
        )?;
        Ok(count > 0)
    })
    .await
}

/// Get recent conversations for a user
pub async fn get_recent_conversations(
    conn: &Connection,
    user_id: i64,
    limit: i64,
) -> Result<Vec<(i64, i64)>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT 
                CASE 
                    WHEN sender_id = ?1 THEN recipient_id 
                    ELSE sender_id 
                END as other_user_id,
                MAX(sent_at) as last_message_time
             FROM messages 
             WHERE (sender_id = ?1 OR recipient_id = ?1) AND group_id IS NULL
             GROUP BY other_user_id
             ORDER BY last_message_time DESC
             LIMIT ?2",
        )?;

        let conversations = stmt
            .query_map(params![user_id, limit], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<std::result::Result<Vec<(i64, i64)>, rusqlite::Error>>()?;

        Ok(conversations)
    })
    .await
}
