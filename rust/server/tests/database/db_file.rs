use crate::database::common::setup_test_db;
use server::database::files;

#[tokio::test]
async fn test_store_and_verify_file() {
    let conn = setup_test_db().await;
    let rec = files::NewFileRecord {
        uploader_id: 1,
        chat_id: 5,
        filename: "stats.csv".into(),
        mime_type: "text/csv".into(),
        size: 1024,
        storage_path: "uuid-path-123".into(),
        message_id: None,
    };

    let file_id = files::store_file_record(&conn, rec).await.unwrap();
    let belongs = files::file_belongs_to_chat(&conn, file_id, 5)
        .await
        .unwrap();

    assert!(belongs);
    let wrong_chat = files::file_belongs_to_chat(&conn, file_id, 99)
        .await
        .unwrap();
    assert!(!wrong_chat);
}
