use crate::database::common::setup_test_db;
use server::database::messages;
use shared::types::message::NewMessage;
use tokio_rusqlite::{params, rusqlite::Error};

/// Insert the minimum rows required for message operations:
///   users row (sender)  →  groups row  →  group_members row  →  message row.
///
/// `messages::send_message` and `messages::delete_message` do not check FK
/// constraints at the application level, but SQLite will enforce them once
/// `PRAGMA foreign_keys = ON` is set.  The helper inserts in dependency order
/// so both the FK-on and FK-off cases work correctly.
async fn setup_message_fixtures(conn: &tokio_rusqlite::Connection, user_id: i64, chat_id: i64) {
    conn.call(move |c| {
        // User
        c.execute(
            "INSERT INTO users (id, username, password_hash, created_at, is_banned)
             VALUES (?1, 'sender', 'hash', 0, 0)",
            params![user_id],
        )?;
        // Chat (group)
        c.execute(
            "INSERT INTO groups (id, name, created_by, created_at, chat_type)
             VALUES (?1, 'test-chat', ?2, 0, 'direct')",
            params![chat_id, user_id],
        )?;
        // Membership
        c.execute(
            "INSERT INTO group_members (chat_id, user_id, joined_at, role)
             VALUES (?1, ?2, 0, 'admin')",
            params![chat_id, user_id],
        )?;
        Ok::<(), Error>(())
    })
    .await
    .expect("Failed to insert message fixtures");
}

#[tokio::test]
async fn test_send_and_retrieve_message() {
    let conn = setup_test_db().await;
    setup_message_fixtures(&conn, 1, 101).await;

    let msg = NewMessage {
        sender_id: 1,
        chat_id: 101,
        content: b"Hello Rust!".to_vec(),
        is_encrypted: false,
        message_type: "text".into(),
    };

    let msg_id = messages::send_message(&conn, msg).await.unwrap();
    let history = messages::get_chat_messages(&conn, 101, 10, 0)
        .await
        .unwrap();

    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, msg_id);
    assert_eq!(history[0].content, b"Hello Rust!");
}

#[tokio::test]
async fn test_delete_message_permission() {
    let conn = setup_test_db().await;
    setup_message_fixtures(&conn, 1, 101).await;

    // Insert the message we will try to delete.
    let msg_id = messages::send_message(
        &conn,
        NewMessage {
            sender_id: 1,
            chat_id: 101,
            content: b"to be deleted".to_vec(),
            is_encrypted: false,
            message_type: "text".into(),
        },
    )
    .await
    .unwrap();

    // Wrong user (999) cannot delete a message they did not send.
    let deleted_by_wrong_user = messages::delete_message(&conn, msg_id, 999).await.unwrap();
    assert!(!deleted_by_wrong_user);

    // The message must still exist after the failed attempt.
    let still_there = messages::get_message_by_id(&conn, msg_id).await.unwrap();
    assert!(
        still_there.is_some(),
        "message should still exist after failed delete"
    );

    // The actual sender (1) can delete their own message.
    let deleted_by_owner = messages::delete_message(&conn, msg_id, 1).await.unwrap();
    assert!(deleted_by_owner);

    // Confirm it is gone.
    let gone = messages::get_message_by_id(&conn, msg_id).await.unwrap();
    assert!(gone.is_none(), "message should be gone after owner delete");
}
