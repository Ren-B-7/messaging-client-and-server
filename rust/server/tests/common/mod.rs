use std::path::PathBuf;
use tempfile::TempDir;
/// Common utilities shared across all database tests
use tokio_rusqlite::Connection;

/// Database test context holding temporary database and path
pub struct DbTestContext {
    pub db: Connection,
    pub temp_dir: TempDir,
    pub db_path: PathBuf,
}

impl DbTestContext {
    /// Create a new isolated test database
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        // Initialize database connection
        let db = Connection::open(db_path.clone()).await?;

        Ok(Self {
            db,
            temp_dir,
            db_path,
        })
    }

    /// Run database initialization/schema setup
    pub async fn initialize_schema(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // This should match your actual schema initialization
        // For now, it's a placeholder
        // You'll want to call your actual schema setup function here
        Ok(())
    }

    /// Close the database connection
    pub async fn close(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.db.close().await?;
        Ok(())
    }
}

/// Helper to assert database state
pub struct DbAssertions;

impl DbAssertions {
    /// Verify a row count in a table
    pub async fn assert_row_count(
        db: &Connection,
        table: &str,
        expected: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // We must clone the string to move it into the closure
        let table_name = table.to_string();

        let count: u32 = db
            .call(move |conn| {
                conn.query_row(&format!("SELECT COUNT(*) FROM {}", table_name), [], |row| {
                    row.get(0)
                })
            })
            .await?;

        assert_eq!(
            count, expected,
            "Table {} has {} rows, expected {}",
            table, count, expected
        );
        Ok(())
    }

    /// Verify a specific value exists in the database
    pub async fn assert_value_exists(
        db: &Connection,
        query: &str,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let query_str = query.to_string();

        let exists = db
            .call(move |conn| {
                let result = conn.query_row(&query_str, [], |_| Ok(true));

                match result {
                    Ok(_) => Ok(true),
                    Err(tokio_rusqlite::rusqlite::Error::QueryReturnedNoRows) => Ok(false),
                    Err(e) => Err(e),
                }
            })
            .await?;

        assert!(exists, "Expected value not found: {}", value);
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
