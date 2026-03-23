use crate::database::common::setup_test_db;
use server::database::{groups, messages};
use shared::types::groups::NewGroup;
use shared::types::message::NewMessage;
use tokio_rusqlite::{params, rusqlite::Error};

async fn insert_user(conn: &tokio_rusqlite::Connection, user_id: i64, username: &str) {
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
    .expect("insert_user failed");
}

// ── create_group ──────────────────────────────────────────────────────────

#[tokio::test]
async fn create_group_returns_valid_chat_id() {
    let conn = setup_test_db().await;
    let chat_id = groups::create_group(
        &conn,
        NewGroup {
            name: "Alpha".into(),
            created_by: 1,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();
    assert!(chat_id > 0);
}

#[tokio::test]
async fn creator_is_auto_added_as_admin_member() {
    let conn = setup_test_db().await;
    let chat_id = groups::create_group(
        &conn,
        NewGroup {
            name: "Dev Team".into(),
            created_by: 1,
            description: Some("Coding discussions".into()),
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();

    let members = groups::get_group_members(&conn, chat_id).await.unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].user_id, 1);
    assert_eq!(members[0].role, "admin");
}

// ── add_group_member / remove_group_member ────────────────────────────────

#[tokio::test]
async fn add_member_increases_member_count() {
    let conn = setup_test_db().await;
    let chat_id = groups::create_group(
        &conn,
        NewGroup {
            name: "G".into(),
            created_by: 1,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();

    groups::add_group_member(&conn, chat_id, 2, "member".into())
        .await
        .unwrap();

    let members = groups::get_group_members(&conn, chat_id).await.unwrap();
    assert_eq!(members.len(), 2);
}

#[tokio::test]
async fn remove_member_decreases_member_count() {
    let conn = setup_test_db().await;
    let chat_id = groups::create_group(
        &conn,
        NewGroup {
            name: "G".into(),
            created_by: 1,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();

    groups::add_group_member(&conn, chat_id, 2, "member".into())
        .await
        .unwrap();

    let removed = groups::remove_group_member(&conn, chat_id, 2)
        .await
        .unwrap();
    assert!(removed, "remove should return true for an existing member");

    let members = groups::get_group_members(&conn, chat_id).await.unwrap();
    assert_eq!(members.len(), 1, "only the creator should remain");
}

#[tokio::test]
async fn remove_nonexistent_member_returns_false() {
    let conn = setup_test_db().await;
    let chat_id = groups::create_group(
        &conn,
        NewGroup {
            name: "G".into(),
            created_by: 1,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();

    let removed = groups::remove_group_member(&conn, chat_id, 999)
        .await
        .unwrap();
    assert!(!removed);
}

// ── is_group_member / is_group_admin ──────────────────────────────────────

#[tokio::test]
async fn is_group_member_true_for_creator() {
    let conn = setup_test_db().await;
    let chat_id = groups::create_group(
        &conn,
        NewGroup {
            name: "G".into(),
            created_by: 7,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();

    assert!(groups::is_group_member(&conn, chat_id, 7).await.unwrap());
}

#[tokio::test]
async fn is_group_member_false_for_non_member() {
    let conn = setup_test_db().await;
    let chat_id = groups::create_group(
        &conn,
        NewGroup {
            name: "G".into(),
            created_by: 1,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();

    assert!(!groups::is_group_member(&conn, chat_id, 99).await.unwrap());
}

#[tokio::test]
async fn is_group_admin_true_for_creator() {
    let conn = setup_test_db().await;
    let chat_id = groups::create_group(
        &conn,
        NewGroup {
            name: "G".into(),
            created_by: 5,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();

    assert!(groups::is_group_admin(&conn, chat_id, 5).await.unwrap());
}

#[tokio::test]
async fn is_group_admin_false_for_regular_member() {
    let conn = setup_test_db().await;
    let chat_id = groups::create_group(
        &conn,
        NewGroup {
            name: "G".into(),
            created_by: 1,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();

    groups::add_group_member(&conn, chat_id, 2, "member".into())
        .await
        .unwrap();

    assert!(!groups::is_group_admin(&conn, chat_id, 2).await.unwrap());
}

// ── delete_group cascade ──────────────────────────────────────────────────

#[tokio::test]
async fn delete_group_removes_all_related_rows() {
    let conn = setup_test_db().await;

    // Create group with two members
    let chat_id = groups::create_group(
        &conn,
        NewGroup {
            name: "Doomed Group".into(),
            created_by: 1,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();
    groups::add_group_member(&conn, chat_id, 2, "member".into())
        .await
        .unwrap();

    // Send a message into the group
    messages::send_message(
        &conn,
        NewMessage {
            sender_id: 1,
            chat_id,
            content: b"before delete".to_vec(),
            is_encrypted: false,
            message_type: "text".into(),
        },
    )
    .await
    .unwrap();

    // Delete the group
    groups::delete_group(&conn, chat_id).await.unwrap();

    // Group must be gone
    let group = groups::get_group(&conn, chat_id).await.unwrap();
    assert!(group.is_none(), "group row must be deleted");

    // Members must be gone
    let members = groups::get_group_members(&conn, chat_id).await.unwrap();
    assert!(members.is_empty(), "group_members must be deleted");

    // Messages must be gone
    let history = messages::get_chat_messages(&conn, chat_id, 100, 0)
        .await
        .unwrap();
    assert!(history.is_empty(), "messages must be cascade-deleted");
}

// ── find_existing_dm ──────────────────────────────────────────────────────

#[tokio::test]
async fn find_existing_dm_returns_none_when_no_dm() {
    let conn = setup_test_db().await;
    let result = groups::find_existing_dm(&conn, 1, 2).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn find_existing_dm_finds_shared_direct_chat() {
    let conn = setup_test_db().await;

    // Create a direct-type group between users 1 and 2
    let chat_id = groups::create_group(
        &conn,
        NewGroup {
            name: "dm-1-2".into(),
            created_by: 1,
            description: None,
            chat_type: "direct".into(),
        },
    )
    .await
    .unwrap();
    groups::add_group_member(&conn, chat_id, 2, "member".into())
        .await
        .unwrap();

    let found = groups::find_existing_dm(&conn, 1, 2).await.unwrap();
    assert_eq!(found, Some(chat_id));

    // Symmetric lookup
    let found_rev = groups::find_existing_dm(&conn, 2, 1).await.unwrap();
    assert_eq!(found_rev, Some(chat_id));
}

// ── get_user_groups_by_activity ───────────────────────────────────────────

#[tokio::test]
async fn groups_ordered_by_most_recent_message() {
    let conn = setup_test_db().await;

    let chat1 = groups::create_group(
        &conn,
        NewGroup {
            name: "Older".into(),
            created_by: 1,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();

    let chat2 = groups::create_group(
        &conn,
        NewGroup {
            name: "Newer".into(),
            created_by: 1,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();

    // Send a message only to chat2 (making it more recently active)
    messages::send_message(
        &conn,
        NewMessage {
            sender_id: 1,
            chat_id: chat2,
            content: b"recent".to_vec(),
            is_encrypted: false,
            message_type: "text".into(),
        },
    )
    .await
    .unwrap();

    let groups_list = groups::get_user_groups_by_activity(&conn, 1).await.unwrap();

    assert_eq!(groups_list.len(), 2);
    assert_eq!(
        groups_list[0].id, chat2,
        "chat2 should be first (has a recent message)"
    );
}
