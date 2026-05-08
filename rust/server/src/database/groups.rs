// -----------------------------------------------------------------------
// database/groups.rs — Refactored to sqlx and chats/chat_members schema
// -----------------------------------------------------------------------

use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::sqlite::SqlitePool;

use shared::types::groups::*;

// Note: Using FromRow on types from shared might require those types to derive it.
// If shared types don't derive FromRow, we'll map manually in the functions.
// Assuming for now they don't, so we map manually or use local wrappers.

pub async fn create_group(pool: &SqlitePool, new_group: NewGroup) -> anyhow::Result<i64> {
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut tx = pool.begin().await?;

    let res = sqlx::query(
        "INSERT INTO chats (name, created_by, created_at, description, chat_type)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&new_group.name)
    .bind(new_group.created_by)
    .bind(created_at)
    .bind(&new_group.description)
    .bind(&new_group.chat_type)
    .execute(&mut *tx)
    .await?;

    let chat_id = res.last_insert_rowid();

    let joined_at = created_at;
    sqlx::query(
        "INSERT INTO chat_members (chat_id, user_id, joined_at, role)
         VALUES (?, ?, ?, ?)",
    )
    .bind(chat_id)
    .bind(new_group.created_by)
    .bind(joined_at)
    .bind("admin")
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(chat_id)
}

pub async fn add_group_member(
    pool: &SqlitePool,
    chat_id: i64,
    user_id: i64,
    role: String,
) -> anyhow::Result<i64> {
    let joined_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let res = sqlx::query(
        "INSERT INTO chat_members (chat_id, user_id, joined_at, role)
         VALUES (?, ?, ?, ?)",
    )
    .bind(chat_id)
    .bind(user_id)
    .bind(joined_at)
    .bind(role)
    .execute(pool)
    .await?;

    Ok(res.last_insert_rowid())
}

pub async fn remove_group_member(
    pool: &SqlitePool,
    chat_id: i64,
    user_id: i64,
) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM chat_members WHERE chat_id = ? AND user_id = ?")
        .bind(chat_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn get_group_members(
    pool: &SqlitePool,
    chat_id: i64,
) -> anyhow::Result<Vec<GroupMember>> {
    let rows = sqlx::query_as::<_, (i64, i64, i64, i64, String)>(
        "SELECT id, chat_id, user_id, joined_at, role
         FROM chat_members
         WHERE chat_id = ?",
    )
    .bind(chat_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| GroupMember {
            id: r.0,
            chat_id: r.1,
            user_id: r.2,
            joined_at: r.3,
            role: r.4,
        })
        .collect())
}

pub async fn get_user_groups(pool: &SqlitePool, user_id: i64) -> anyhow::Result<Vec<Group>> {
    let rows = sqlx::query_as::<_, (i64, String, i64, i64, Option<String>, String)>(
        "SELECT g.id, g.name, g.created_by, g.created_at, g.description, g.chat_type
         FROM chats g
         INNER JOIN chat_members gm ON g.id = gm.chat_id
         WHERE gm.user_id = ?
         ORDER BY g.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Group {
            id: r.0,
            name: r.1,
            created_by: r.2,
            created_at: r.3,
            description: r.4,
            chat_type: r.5,
        })
        .collect())
}

pub async fn get_group(pool: &SqlitePool, chat_id: i64) -> anyhow::Result<Option<Group>> {
    let row = sqlx::query_as::<_, (i64, String, i64, i64, Option<String>, String)>(
        "SELECT id, name, created_by, created_at, description, chat_type
         FROM chats
         WHERE id = ?",
    )
    .bind(chat_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| Group {
        id: r.0,
        name: r.1,
        created_by: r.2,
        created_at: r.3,
        description: r.4,
        chat_type: r.5,
    }))
}

pub async fn is_group_member(
    pool: &SqlitePool,
    chat_id: i64,
    user_id: i64,
) -> anyhow::Result<bool> {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM chat_members WHERE chat_id = ? AND user_id = ?")
            .bind(chat_id)
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(row.0 > 0)
}

pub async fn is_group_admin(pool: &SqlitePool, chat_id: i64, user_id: i64) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM chat_members
         WHERE chat_id = ? AND user_id = ? AND role = 'admin'",
    )
    .bind(chat_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0 > 0)
}

pub async fn find_existing_dm(
    pool: &SqlitePool,
    user1_id: i64,
    user2_id: i64,
) -> anyhow::Result<Option<i64>> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT g.id
         FROM chats g
         JOIN chat_members gm1 ON g.id = gm1.chat_id AND gm1.user_id = ?
         JOIN chat_members gm2 ON g.id = gm2.chat_id AND gm2.user_id = ?
         WHERE g.chat_type = 'direct'
         AND (SELECT COUNT(*) FROM chat_members WHERE chat_id = g.id) = 2
         LIMIT 1",
    )
    .bind(user1_id)
    .bind(user2_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.0))
}

pub async fn update_group_name(
    pool: &SqlitePool,
    chat_id: i64,
    new_name: String,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE chats SET name = ? WHERE id = ?")
        .bind(new_name)
        .bind(chat_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_group_description(
    pool: &SqlitePool,
    chat_id: i64,
    new_description: String,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE chats SET description = ? WHERE id = ?")
        .bind(new_description)
        .bind(chat_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete a group and all data that belongs to it.
pub async fn delete_group(pool: &SqlitePool, chat_id: i64) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;

    // 1. Files that belong to this chat
    sqlx::query("DELETE FROM files WHERE chat_id = ?")
        .bind(chat_id)
        .execute(&mut *tx)
        .await?;

    // 2. Messages in this chat.
    sqlx::query("DELETE FROM messages WHERE chat_id = ?")
        .bind(chat_id)
        .execute(&mut *tx)
        .await?;

    // 3. Group membership rows.
    sqlx::query("DELETE FROM chat_members WHERE chat_id = ?")
        .bind(chat_id)
        .execute(&mut *tx)
        .await?;

    // 4. The group itself.
    sqlx::query("DELETE FROM chats WHERE id = ?")
        .bind(chat_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn update_member_role(
    pool: &SqlitePool,
    chat_id: i64,
    user_id: i64,
    new_role: String,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE chat_members SET role = ? WHERE chat_id = ? AND user_id = ?")
        .bind(new_role)
        .bind(chat_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Return all groups/chats a user belongs to, ordered by most-recent message activity.
pub async fn get_user_groups_by_activity(
    pool: &SqlitePool,
    user_id: i64,
) -> anyhow::Result<Vec<Group>> {
    let rows = sqlx::query_as::<_, (i64, String, i64, i64, Option<String>, String)>(
        "SELECT g.id, g.name, g.created_by, g.created_at, g.description, g.chat_type
         FROM   chats g
         INNER JOIN chat_members gm ON g.id = gm.chat_id
         LEFT  JOIN messages m       ON m.chat_id = g.id
         WHERE  gm.user_id = ?
         GROUP  BY g.id
         ORDER  BY COALESCE(MAX(m.sent_at), g.created_at) DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Group {
            id: r.0,
            name: r.1,
            created_by: r.2,
            created_at: r.3,
            description: r.4,
            chat_type: r.5,
        })
        .collect())
}
