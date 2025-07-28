/// Database migrations for WhatsApp client

use crate::error::{Error, Result};
use super::schema::{SCHEMA_VERSION, CREATE_TABLES, CREATE_INDEXES, CREATE_TRIGGERS};
use sqlx::{SqlitePool, Row};

/// Run all database migrations
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    // Check current schema version
    let current_version = get_current_version(pool).await?;
    
    if current_version >= SCHEMA_VERSION {
        tracing::info!("Database schema is up to date (version {})", current_version);
        return Ok(());
    }
    
    tracing::info!("Running database migrations from version {} to {}", current_version, SCHEMA_VERSION);
    
    // Start transaction for all migrations
    let mut tx = pool.begin().await
        .map_err(|e| Error::Database(format!("Failed to begin migration transaction: {}", e)))?;
    
    // Run migrations based on current version
    match current_version {
        0 => {
            // Initial migration - create all tables
            migrate_to_v1(&mut tx).await?;
        }
        _ => {
            return Err(Error::Database(format!("Unknown schema version: {}", current_version)));
        }
    }
    
    // Update schema version
    sqlx::query("INSERT OR REPLACE INTO schema_version (version) VALUES (?)")
        .bind(SCHEMA_VERSION)
        .execute(&mut *tx)
        .await
        .map_err(|e| Error::Database(format!("Failed to update schema version: {}", e)))?;
    
    // Commit transaction
    tx.commit().await
        .map_err(|e| Error::Database(format!("Failed to commit migration transaction: {}", e)))?;
    
    tracing::info!("Database migrations completed successfully");
    Ok(())
}

/// Get current database schema version
async fn get_current_version(pool: &SqlitePool) -> Result<i32> {
    // Check if schema_version table exists
    let table_exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='schema_version'"
    )
    .fetch_one(pool)
    .await
    .map_err(|e| Error::Database(format!("Failed to check schema_version table: {}", e)))?;
    
    if !table_exists {
        return Ok(0);
    }
    
    // Get current version
    let version: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(version) FROM schema_version"
    )
    .fetch_one(pool)
    .await
    .map_err(|e| Error::Database(format!("Failed to get current schema version: {}", e)))?;
    
    Ok(version.unwrap_or(0))
}

/// Migration to version 1 - initial schema
async fn migrate_to_v1(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> Result<()> {
    tracing::info!("Running migration to version 1 (initial schema)");
    
    // Create all tables
    for sql in CREATE_TABLES {
        sqlx::query(sql)
            .execute(&mut **tx)
            .await
            .map_err(|e| Error::Database(format!("Failed to create table: {}", e)))?;
    }
    
    // Create indexes
    for sql in CREATE_INDEXES {
        sqlx::query(sql)
            .execute(&mut **tx)
            .await
            .map_err(|e| Error::Database(format!("Failed to create index: {}", e)))?;
    }
    
    // Create triggers
    for sql in CREATE_TRIGGERS {
        sqlx::query(sql)
            .execute(&mut **tx)
            .await
            .map_err(|e| Error::Database(format!("Failed to create trigger: {}", e)))?;
    }
    
    tracing::info!("Migration to version 1 completed");
    Ok(())
}

/// Migration helper functions for future versions
#[allow(dead_code)]
pub struct MigrationHelper;

impl MigrationHelper {
    /// Add a new column to a table
    pub async fn add_column(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        table: &str,
        column: &str,
        column_type: &str,
        default_value: Option<&str>,
    ) -> Result<()> {
        let mut sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, column_type);
        
        if let Some(default) = default_value {
            sql.push_str(&format!(" DEFAULT {}", default));
        }
        
        sqlx::query(&sql)
            .execute(&mut **tx)
            .await
            .map_err(|e| Error::Database(format!("Failed to add column: {}", e)))?;
        
        Ok(())
    }
    
    /// Create a new index
    pub async fn create_index(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        index_name: &str,
        table: &str,
        columns: &[&str],
        unique: bool,
    ) -> Result<()> {
        let unique_str = if unique { "UNIQUE " } else { "" };
        let columns_str = columns.join(", ");
        
        let sql = format!(
            "CREATE {}INDEX IF NOT EXISTS {} ON {}({})",
            unique_str, index_name, table, columns_str
        );
        
        sqlx::query(&sql)
            .execute(&mut **tx)
            .await
            .map_err(|e| Error::Database(format!("Failed to create index: {}", e)))?;
        
        Ok(())
    }
    
    /// Drop an index
    pub async fn drop_index(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        index_name: &str,
    ) -> Result<()> {
        let sql = format!("DROP INDEX IF EXISTS {}", index_name);
        
        sqlx::query(&sql)
            .execute(&mut **tx)
            .await
            .map_err(|e| Error::Database(format!("Failed to drop index: {}", e)))?;
        
        Ok(())
    }
    
    /// Rename a table
    pub async fn rename_table(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        old_name: &str,
        new_name: &str,
    ) -> Result<()> {
        let sql = format!("ALTER TABLE {} RENAME TO {}", old_name, new_name);
        
        sqlx::query(&sql)
            .execute(&mut **tx)
            .await
            .map_err(|e| Error::Database(format!("Failed to rename table: {}", e)))?;
        
        Ok(())
    }
    
    /// Execute raw SQL in a migration
    pub async fn execute_sql(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        sql: &str,
    ) -> Result<()> {
        sqlx::query(sql)
            .execute(&mut **tx)
            .await
            .map_err(|e| Error::Database(format!("Failed to execute SQL: {}", e)))?;
        
        Ok(())
    }
}

/// Validate database integrity
pub async fn validate_database(pool: &SqlitePool) -> Result<Vec<String>> {
    let mut issues = Vec::new();
    
    // Check foreign key integrity
    let fk_violations: Vec<String> = sqlx::query_scalar("PRAGMA foreign_key_check")
        .fetch_all(pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to check foreign keys: {}", e)))?;
    
    if !fk_violations.is_empty() {
        issues.extend(fk_violations.into_iter().map(|v| format!("Foreign key violation: {}", v)));
    }
    
    // Check database integrity
    let integrity_check: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to check integrity: {}", e)))?;
    
    if integrity_check != "ok" {
        issues.push(format!("Database integrity issue: {}", integrity_check));
    }
    
    // Check for orphaned records
    let orphaned_participants: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM group_participants WHERE group_jid NOT IN (SELECT jid FROM groups)"
    )
    .fetch_one(pool)
    .await
    .map_err(|e| Error::Database(format!("Failed to check orphaned participants: {}", e)))?;
    
    if orphaned_participants > 0 {
        issues.push(format!("Found {} orphaned group participants", orphaned_participants));
    }
    
    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use tempfile::tempdir;
    
    async fn create_test_db() -> Database {
        let config = crate::database::DatabaseConfig {
            database_url: "sqlite::memory:".to_string(),
            max_connections: 5,
            connection_timeout: 10,
            enable_wal: false,
        };
        
        Database::new(config).await.unwrap()
    }
    
    #[tokio::test]
    async fn test_migrations() {
        let db = create_test_db().await;
        
        // Migrations should have run during database creation
        let version = get_current_version(db.pool()).await.unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        
        // Check that all tables exist
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        
        let expected_tables = vec![
            "devices", "identity_keys", "sessions", "pre_keys", "signed_pre_keys",
            "group_sessions", "sender_keys", "groups", "group_participants",
            "contacts", "messages", "chats", "media_files", "settings", "schema_version"
        ];
        
        for expected_table in expected_tables {
            assert!(tables.contains(&expected_table.to_string()), 
                   "Table {} not found", expected_table);
        }
        
        db.close().await;
    }
    
    #[tokio::test]
    async fn test_database_validation() {
        let db = create_test_db().await;
        
        // Validate fresh database
        let issues = validate_database(db.pool()).await.unwrap();
        assert!(issues.is_empty(), "Fresh database should have no issues: {:?}", issues);
        
        db.close().await;
    }
    
    #[tokio::test]
    async fn test_migration_helper() {
        let db = create_test_db().await;
        let mut tx = db.pool().begin().await.unwrap();
        
        // Test adding a column
        MigrationHelper::add_column(
            &mut tx,
            "settings",
            "test_column",
            "TEXT",
            Some("'default_value'")
        ).await.unwrap();
        
        // Test creating an index
        MigrationHelper::create_index(
            &mut tx,
            "test_index",
            "settings",
            &["test_column"],
            false
        ).await.unwrap();
        
        tx.commit().await.unwrap();
        db.close().await;
    }
}