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
    let conn = setup_test_db().await;
    let id = register::register_user(
        &conn,
        NewUser {
            username: "alice".into(),
            password_hash: "hash_alice".into(),
            email: Some("alice@example.com".into()),
        },
    )
    .await
    .unwrap();
    assert!(id > 0);
}

#[tokio::test]
async fn first_registered_user_is_auto_promoted_to_admin() {
    let conn = setup_test_db().await;

    let id = register::register_user(
        &conn,
        NewUser {
            username: "first_user".into(),
            password_hash: "hash".into(),
            email: None,
        },
    )
    .await
    .unwrap();

    // The DB layer sets is_admin=1 for the first user
    let is_admin: i64 = conn
        .call(move |c| {
            c.query_row("SELECT is_admin FROM users WHERE id = ?1", [id], |r| {
                r.get(0)
            })
        })
        .await
        .unwrap();

    assert_eq!(is_admin, 1, "first user must be auto-promoted to admin");
}

#[tokio::test]
async fn second_registered_user_is_not_admin() {
    let conn = setup_test_db().await;

    register::register_user(
        &conn,
        NewUser {
            username: "first".into(),
            password_hash: "h".into(),
            email: None,
        },
    )
    .await
    .unwrap();

    let id2 = register::register_user(
        &conn,
        NewUser {
            username: "second".into(),
            password_hash: "h".into(),
            email: None,
        },
    )
    .await
    .unwrap();

    let is_admin: i64 = conn
        .call(move |c| {
            c.query_row("SELECT is_admin FROM users WHERE id = ?1", [id2], |r| {
                r.get(0)
            })
        })
        .await
        .unwrap();

    assert_eq!(is_admin, 0, "second user must NOT be admin");
}

#[tokio::test]
async fn register_user_with_email() {
    let conn = setup_test_db().await;
    let id = register::register_user(
        &conn,
        NewUser {
            username: "carol".into(),
            password_hash: "hash_carol".into(),
            email: Some("carol@example.com".into()),
        },
    )
    .await
    .unwrap();

    let email: Option<String> = conn
        .call(move |c| c.query_row("SELECT email FROM users WHERE id = ?1", [id], |r| r.get(0)))
        .await
        .unwrap();

    assert_eq!(email, Some("carol@example.com".to_string()));
}

#[tokio::test]
async fn register_user_without_email() {
    let conn = setup_test_db().await;
    let id = register::register_user(
        &conn,
        NewUser {
            username: "dave".into(),
            password_hash: "hash_dave".into(),
            email: None,
        },
    )
    .await
    .unwrap();

    let email: Option<String> = conn
        .call(move |c| c.query_row("SELECT email FROM users WHERE id = ?1", [id], |r| r.get(0)))
        .await
        .unwrap();

    assert!(email.is_none());
}

// ── username_exists / email_exists ────────────────────────────────────────

#[tokio::test]
async fn username_exists_false_before_registration() {
    let conn = setup_test_db().await;
    let exists = register::username_exists(&conn, "ghost".into())
        .await
        .unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn username_exists_true_after_registration() {
    let conn = setup_test_db().await;
    register::register_user(
        &conn,
        NewUser {
            username: "existing".into(),
            password_hash: "h".into(),
            email: None,
        },
    )
    .await
    .unwrap();

    assert!(
        register::username_exists(&conn, "existing".into())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn email_exists_false_when_no_user_has_that_email() {
    let conn = setup_test_db().await;
    let exists = register::email_exists(&conn, "nobody@example.com".into())
        .await
        .unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn email_exists_true_after_registration() {
    let conn = setup_test_db().await;
    register::register_user(
        &conn,
        NewUser {
            username: "frank".into(),
            password_hash: "h".into(),
            email: Some("frank@example.com".into()),
        },
    )
    .await
    .unwrap();

    assert!(
        register::email_exists(&conn, "frank@example.com".into())
            .await
            .unwrap()
    );
}

// ── promote_user / demote_user ────────────────────────────────────────────

#[tokio::test]
async fn promote_user_sets_is_admin_flag() {
    let conn = setup_test_db().await;

    // Register two users so the second is not auto-admin
    register::register_user(
        &conn,
        NewUser {
            username: "root".into(),
            password_hash: "h".into(),
            email: None,
        },
    )
    .await
    .unwrap();

    let id = register::register_user(
        &conn,
        NewUser {
            username: "promoted".into(),
            password_hash: "h".into(),
            email: None,
        },
    )
    .await
    .unwrap();

    register::promote_user(&conn, id).await.unwrap();

    let is_admin: i64 = conn
        .call(move |c| {
            c.query_row("SELECT is_admin FROM users WHERE id = ?1", [id], |r| {
                r.get(0)
            })
        })
        .await
        .unwrap();

    assert_eq!(is_admin, 1);
}

#[tokio::test]
async fn demote_user_clears_is_admin_flag() {
    let conn = setup_test_db().await;

    let id = register::register_user(
        &conn,
        NewUser {
            username: "will_be_demoted".into(),
            password_hash: "h".into(),
            email: None,
        },
    )
    .await
    .unwrap();
    // First user is auto-admin; demote them.
    register::demote_user(&conn, id).await.unwrap();

    let is_admin: i64 = conn
        .call(move |c| {
            c.query_row("SELECT is_admin FROM users WHERE id = ?1", [id], |r| {
                r.get(0)
            })
        })
        .await
        .unwrap();

    assert_eq!(is_admin, 0);
}

// ── update_username ───────────────────────────────────────────────────────

#[tokio::test]
async fn update_username_persists_new_name() {
    let conn = setup_test_db().await;

    let id = register::register_user(
        &conn,
        NewUser {
            username: "old_name".into(),
            password_hash: "h".into(),
            email: None,
        },
    )
    .await
    .unwrap();

    register::update_username(&conn, id, "new_name".into())
        .await
        .unwrap();

    let username: String = conn
        .call(move |c| {
            c.query_row("SELECT username FROM users WHERE id = ?1", [id], |r| {
                r.get(0)
            })
        })
        .await
        .unwrap();

    assert_eq!(username, "new_name");
}
