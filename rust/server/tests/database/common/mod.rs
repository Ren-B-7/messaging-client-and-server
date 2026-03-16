use server::database::create;
use tokio_rusqlite::Connection;

pub async fn setup_test_db() -> Connection {
    let conn = Connection::open(":memory:")
        .await
        .expect("Failed to open test DB");

    // Run the same table creation logic your real server uses
    create::create_tables(&conn)
        .await
        .expect("Failed to create tables");

    conn
}
