use crate::database::common::setup_test_db;
use server::database::groups;
use shared::types::groups::NewGroup;

#[tokio::test]
async fn test_group_creation_and_membership() {
    let conn = setup_test_db().await;
    let new_group = NewGroup {
        name: "Dev Team".into(),
        created_by: 1,
        description: Some("Coding discussions".to_string()),
        chat_type: "group".into(),
    };

    let chat_id = groups::create_group(&conn, new_group).await.unwrap();
    let members = groups::get_group_members(&conn, chat_id).await.unwrap();

    // Creator should be admin automatically
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].role, "admin");
}
