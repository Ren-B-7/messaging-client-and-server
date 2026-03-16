use crate::database::common::setup_test_db;
use server::database::messages;
use shared::types::message::NewMessage;

#[tokio::test]
async fn test_send_and_retrieve_message() {
    let conn = setup_test_db().await;
    let msg = NewMessage {
        sender_id: 1,
        chat_id: 101,
        content: "Hello Rust!".into(),
        is_encrypted: false,
        message_type: "text".into(),
    };

    let msg_id = messages::send_message(&conn, msg).await.unwrap();
    let history = messages::get_chat_messages(&conn, 101, 10, 0)
        .await
        .unwrap();

    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, msg_id);
    assert_eq!(history[0].content, "Hello Rust!".as_bytes());
}

#[tokio::test]
async fn test_delete_message_permission() {
    let conn = setup_test_db().await;
    let msg_id = 1; // Assume exists from sender 1

    // Attempt delete by wrong user
    let deleted = messages::delete_message(&conn, msg_id, 999).await.unwrap();
    assert!(!deleted);

    // Attempt delete by owner
    let deleted = messages::delete_message(&conn, msg_id, 1).await.unwrap();
    assert!(deleted);
}
