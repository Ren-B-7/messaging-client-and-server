use crate::database::common::setup_test_db;
use server::database::ban;
use tokio_rusqlite::{params, rusqlite::Error};

/// Insert a minimal user row so ban operations have something to UPDATE.
///
/// `ban_user` issues `UPDATE users SET is_banned = 1 … WHERE id = ?` — if
/// the row doesn't exist the UPDATE silently affects 0 rows and
/// `get_ban_info` returns `None`.  The original tests skipped this step
/// (the `// ... setup user ...` comment was never filled in).
async fn insert_test_user(conn: &tokio_rusqlite::Connection, user_id: i64, username: &str) {
    let username = username.to_string();
    conn.call(move |c| {
        c.execute(
            "INSERT INTO users (id, username, password_hash, created_at, is_banned)
             VALUES (?1, ?2, 'hash', 0, 0)",
            params![user_id, username],
        )?;
        Ok::<(), Error>(())
    })
    .await
    .expect("Failed to insert test user");
}

#[tokio::test]
async fn test_ban_user_and_check_status() {
    let conn = setup_test_db().await;

    // Create the user that will be banned.
    insert_test_user(&conn, 1, "alice").await;

    ban::ban_user(&conn, 1, 99, Some("Terms of Service violation".into()))
        .await
        .unwrap();

    let info = ban::get_ban_info(&conn, 1).await.unwrap().unwrap();
    assert!(info.is_banned);
    assert_eq!(info.ban_reason, Some("Terms of Service violation".into()));
}

#[tokio::test]
async fn test_unban_user() {
    let conn = setup_test_db().await;

    // Create and ban the user, then verify unban works.
    insert_test_user(&conn, 1, "alice").await;

    ban::ban_user(&conn, 1, 99, None).await.unwrap();
    ban::unban_user(&conn, 1).await.unwrap();

    let info = ban::get_ban_info(&conn, 1).await.unwrap().unwrap();
    assert!(!info.is_banned);
}
