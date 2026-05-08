use server::database::create;
use sqlx::sqlite::SqlitePool;

pub async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to open test DB");

    // Run the same table creation logic your real server uses
    create::create_tables(&pool)
        .await
        .expect("Failed to create tables");

    pool
}
