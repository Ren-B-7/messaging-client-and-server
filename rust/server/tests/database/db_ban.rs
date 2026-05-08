use crate::database::common::setup_test_db;
use server::database::{ban, login};
use shared::types::login::NewSession;
use sqlx::sqlite::SqlitePool;

async fn insert_test_user(pool: &SqlitePool, user_id: i64, username: &str) {
    let username = username.to_string();

    // Ensure the banner user exists (id 99 is used in tests)
    sqlx::query(
        "INSERT OR IGNORE INTO users (id, username, password_hash, created_at, is_banned)
         VALUES (99, 'admin_banner', 'hash', 0, 0)",
    )
    .execute(pool)
    .await
    .expect("failed to insert banner user");

    sqlx::query(
        "INSERT INTO users (id, username, password_hash, created_at, is_banned)
         VALUES (?, ?, 'hash', 0, 0)",
    )
    .bind(user_id)
    .bind(username)
    .execute(pool)
    .await
    .expect("insert_test_user failed");
}

// ── ban_user ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn ban_sets_is_banned_flag() {
    let pool = setup_test_db().await;
    insert_test_user(&pool, 1, "alice").await;

    ban::ban_user(&pool, 1, 99, Some("Terms of Service violation".into()))
        .await
        .unwrap();

    let info = ban::get_ban_info(&pool, 1).await.unwrap().unwrap();
    assert!(info.is_banned);
    assert_eq!(info.ban_reason, Some("Terms of Service violation".into()));
}

#[tokio::test]
async fn ban_without_reason_records_none() {
    let pool = setup_test_db().await;
    insert_test_user(&pool, 2, "bob").await;

    ban::ban_user(&pool, 2, 99, None).await.unwrap();

    let info = ban::get_ban_info(&pool, 2).await.unwrap().unwrap();
    assert!(info.is_banned);
    assert!(info.ban_reason.is_none());
}

#[tokio::test]
async fn ban_deletes_all_active_sessions() {
    let pool = setup_test_db().await;
    insert_test_user(&pool, 3, "charlie").await;

    // Create a session for the user
    login::create_session(
        &pool,
        NewSession {
            user_id: 3,
            session_id: "session-abc-123".to_string(),
            expires_at: i64::MAX,
            ip_address: None,
        },
    )
    .await
    .unwrap();

    // Verify the session exists before banning
    let before = login::validate_session_id(&pool, "session-abc-123".to_string())
        .await
        .unwrap();
    assert!(before.is_some(), "session must exist before ban");

    // Ban the user
    ban::ban_user(&pool, 3, 99, Some("test ban".into()))
        .await
        .unwrap();

    // Session must be gone after ban
    let after = login::validate_session_id(&pool, "session-abc-123".to_string())
        .await
        .unwrap();
    assert!(after.is_none(), "ban must delete all active sessions");
}

// ── unban_user ────────────────────────────────────────────────────────────

#[tokio::test]
async fn unban_clears_is_banned_flag() {
    let pool = setup_test_db().await;
    insert_test_user(&pool, 4, "dave").await;

    ban::ban_user(&pool, 4, 99, None).await.unwrap();
    ban::unban_user(&pool, 4).await.unwrap();

    let info = ban::get_ban_info(&pool, 4).await.unwrap().unwrap();
    assert!(!info.is_banned);
}

#[tokio::test]
async fn unban_clears_ban_reason() {
    let pool = setup_test_db().await;
    insert_test_user(&pool, 5, "eve").await;

    ban::ban_user(&pool, 5, 99, Some("reason".into()))
        .await
        .unwrap();
    ban::unban_user(&pool, 5).await.unwrap();

    let info = ban::get_ban_info(&pool, 5).await.unwrap().unwrap();
    assert!(info.ban_reason.is_none(), "unban must clear ban_reason");
}

// ── get_ban_info ──────────────────────────────────────────────────────────

#[tokio::test]
async fn get_ban_info_returns_none_for_unknown_user() {
    let pool = setup_test_db().await;
    let info = ban::get_ban_info(&pool, 9999).await.unwrap();
    assert!(info.is_none());
}

#[tokio::test]
async fn get_ban_info_returns_correct_username() {
    let pool = setup_test_db().await;
    insert_test_user(&pool, 6, "frank").await;

    let info = ban::get_ban_info(&pool, 6).await.unwrap().unwrap();
    assert_eq!(info.username, "frank");
    assert!(!info.is_banned);
}

// ── is_user_banned ────────────────────────────────────────────────────────

#[tokio::test]
async fn is_user_banned_false_before_ban() {
    let pool = setup_test_db().await;
    insert_test_user(&pool, 7, "grace").await;

    let banned = ban::is_user_banned(&pool, 7).await.unwrap();
    assert!(!banned);
}

#[tokio::test]
async fn is_user_banned_true_after_ban() {
    let pool = setup_test_db().await;
    insert_test_user(&pool, 8, "henry").await;

    ban::ban_user(&pool, 8, 99, None).await.unwrap();

    let banned = ban::is_user_banned(&pool, 8).await.unwrap();
    assert!(banned);
}

// ── get_banned_users ──────────────────────────────────────────────────────

#[tokio::test]
async fn get_banned_users_empty_initially() {
    let pool = setup_test_db().await;
    let list = ban::get_banned_users(&pool).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn get_banned_users_lists_all_banned() {
    let pool = setup_test_db().await;
    insert_test_user(&pool, 10, "user_a").await;
    insert_test_user(&pool, 11, "user_b").await;
    insert_test_user(&pool, 12, "user_c").await;

    ban::ban_user(&pool, 10, 99, None).await.unwrap();
    ban::ban_user(&pool, 11, 99, None).await.unwrap();
    // user 12 is not banned

    let list = ban::get_banned_users(&pool).await.unwrap();
    assert_eq!(list.len(), 2);
    let ids: Vec<i64> = list.iter().map(|b| b.user_id).collect();
    assert!(ids.contains(&10));
    assert!(ids.contains(&11));
    assert!(!ids.contains(&12));
}
