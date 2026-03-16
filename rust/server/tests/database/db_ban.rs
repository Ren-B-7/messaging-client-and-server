use crate::database::common::setup_test_db;
use server::database::ban; // Assume a helper that creates schema

#[tokio::test]
async fn test_ban_user_and_check_status() {
    let conn = setup_test_db().await;
    // ... setup user ...
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
    ban::ban_user(&conn, 1, 99, None).await.unwrap();
    ban::unban_user(&conn, 1).await.unwrap();

    let info = ban::get_ban_info(&conn, 1).await.unwrap().unwrap();
    assert!(!info.is_banned);
}
