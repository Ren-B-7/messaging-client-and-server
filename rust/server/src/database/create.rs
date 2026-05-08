use sqlx::sqlite::SqlitePool;
use tracing::info;

/// Open or create the database and ensure the schema is up to date.
pub async fn open_database(path: &str) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePool::connect(&format!("sqlite:{}", path)).await?;

    // Run the migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    info!("Database connected and migrations applied");

    Ok(pool)
}

/// Initialize the database schema and run any pending migrations.
/// (Maintained for API compatibility if needed, but logic moved to migrations)
pub async fn create_tables(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
