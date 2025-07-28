/// SQLite database backend for persistent storage

pub mod schema;
pub mod sqlite;
pub mod migrations;
pub mod pool;

use crate::error::{Error, Result};
use sqlx::{Pool, Sqlite, Row};

/// Database connection pool type
pub type DatabasePool = Pool<Sqlite>;

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Database file path
    pub database_url: String,
    /// Maximum number of connections in pool
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    /// Enable WAL mode for better concurrency
    pub enable_wal: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: "sqlite:whatsmeow.db".to_string(),
            max_connections: 10,
            connection_timeout: 30,
            enable_wal: true,
        }
    }
}

/// Main database manager
pub struct Database {
    pool: DatabasePool,
    config: DatabaseConfig,
}

impl Database {
    /// Create new database instance
    pub async fn new(config: DatabaseConfig) -> Result<Self> {
        let pool = if config.database_url == "sqlite::memory:" {
            // Handle in-memory database
            sqlx::SqlitePool::connect(&config.database_url)
                .await
                .map_err(|e| Error::Database(format!("Failed to connect to database: {}", e)))?
        } else {
            // Handle file-based database
            sqlx::SqlitePool::connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&config.database_url.replace("sqlite:", ""))
                    .create_if_missing(true)
                    .pragma("foreign_keys", "ON")
                    .pragma("journal_mode", if config.enable_wal { "WAL" } else { "DELETE" })
                    .pragma("synchronous", "NORMAL")
                    .pragma("cache_size", "-64000") // 64MB cache
                    .pragma("temp_store", "MEMORY")
            )
            .await
            .map_err(|e| Error::Database(format!("Failed to connect to database: {}", e)))?
        };

        let database = Self { pool, config };
        
        // Run migrations
        database.migrate().await?;
        
        Ok(database)
    }
    
    /// Run database migrations
    async fn migrate(&self) -> Result<()> {
        migrations::run_migrations(&self.pool).await
    }
    
    /// Get database pool
    pub fn pool(&self) -> &DatabasePool {
        &self.pool
    }
    
    /// Close database connection
    pub async fn close(&self) {
        self.pool.close().await;
    }
    
    /// Optimize database (run VACUUM and ANALYZE)
    pub async fn optimize(&self) -> Result<()> {
        sqlx::query("VACUUM").execute(&self.pool).await
            .map_err(|e| Error::Database(format!("VACUUM failed: {}", e)))?;
        
        sqlx::query("ANALYZE").execute(&self.pool).await
            .map_err(|e| Error::Database(format!("ANALYZE failed: {}", e)))?;
        
        Ok(())
    }
    
    /// Get database statistics
    pub async fn get_stats(&self) -> Result<DatabaseStats> {
        let row = sqlx::query("SELECT page_count, page_size FROM pragma_page_count(), pragma_page_size()")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to get stats: {}", e)))?;
        
        let page_count: i64 = row.get(0);
        let page_size: i64 = row.get(1);
        
        Ok(DatabaseStats {
            total_size: page_count * page_size,
            page_count,
            page_size,
        })
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    /// Total database size in bytes
    pub total_size: i64,
    /// Number of pages
    pub page_count: i64,
    /// Page size in bytes
    pub page_size: i64,
}

/// Database transaction helper
pub struct Transaction<'a> {
    tx: sqlx::Transaction<'a, Sqlite>,
}

impl<'a> Transaction<'a> {
    /// Create new transaction
    pub async fn begin(pool: &DatabasePool) -> Result<Transaction<'_>> {
        let tx = pool.begin().await
            .map_err(|e| Error::Database(format!("Failed to begin transaction: {}", e)))?;
        
        Ok(Transaction { tx })
    }
    
    /// Commit transaction
    pub async fn commit(self) -> Result<()> {
        self.tx.commit().await
            .map_err(|e| Error::Database(format!("Failed to commit transaction: {}", e)))
    }
    
    /// Rollback transaction
    pub async fn rollback(self) -> Result<()> {
        self.tx.rollback().await
            .map_err(|e| Error::Database(format!("Failed to rollback transaction: {}", e)))
    }
    
    /// Get inner transaction
    pub fn inner(&mut self) -> &mut sqlx::Transaction<'a, Sqlite> {
        &mut self.tx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    async fn create_test_database() -> Database {
        let config = DatabaseConfig {
            database_url: "sqlite::memory:".to_string(),
            max_connections: 5,
            connection_timeout: 10,
            enable_wal: false, // Disable WAL for tests
        };
        
        Database::new(config).await.unwrap()
    }
    
    #[tokio::test]
    async fn test_database_creation() {
        let db = create_test_database().await;
        
        // Test connection
        let stats = db.get_stats().await.unwrap();
        assert!(stats.page_count > 0);
        assert!(stats.page_size > 0);
        
        db.close().await;
    }
    
    #[tokio::test]
    async fn test_transaction() {
        let db = create_test_database().await;
        
        // Test transaction creation and commit
        let tx = Transaction::begin(&db.pool).await.unwrap();
        tx.commit().await.unwrap();
        
        // Test transaction rollback
        let tx = Transaction::begin(&db.pool).await.unwrap();
        tx.rollback().await.unwrap();
        
        db.close().await;
    }
    
    #[tokio::test]
    async fn test_database_optimization() {
        let db = create_test_database().await;
        
        // Test optimization
        db.optimize().await.unwrap();
        
        db.close().await;
    }
}