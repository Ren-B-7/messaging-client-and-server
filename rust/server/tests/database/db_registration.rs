// This file tests the database::register module (register_user, promote_user,
// demote_user, username_exists, email_exists, update_username).
// Tests that duplicate db_utils.rs (hashing, compression, validation) have
// been removed — those belong in db_utils.rs only.

use crate::database::common::setup_test_db;
use server::database::register;
use shared::types::user::NewUser;

// ── register_user ─────────────────────────────────────────────────────────

#[tokio::test]
async fn register_user_returns_valid_id() {
    let pool = setup_test_db().await;
    let id = register::register_user(
        &pool,
        NewUser {
            username: "alice".into(),
            password_hash: "hash_alice".into(),
            email: Some("alice@example.com".into()),
            name: None,
        },
    )
    .await
    .unwrap();
    assert!(id > 0);
}

#[tokio::test]
async fn first_registered_user_is_auto_promoted_to_admin() {
    let pool = setup_test_db().await;

    let id = register::register_user(
        &pool,
        NewUser {
            username: "first_user".into(),
            password_hash: "hash".into(),
            email: None,
            name: None,
        },
    )
    .await
    .unwrap();

    // The DB layer sets is_admin=1 for the first user
    let is_admin: (i64,) = sqlx::query_as("SELECT is_admin FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(is_admin.0, 1, "first user must be auto-promoted to admin");
}

#[tokio::test]
async fn second_registered_user_is_not_admin() {
    let pool = setup_test_db().await;

    register::register_user(
        &pool,
        NewUser {
            username: "first".into(),
            password_hash: "h".into(),
            email: None,
            name: None,
        },
    )
    .await
    .unwrap();

    let id2 = register::register_user(
        &pool,
        NewUser {
            username: "second".into(),
            password_hash: "h".into(),
            email: None,
            name: None,
        },
    )
    .await
    .unwrap();

    let is_admin: (i64,) = sqlx::query_as("SELECT is_admin FROM users WHERE id = ?")
        .bind(id2)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(is_admin.0, 0, "second user must NOT be admin");
}

#[tokio::test]
async fn register_user_with_email() {
    let pool = setup_test_db().await;
    let id = register::register_user(
        &pool,
        NewUser {
            username: "carol".into(),
            password_hash: "hash_carol".into(),
            email: Some("carol@example.com".into()),
            name: None,
        },
    )
    .await
    .unwrap();

    let email: (Option<String>,) = sqlx::query_as("SELECT email FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(email.0, Some("carol@example.com".to_string()));
}

#[tokio::test]
async fn register_user_without_email() {
    let pool = setup_test_db().await;
    let id = register::register_user(
        &pool,
        NewUser {
            username: "dave".into(),
            password_hash: "hash_dave".into(),
            email: None,
            name: None,
        },
    )
    .await
    .unwrap();

    let email: (Option<String>,) = sqlx::query_as("SELECT email FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert!(email.0.is_none());
}

// ── username_exists / email_exists ────────────────────────────────────────

#[tokio::test]
async fn username_exists_false_before_registration() {
    let pool = setup_test_db().await;
    let exists = register::username_exists(&pool, "ghost".into())
        .await
        .unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn username_exists_true_after_registration() {
    let pool = setup_test_db().await;
    register::register_user(
        &pool,
        NewUser {
            username: "existing".into(),
            password_hash: "h".into(),
            email: None,
            name: None,
        },
    )
    .await
    .unwrap();

    assert!(
        register::username_exists(&pool, "existing".into())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn email_exists_false_when_no_user_has_that_email() {
    let pool = setup_test_db().await;
    let exists = register::email_exists(&pool, "nobody@example.com".into())
        .await
        .unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn email_exists_true_after_registration() {
    let pool = setup_test_db().await;
    register::register_user(
        &pool,
        NewUser {
            username: "frank".into(),
            password_hash: "h".into(),
            email: Some("frank@example.com".into()),
            name: None,
        },
    )
    .await
    .unwrap();

    assert!(
        register::email_exists(&pool, "frank@example.com".into())
            .await
            .unwrap()
    );
}

// ── promote_user / demote_user ────────────────────────────────────────────

#[tokio::test]
async fn promote_user_sets_is_admin_flag() {
    let pool = setup_test_db().await;

    // Register two users so the second is not auto-admin
    register::register_user(
        &pool,
        NewUser {
            username: "root".into(),
            password_hash: "h".into(),
            email: None,
            name: None,
        },
    )
    .await
    .unwrap();

    let id = register::register_user(
        &pool,
        NewUser {
            username: "promoted".into(),
            password_hash: "h".into(),
            email: None,
            name: None,
        },
    )
    .await
    .unwrap();

    register::promote_user(&pool, id).await.unwrap();

    let is_admin: (i64,) = sqlx::query_as("SELECT is_admin FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(is_admin.0, 1);
}

#[tokio::test]
async fn demote_user_clears_is_admin_flag() {
    let pool = setup_test_db().await;

    let id = register::register_user(
        &pool,
        NewUser {
            username: "will_be_demoted".into(),
            password_hash: "h".into(),
            email: None,
            name: None,
        },
    )
    .await
    .unwrap();
    // First user is auto-admin; demote them.
    register::demote_user(&pool, id).await.unwrap();

    let is_admin: (i64,) = sqlx::query_as("SELECT is_admin FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(is_admin.0, 0);
}

// ── update_username ───────────────────────────────────────────────────────

#[tokio::test]
async fn update_username_persists_new_name() {
    let pool = setup_test_db().await;

    let id = register::register_user(
        &pool,
        NewUser {
            username: "old_name".into(),
            password_hash: "h".into(),
            email: None,
            name: None,
        },
    )
    .await
    .unwrap();

    register::update_username(&pool, id, "new_name".into())
        .await
        .unwrap();

    let username: (String,) = sqlx::query_as("SELECT username FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(username.0, "new_name");
}
