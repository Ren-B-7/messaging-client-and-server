use crate::database::common::setup_test_db;
use server::database::messages;
use shared::types::message::NewMessage;
use sqlx::sqlite::SqlitePool;

async fn setup_fixtures(pool: &SqlitePool, user_id: i64, chat_id: i64) {
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, created_at, is_banned)
         VALUES (?, 'sender', 'hash', 0, 0)",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .expect("setup_fixtures user failed");

    sqlx::query(
        "INSERT INTO chats (id, name, created_by, created_at, chat_type)
         VALUES (?, 'chat', ?, 0, 'direct')",
    )
    .bind(chat_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("setup_fixtures chat failed");

    sqlx::query(
        "INSERT INTO chat_members (chat_id, user_id, joined_at, role)
         VALUES (?, ?, 0, 'admin')",
    )
    .bind(chat_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("setup_fixtures member failed");
}

async fn setup_two_users_in_chat(pool: &SqlitePool, user1: i64, user2: i64, chat_id: i64) {
    for (id, name) in [(user1, "alice"), (user2, "bob")] {
        sqlx::query(
            "INSERT OR IGNORE INTO users (id, username, password_hash, created_at, is_banned)
             VALUES (?, ?, 'hash', 0, 0)",
        )
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .expect("setup_two_users_in_chat user failed");
    }

    sqlx::query(
        "INSERT INTO chats (id, name, created_by, created_at, chat_type)
         VALUES (?, 'dm', ?, 0, 'direct')",
    )
    .bind(chat_id)
    .bind(user1)
    .execute(pool)
    .await
    .expect("setup_two_users_in_chat chat failed");

    for uid in [user1, user2] {
        sqlx::query(
            "INSERT INTO chat_members (chat_id, user_id, joined_at, role)
             VALUES (?, ?, 0, 'member')",
        )
        .bind(chat_id)
        .bind(uid)
        .execute(pool)
        .await
        .expect("setup_two_users_in_chat member failed");
    }
}

// ── send_message / get_chat_messages ──────────────────────────────────────

#[tokio::test]
async fn send_and_retrieve_message() {
    let pool = setup_test_db().await;
    setup_fixtures(&pool, 1, 101).await;

    let msg_id = messages::send_message(
        &pool,
        NewMessage {
            sender_id: 1,
            chat_id: 101,
            content: b"Hello Rust!".to_vec(),
            is_encrypted: false,
            message_type: "text".into(),
        },
    )
    .await
    .unwrap();

    let history = messages::get_chat_messages(&pool, 101, 10, 0)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, msg_id);
    assert_eq!(history[0].content, b"Hello Rust!");
}

#[tokio::test]
async fn get_chat_messages_returns_newest_first() {
    let pool = setup_test_db().await;
    setup_fixtures(&pool, 1, 101).await;

    for i in 0..3u8 {
        messages::send_message(
            &pool,
            NewMessage {
                sender_id: 1,
                chat_id: 101,
                content: vec![i],
                is_encrypted: false,
                message_type: "text".into(),
            },
        )
        .await
        .unwrap();
    }

    let history = messages::get_chat_messages(&pool, 101, 10, 0)
        .await
        .unwrap();
    // DESC ordering: highest id first
    assert!(
        history[0].id > history[1].id,
        "messages should be ordered newest-first"
    );
}

#[tokio::test]
async fn get_chat_messages_respects_limit() {
    let pool = setup_test_db().await;
    setup_fixtures(&pool, 1, 101).await;

    for _ in 0..5 {
        messages::send_message(
            &pool,
            NewMessage {
                sender_id: 1,
                chat_id: 101,
                content: b"msg".to_vec(),
                is_encrypted: false,
                message_type: "text".into(),
            },
        )
        .await
        .unwrap();
    }

    let history = messages::get_chat_messages(&pool, 101, 3, 0).await.unwrap();
    assert_eq!(history.len(), 3);
}

// ── delete_message ────────────────────────────────────────────────────────

#[tokio::test]
async fn owner_can_delete_own_message() {
    let pool = setup_test_db().await;
    setup_fixtures(&pool, 1, 101).await;

    let msg_id = messages::send_message(
        &pool,
        NewMessage {
            sender_id: 1,
            chat_id: 101,
            content: b"delete me".to_vec(),
            is_encrypted: false,
            message_type: "text".into(),
        },
    )
    .await
    .unwrap();

    let deleted = messages::delete_message(&pool, msg_id, 1).await.unwrap();
    assert!(deleted);

    let gone = messages::get_message_by_id(&pool, msg_id).await.unwrap();
    assert!(gone.is_none());
}

#[tokio::test]
async fn non_owner_cannot_delete_message() {
    let pool = setup_test_db().await;
    setup_fixtures(&pool, 1, 101).await;

    let msg_id = messages::send_message(
        &pool,
        NewMessage {
            sender_id: 1,
            chat_id: 101,
            content: b"not yours".to_vec(),
            is_encrypted: false,
            message_type: "text".into(),
        },
    )
    .await
    .unwrap();

    let deleted = messages::delete_message(&pool, msg_id, 999).await.unwrap();
    assert!(!deleted, "wrong user must not be able to delete");

    let still_there = messages::get_message_by_id(&pool, msg_id).await.unwrap();
    assert!(still_there.is_some());
}

// ── mark_delivered / mark_read ────────────────────────────────────────────

#[tokio::test]
async fn mark_delivered_sets_delivered_at() {
    let pool = setup_test_db().await;
    setup_fixtures(&pool, 1, 101).await;

    let msg_id = messages::send_message(
        &pool,
        NewMessage {
            sender_id: 1,
            chat_id: 101,
            content: b"deliver".to_vec(),
            is_encrypted: false,
            message_type: "text".into(),
        },
    )
    .await
    .unwrap();

    messages::mark_delivered(&pool, msg_id).await.unwrap();

    let msg = messages::get_message_by_id(&pool, msg_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        msg.delivered_at.is_some(),
        "delivered_at must be set after mark_delivered"
    );
}

#[tokio::test]
async fn mark_read_sets_read_at() {
    let pool = setup_test_db().await;
    setup_fixtures(&pool, 1, 101).await;

    let msg_id = messages::send_message(
        &pool,
        NewMessage {
            sender_id: 1,
            chat_id: 101,
            content: b"read me".to_vec(),
            is_encrypted: false,
            message_type: "text".into(),
        },
    )
    .await
    .unwrap();

    messages::mark_read(&pool, msg_id).await.unwrap();

    let msg = messages::get_message_by_id(&pool, msg_id)
        .await
        .unwrap()
        .unwrap();
    assert!(msg.read_at.is_some(), "read_at must be set after mark_read");
}

// ── get_unread_count ──────────────────────────────────────────────────────

#[tokio::test]
async fn unread_count_is_zero_with_no_messages() {
    let pool = setup_test_db().await;
    setup_two_users_in_chat(&pool, 1, 2, 200).await;

    let count = messages::get_unread_count(&pool, 2).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn unread_count_increments_for_messages_from_other_user() {
    let pool = setup_test_db().await;
    setup_two_users_in_chat(&pool, 1, 2, 200).await;

    // User 1 sends 3 messages to user 2
    for _ in 0..3 {
        messages::send_message(
            &pool,
            NewMessage {
                sender_id: 1,
                chat_id: 200,
                content: b"unread".to_vec(),
                is_encrypted: false,
                message_type: "text".into(),
            },
        )
        .await
        .unwrap();
    }

    let count = messages::get_unread_count(&pool, 2).await.unwrap();
    assert_eq!(count, 3, "user 2 should see 3 unread messages");
}

#[tokio::test]
async fn own_messages_do_not_count_as_unread() {
    let pool = setup_test_db().await;
    setup_two_users_in_chat(&pool, 1, 2, 200).await;

    // User 2 sends their own messages — should not appear in their unread count
    for _ in 0..2 {
        messages::send_message(
            &pool,
            NewMessage {
                sender_id: 2,
                chat_id: 200,
                content: b"my own".to_vec(),
                is_encrypted: false,
                message_type: "text".into(),
            },
        )
        .await
        .unwrap();
    }

    let count = messages::get_unread_count(&pool, 2).await.unwrap();
    assert_eq!(count, 0, "own messages must not appear as unread");
}

#[tokio::test]
async fn unread_count_decrements_after_mark_read() {
    let pool = setup_test_db().await;
    setup_two_users_in_chat(&pool, 1, 2, 200).await;

    let msg_id = messages::send_message(
        &pool,
        NewMessage {
            sender_id: 1,
            chat_id: 200,
            content: b"please read".to_vec(),
            is_encrypted: false,
            message_type: "text".into(),
        },
    )
    .await
    .unwrap();

    let before = messages::get_unread_count(&pool, 2).await.unwrap();
    assert_eq!(before, 1);

    messages::mark_read(&pool, msg_id).await.unwrap();

    let after = messages::get_unread_count(&pool, 2).await.unwrap();
    assert_eq!(after, 0, "unread count must drop after mark_read");
}
