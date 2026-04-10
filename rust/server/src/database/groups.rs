// -----------------------------------------------------------------------
// database/groups.rs — delete_group cascade fix
// -----------------------------------------------------------------------
// Previously delete_group deleted group_members and groups rows but left
// behind all messages and files belonging to that chat. SQLite does not
// enforce FK cascades unless PRAGMA foreign_keys = ON is set at connection
// open time.  Until that PRAGMA is added, the deletes must be explicit.
//
// The fixed delete_group function removes in the correct dependency order:
//   1. files (references messages + groups)
//   2. messages (references groups)
//   3. group_members (references groups)
//   4. groups
// All inside a single transaction so the group is never half-deleted.
// -----------------------------------------------------------------------

use std::time::{SystemTime, UNIX_EPOCH};

use tokio_rusqlite::{Connection, OptionalExtension, Result, params, rusqlite};

use shared::types::groups::*;

pub async fn create_group(conn: &Connection, new_group: NewGroup) -> Result<i64> {
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let chat_id = conn
        .call(move |conn: &mut rusqlite::Connection| {
            conn.execute(
                "INSERT INTO groups (name, created_by, created_at, description, chat_type)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    new_group.name,
                    new_group.created_by,
                    created_at,
                    new_group.description,
                    new_group.chat_type,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
        .await?;

    add_group_member(conn, chat_id, new_group.created_by, "admin".to_string()).await?;
    Ok(chat_id)
}

pub async fn add_group_member(
    conn: &Connection,
    chat_id: i64,
    user_id: i64,
    role: String,
) -> Result<i64> {
    let joined_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "INSERT INTO group_members (chat_id, user_id, joined_at, role)
             VALUES (?1, ?2, ?3, ?4)",
            params![chat_id, user_id, joined_at, role],
        )?;
        Ok(conn.last_insert_rowid())
    })
    .await
}

pub async fn remove_group_member(conn: &Connection, chat_id: i64, user_id: i64) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let count = conn.execute(
            "DELETE FROM group_members WHERE chat_id = ?1 AND user_id = ?2",
            params![chat_id, user_id],
        )?;
        Ok(count > 0)
    })
    .await
}

pub async fn get_group_members(conn: &Connection, chat_id: i64) -> Result<Vec<GroupMember>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, chat_id, user_id, joined_at, role
             FROM group_members
             WHERE chat_id = ?1",
        )?;

        let members = stmt
            .query_map(params![chat_id], |row: &rusqlite::Row| {
                Ok(GroupMember {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    user_id: row.get(2)?,
                    joined_at: row.get(3)?,
                    role: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<GroupMember>, rusqlite::Error>>()?;

        Ok(members)
    })
    .await
}

pub async fn get_user_groups(conn: &Connection, user_id: i64) -> Result<Vec<Group>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT g.id, g.name, g.created_by, g.created_at, g.description, g.chat_type
             FROM groups g
             INNER JOIN group_members gm ON g.id = gm.chat_id
             WHERE gm.user_id = ?1
             ORDER BY g.created_at DESC",
        )?;

        let groups = stmt
            .query_map(params![user_id], |row: &rusqlite::Row| {
                Ok(Group {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_by: row.get(2)?,
                    created_at: row.get(3)?,
                    description: row.get(4)?,
                    chat_type: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<Group>, rusqlite::Error>>()?;

        Ok(groups)
    })
    .await
}

pub async fn get_group(conn: &Connection, chat_id: i64) -> Result<Option<Group>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, name, created_by, created_at, description, chat_type
             FROM groups
             WHERE id = ?1",
        )?;

        let group = stmt
            .query_row(params![chat_id], |row: &rusqlite::Row| {
                Ok(Group {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_by: row.get(2)?,
                    created_at: row.get(3)?,
                    description: row.get(4)?,
                    chat_type: row.get(5)?,
                })
            })
            .optional()?;

        Ok(group)
    })
    .await
}

pub async fn is_group_member(conn: &Connection, chat_id: i64, user_id: i64) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt =
            conn.prepare("SELECT COUNT(*) FROM group_members WHERE chat_id = ?1 AND user_id = ?2")?;
        let count: i64 = stmt.query_row(params![chat_id, user_id], |row| row.get(0))?;
        Ok(count > 0)
    })
    .await
}

pub async fn is_group_admin(conn: &Connection, chat_id: i64, user_id: i64) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) FROM group_members
             WHERE chat_id = ?1 AND user_id = ?2 AND role = 'admin'",
        )?;
        let count: i64 = stmt.query_row(params![chat_id, user_id], |row| row.get(0))?;
        Ok(count > 0)
    })
    .await
}

pub async fn find_existing_dm(
    conn: &Connection,
    user1_id: i64,
    user2_id: i64,
) -> Result<Option<i64>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        // Find a 'direct' chat where:
        // 1. Both user1 and user2 are members.
        // 2. The total member count is exactly 2.
        // This ensures we find the unique DM between these two users.
        let mut stmt = conn.prepare(
            "SELECT g.id
             FROM groups g
             JOIN group_members gm1 ON g.id = gm1.chat_id AND gm1.user_id = ?1
             JOIN group_members gm2 ON g.id = gm2.chat_id AND gm2.user_id = ?2
             WHERE g.chat_type = 'direct'
             AND (SELECT COUNT(*) FROM group_members WHERE chat_id = g.id) = 2
             LIMIT 1",
        )?;

        let chat_id = stmt
            .query_row(params![user1_id, user2_id], |row: &rusqlite::Row| {
                row.get(0)
            })
            .optional()?;

        Ok(chat_id)
    })
    .await
}

pub async fn update_group_name(conn: &Connection, chat_id: i64, new_name: String) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE groups SET name = ?1 WHERE id = ?2",
            params![new_name, chat_id],
        )?;
        Ok(())
    })
    .await
}

pub async fn update_group_description(
    conn: &Connection,
    chat_id: i64,
    new_description: String,
) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE groups SET description = ?1 WHERE id = ?2",
            params![new_description, chat_id],
        )?;
        Ok(())
    })
    .await
}

/// Delete a group and all data that belongs to it.
///
/// Deletes in dependency order inside a single transaction:
///   1. `files`         — references `messages` and `groups`
///   2. `messages`      — references `groups`
///   3. `group_members` — references `groups`
///   4. `groups`        — the root row
///
/// Previously only `group_members` and `groups` were deleted, leaving
/// orphaned `messages` and `files` rows that accumulated indefinitely.
/// SQLite does not enforce FK cascades by default
/// (`PRAGMA foreign_keys` defaults to OFF), so the deletes must be explicit
/// until that PRAGMA is enabled at connection-open time.
pub async fn delete_group(conn: &Connection, chat_id: i64) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let tx = conn.transaction()?;

        // 1. Files that belong to this chat (must go before messages because
        //    the files table has a nullable FK to messages).
        tx.execute("DELETE FROM files WHERE chat_id = ?1", params![chat_id])?;

        // 2. Messages in this chat.
        tx.execute("DELETE FROM messages WHERE chat_id = ?1", params![chat_id])?;

        // 3. Group membership rows.
        tx.execute(
            "DELETE FROM group_members WHERE chat_id = ?1",
            params![chat_id],
        )?;

        // 4. The group itself.
        tx.execute("DELETE FROM groups WHERE id = ?1", params![chat_id])?;

        tx.commit()?;
        Ok(())
    })
    .await
}

pub async fn update_member_role(
    conn: &Connection,
    chat_id: i64,
    user_id: i64,
    new_role: String,
) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE group_members SET role = ?1 WHERE chat_id = ?2 AND user_id = ?3",
            params![new_role, chat_id, user_id],
        )?;
        Ok(())
    })
    .await
}

/// Return all groups/chats a user belongs to, ordered by most-recent message activity.
///
/// Previously `get_user_groups` ordered by `g.created_at DESC`, which meant
/// a chat created a year ago but messaged today would appear at the bottom of
/// the list.  This function replaces it for the chat-list endpoint, using
/// `COALESCE(MAX(m.sent_at), g.created_at)` as the sort key so chats bubble
/// up when they receive new messages — matching every standard messenger UX.
///
/// Chats with no messages are sorted by their creation time and appear after
/// active chats (because `COALESCE` falls back to `created_at`).
pub async fn get_user_groups_by_activity(conn: &Connection, user_id: i64) -> Result<Vec<Group>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT g.id, g.name, g.created_by, g.created_at, g.description, g.chat_type
             FROM   groups g
             INNER JOIN group_members gm ON g.id = gm.chat_id
             LEFT  JOIN messages m       ON m.chat_id = g.id
             WHERE  gm.user_id = ?1
             GROUP  BY g.id
             ORDER  BY COALESCE(MAX(m.sent_at), g.created_at) DESC",
        )?;

        let groups = stmt
            .query_map(params![user_id], |row: &rusqlite::Row| {
                Ok(Group {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_by: row.get(2)?,
                    created_at: row.get(3)?,
                    description: row.get(4)?,
                    chat_type: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<Group>, rusqlite::Error>>()?;

        Ok(groups)
    })
    .await
}
