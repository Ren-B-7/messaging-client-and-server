use std::time::{SystemTime, UNIX_EPOCH};

use tokio_rusqlite::{Connection, OptionalExtension, Result, params, rusqlite};

#[derive(Debug, Clone)]
pub struct Group {
    pub id: i64,
    pub name: String,
    pub created_by: i64,
    pub created_at: i64,
    pub description: Option<String>,
    pub chat_type: String,
}

#[derive(Debug, Clone)]
pub struct GroupMember {
    pub id: i64,
    pub group_id: i64,
    pub user_id: i64,
    pub joined_at: i64,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct NewGroup {
    pub name: String,
    pub created_by: i64,
    pub description: Option<String>,
    /// Either "direct" or "group".
    pub chat_type: String,
}

/// Create a new group or direct chat.
///
/// The creator is always added as the first member.  For `chat_type = "group"`
/// the creator receives the `"admin"` role.  For `chat_type = "direct"` they
/// receive `"admin"` too — every participant in a DM is an admin because there
/// is no meaningful moderation hierarchy in a direct message.
pub async fn create_group(conn: &Connection, new_group: NewGroup) -> Result<i64> {
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let group_id = conn
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

    // Creator is always an admin regardless of chat type.
    add_group_member(conn, group_id, new_group.created_by, "admin".to_string()).await?;

    Ok(group_id)
}

/// Add a member to a group.
///
/// For direct chats, callers should always pass `"admin"` as the role so that
/// every participant has equal standing.
pub async fn add_group_member(
    conn: &Connection,
    group_id: i64,
    user_id: i64,
    role: String,
) -> Result<i64> {
    let joined_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "INSERT INTO group_members (group_id, user_id, joined_at, role)
             VALUES (?1, ?2, ?3, ?4)",
            params![group_id, user_id, joined_at, role],
        )?;
        Ok(conn.last_insert_rowid())
    })
    .await
}

/// Remove a member from a group.
pub async fn remove_group_member(conn: &Connection, group_id: i64, user_id: i64) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let count = conn.execute(
            "DELETE FROM group_members WHERE group_id = ?1 AND user_id = ?2",
            params![group_id, user_id],
        )?;
        Ok(count > 0)
    })
    .await
}

/// Get all members of a group.
pub async fn get_group_members(conn: &Connection, group_id: i64) -> Result<Vec<GroupMember>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, group_id, user_id, joined_at, role
             FROM group_members
             WHERE group_id = ?1",
        )?;

        let members = stmt
            .query_map(params![group_id], |row: &rusqlite::Row| {
                Ok(GroupMember {
                    id: row.get(0)?,
                    group_id: row.get(1)?,
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

/// Get all groups/chats a user is a member of.
pub async fn get_user_groups(conn: &Connection, user_id: i64) -> Result<Vec<Group>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT g.id, g.name, g.created_by, g.created_at, g.description, g.chat_type
             FROM groups g
             INNER JOIN group_members gm ON g.id = gm.group_id
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

/// Get a group/chat by ID.
pub async fn get_group(conn: &Connection, group_id: i64) -> Result<Option<Group>> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT id, name, created_by, created_at, description, chat_type
             FROM groups
             WHERE id = ?1",
        )?;

        let group = stmt
            .query_row(params![group_id], |row: &rusqlite::Row| {
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

/// Check if a user is a member of a group.
pub async fn is_group_member(conn: &Connection, group_id: i64, user_id: i64) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM group_members WHERE group_id = ?1 AND user_id = ?2")?;
        let count: i64 = stmt.query_row(params![group_id, user_id], |row| row.get(0))?;
        Ok(count > 0)
    })
    .await
}

/// Check if a user is an admin of a group.
pub async fn is_group_admin(conn: &Connection, group_id: i64, user_id: i64) -> Result<bool> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) FROM group_members
             WHERE group_id = ?1 AND user_id = ?2 AND role = 'admin'",
        )?;
        let count: i64 = stmt.query_row(params![group_id, user_id], |row| row.get(0))?;
        Ok(count > 0)
    })
    .await
}

/// Update a group's name.
pub async fn update_group_name(conn: &Connection, group_id: i64, new_name: String) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE groups SET name = ?1 WHERE id = ?2",
            params![new_name, group_id],
        )?;
        Ok(())
    })
    .await
}

/// Update a group's description.
pub async fn update_group_description(
    conn: &Connection,
    group_id: i64,
    new_description: String,
) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE groups SET description = ?1 WHERE id = ?2",
            params![new_description, group_id],
        )?;
        Ok(())
    })
    .await
}

/// Delete a group and all its members.
pub async fn delete_group(conn: &Connection, group_id: i64) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        // Cascade handles members, but being explicit is fine too.
        conn.execute(
            "DELETE FROM group_members WHERE group_id = ?1",
            params![group_id],
        )?;
        conn.execute("DELETE FROM groups WHERE id = ?1", params![group_id])?;
        Ok(())
    })
    .await
}

/// Update the role of a group member.
pub async fn update_member_role(
    conn: &Connection,
    group_id: i64,
    user_id: i64,
    new_role: String,
) -> Result<()> {
    conn.call(move |conn: &mut rusqlite::Connection| {
        conn.execute(
            "UPDATE group_members SET role = ?1 WHERE group_id = ?2 AND user_id = ?3",
            params![new_role, group_id, user_id],
        )?;
        Ok(())
    })
    .await
}
