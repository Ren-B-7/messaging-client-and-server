use sqlx::sqlite::SqlitePool;
use std::path::PathBuf;
use tempfile::TempDir;

/// Database test context holding temporary database and path
pub struct DbTestContext {
    pub db: SqlitePool,
    pub temp_dir: TempDir,
    pub db_path: PathBuf,
}

impl DbTestContext {
    /// Create a new isolated test database
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        // Initialize database connection
        let db = SqlitePool::connect(&format!("sqlite:{}", db_path.to_string_lossy())).await?;

        Ok(Self {
            db,
            temp_dir,
            db_path,
        })
    }

    /// Run database initialization/schema setup
    pub async fn initialize_schema(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        server::database::create::create_tables(&self.db).await?;
        Ok(())
    }

    /// Close the database connection
    pub async fn close(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.db.close().await;
        Ok(())
    }
}

/// Helper to assert database state
pub struct DbAssertions;

impl DbAssertions {
    /// Verify a row count in a table
    pub async fn assert_row_count(
        db: &SqlitePool,
        table: &str,
        expected: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
            .fetch_one(db)
            .await?;

        assert_eq!(
            count.0 as u32, expected,
            "Table {} has {} rows, expected {}",
            table, count.0, expected
        );
        Ok(())
    }

    /// Verify a specific value exists in the database
    pub async fn assert_value_exists(
        db: &SqlitePool,
        query: &str,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let row: Option<(i64,)> = sqlx::query_as(query).fetch_optional(db).await?;

        assert!(row.is_some(), "Expected value not found: {}", value);
        Ok(())
    }
}

/// Test fixtures for common database objects
pub struct TestFixtures;

impl TestFixtures {
    pub const TEST_USER_ID: &'static str = "test_user_123";
    pub const TEST_USERNAME: &'static str = "testuser";
    pub const TEST_EMAIL: &'static str = "test@example.com";
    pub const TEST_GROUP_ID: &'static str = "test_group_456";
    pub const TEST_GROUP_NAME: &'static str = "Test Group";
    pub const TEST_MESSAGE: &'static str = "Test message content";
}
