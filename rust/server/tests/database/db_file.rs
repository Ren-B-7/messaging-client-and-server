use crate::database::common::setup_test_db;
use server::database::files;

#[tokio::test]
async fn test_store_and_verify_file() {
    let pool = setup_test_db().await;

    // Must register users first to satisfy foreign key constraints
    use server::database::register;
    use shared::types::user::NewUser;
    let uploader_id = register::register_user(
        &pool,
        NewUser {
            username: "uploader".into(),
            password_hash: "h".into(),
            email: None,
            name: None,
        },
    )
    .await
    .unwrap();

    use server::database::groups;
    use shared::types::groups::NewGroup;
    let chat_id = groups::create_group(
        &pool,
        NewGroup {
            name: "test chat".into(),
            created_by: uploader_id,
            description: None,
            chat_type: "group".into(),
        },
    )
    .await
    .unwrap();

    let rec = files::NewFileRecord {
        uploader_id,
        chat_id,
        filename: "stats.csv".into(),
        mime_type: "text/csv".into(),
        size: 1024,
        storage_path: "uuid-path-123".into(),
        message_id: None,
    };

    let file_id = files::store_file_record(&pool, rec).await.unwrap();
    let belongs = files::file_belongs_to_chat(&pool, file_id, chat_id)
        .await
        .unwrap();

    assert!(belongs);
    let wrong_chat = files::file_belongs_to_chat(&pool, file_id, 999)
        .await
        .unwrap();
    assert!(!wrong_chat);
}
