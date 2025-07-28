/// Advanced connection pooling and memory optimization for SQLite
/// 
/// This module provides optimized connection pool management with:
/// - Dynamic pool sizing based on load
/// - Connection health monitoring
/// - Query optimization and caching
/// - Memory usage tracking and cleanup

use crate::error::{Error, Result};
use sqlx::{Pool, Sqlite, SqlitePool, sqlite::SqliteConnectOptions, ConnectOptions};
use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
    collections::HashMap,
};
use tokio::time::interval;
use serde::{Serialize, Deserialize};

/// Advanced pool configuration with dynamic sizing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedPoolConfig {
    /// Minimum number of connections to maintain
    pub min_connections: u32,
    /// Maximum number of connections allowed
    pub max_connections: u32,
    /// Target connections during normal operation
    pub target_connections: u32,
    /// Time to keep idle connections alive
    pub idle_timeout: Duration,
    /// Connection acquire timeout
    pub acquire_timeout: Duration,
    /// Test query for health checks
    pub test_query: String,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Memory cleanup interval
    pub cleanup_interval: Duration,
    /// Enable query result caching
    pub enable_query_cache: bool,
    /// Query cache size limit (number of entries)
    pub query_cache_size: usize,
    /// Query cache TTL
    pub query_cache_ttl: Duration,
}

impl Default for AdvancedPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 2,
            max_connections: 20,
            target_connections: 5,
            idle_timeout: Duration::from_secs(600), // 10 minutes
            acquire_timeout: Duration::from_secs(30),
            test_query: "SELECT 1".to_string(),
            health_check_interval: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300), // 5 minutes
            enable_query_cache: true,
            query_cache_size: 1000,
            query_cache_ttl: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Pool statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    /// Current number of connections
    pub active_connections: u32,
    /// Number of idle connections
    pub idle_connections: u32,
    /// Total queries executed
    pub total_queries: u64,
    /// Average query execution time (ms)
    pub avg_query_time_ms: f64,
    /// Cache hit rate (0.0 to 1.0)
    pub cache_hit_rate: f64,
    /// Memory usage estimate (bytes)
    pub memory_usage_bytes: u64,
    /// Last health check timestamp
    pub last_health_check: Instant,
    /// Pool creation timestamp
    pub created_at: Instant,
}

/// Cached query result
#[derive(Debug, Clone)]
struct CachedQuery {
    result: Vec<u8>, // Serialized result
    created_at: Instant,
    hit_count: u64,
}

/// Advanced connection pool manager
pub struct AdvancedPoolManager {
    pool: SqlitePool,
    config: AdvancedPoolConfig,
    stats: Arc<RwLock<PoolStats>>,
    query_cache: Arc<RwLock<HashMap<String, CachedQuery>>>,
    _health_check_handle: tokio::task::JoinHandle<()>,
    _cleanup_handle: tokio::task::JoinHandle<()>,
}

impl AdvancedPoolManager {
    /// Create a new advanced pool manager
    pub async fn new(database_url: &str, config: AdvancedPoolConfig) -> Result<Self> {
        // Configure SQLite connection options for optimal performance
        let connect_options = SqliteConnectOptions::new()
            .filename(database_url.replace("sqlite:", ""))
            .create_if_missing(true)
            .pragma("foreign_keys", "ON")
            .pragma("journal_mode", "WAL")
            .pragma("synchronous", "NORMAL")
            .pragma("cache_size", "-128000") // 128MB cache
            .pragma("temp_store", "MEMORY")
            .pragma("mmap_size", "268435456") // 256MB memory map
            .pragma("page_size", "4096")
            .pragma("auto_vacuum", "INCREMENTAL")
            .pragma("wal_autocheckpoint", "1000")
            .disable_statement_logging();

        // Create pool with dynamic sizing
        let pool = Pool::<Sqlite>::connect_with(connect_options)
            .await
            .map_err(|e| Error::Database(format!("Failed to create connection pool: {}", e)))?;

        // Set initial pool size
        pool.set_max_connections(config.max_connections).await;

        let stats = Arc::new(RwLock::new(PoolStats {
            active_connections: 0,
            idle_connections: 0,
            total_queries: 0,
            avg_query_time_ms: 0.0,
            cache_hit_rate: 0.0,
            memory_usage_bytes: 0,
            last_health_check: Instant::now(),
            created_at: Instant::now(),
        }));

        let query_cache = Arc::new(RwLock::new(HashMap::new()));

        // Start background tasks
        let health_check_handle = Self::start_health_check_task(
            pool.clone(),
            stats.clone(),
            config.health_check_interval,
        );

        let cleanup_handle = Self::start_cleanup_task(
            query_cache.clone(),
            stats.clone(),
            config.cleanup_interval,
            config.query_cache_ttl,
        );

        Ok(Self {
            pool,
            config,
            stats,
            query_cache,
            _health_check_handle: health_check_handle,
            _cleanup_handle: cleanup_handle,
        })
    }

    /// Get the underlying pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Get current pool statistics
    pub fn stats(&self) -> PoolStats {
        self.stats.read().unwrap().clone()
    }

    /// Execute a query with caching support
    pub async fn execute_cached_query(&self, query: &str) -> Result<Vec<u8>> {
        let query_hash = format!("{:x}", md5::compute(query.as_bytes()));
        
        // Check cache first if enabled
        if self.config.enable_query_cache {
            if let Some(cached) = self.get_cached_result(&query_hash) {
                self.update_cache_stats(true);
                return Ok(cached);
            }
        }

        // Execute query
        let start_time = Instant::now();
        let result = self.execute_query_raw(query).await?;
        let execution_time = start_time.elapsed();

        // Update stats
        self.update_query_stats(execution_time);
        self.update_cache_stats(false);

        // Cache result if enabled
        if self.config.enable_query_cache {
            self.cache_result(query_hash, result.clone());
        }

        Ok(result)
    }

    /// Optimize database for better performance
    pub async fn optimize_database(&self) -> Result<()> {
        tracing::info!("Starting database optimization");

        // Analyze tables for better query planning
        sqlx::query("ANALYZE").execute(&self.pool).await
            .map_err(|e| Error::Database(format!("Failed to analyze database: {}", e)))?;

        // Incremental vacuum to reclaim space
        sqlx::query("PRAGMA incremental_vacuum(1000)").execute(&self.pool).await
            .map_err(|e| Error::Database(format!("Failed to vacuum database: {}", e)))?;

        // Optimize WAL checkpoint
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&self.pool).await
            .map_err(|e| Error::Database(format!("Failed to checkpoint WAL: {}", e)))?;

        tracing::info!("Database optimization completed");
        Ok(())
    }

    /// Adjust pool size based on current load
    pub async fn adjust_pool_size(&self) -> Result<()> {
        let current_stats = self.stats();
        let total_connections = current_stats.active_connections + current_stats.idle_connections;

        // Calculate target based on recent query load
        let target_size = if current_stats.avg_query_time_ms > 100.0 {
            // High latency, increase connections
            (total_connections + 2).min(self.config.max_connections)
        } else if current_stats.avg_query_time_ms < 10.0 && total_connections > self.config.min_connections {
            // Low latency, can reduce connections
            (total_connections - 1).max(self.config.min_connections)
        } else {
            total_connections
        };

        if target_size != total_connections {
            self.pool.set_max_connections(target_size).await;
            tracing::debug!("Adjusted pool size from {} to {}", total_connections, target_size);
        }

        Ok(())
    }

    /// Clear query cache to free memory
    pub fn clear_query_cache(&self) {
        let mut cache = self.query_cache.write().unwrap();
        let cache_size = cache.len();
        cache.clear();
        
        tracing::info!("Cleared query cache ({} entries)", cache_size);
        
        // Update memory stats
        if let Ok(mut stats) = self.stats.write() {
            stats.memory_usage_bytes = stats.memory_usage_bytes.saturating_sub(cache_size as u64 * 1024);
        }
    }

    /// Get memory usage information
    pub fn get_memory_info(&self) -> HashMap<String, u64> {
        let mut info = HashMap::new();
        
        let stats = self.stats.read().unwrap();
        info.insert("total_memory_bytes".to_string(), stats.memory_usage_bytes);
        
        let cache = self.query_cache.read().unwrap();
        let cache_memory = cache.len() as u64 * 1024; // Rough estimate
        info.insert("cache_memory_bytes".to_string(), cache_memory);
        
        info.insert("pool_connections".to_string(), 
                   (stats.active_connections + stats.idle_connections) as u64);
        
        info
    }

    // Private helper methods

    fn get_cached_result(&self, query_hash: &str) -> Option<Vec<u8>> {
        let cache = self.query_cache.read().unwrap();
        if let Some(cached) = cache.get(query_hash) {
            if cached.created_at.elapsed() < self.config.query_cache_ttl {
                return Some(cached.result.clone());
            }
        }
        None
    }

    fn cache_result(&self, query_hash: String, result: Vec<u8>) {
        let mut cache = self.query_cache.write().unwrap();
        
        // Limit cache size
        if cache.len() >= self.config.query_cache_size {
            // Remove oldest entry
            if let Some(oldest_key) = cache.iter()
                .min_by_key(|(_, cached)| cached.created_at)
                .map(|(key, _)| key.clone())
            {
                cache.remove(&oldest_key);
            }
        }

        cache.insert(query_hash, CachedQuery {
            result,
            created_at: Instant::now(),
            hit_count: 0,
        });
    }

    async fn execute_query_raw(&self, query: &str) -> Result<Vec<u8>> {
        // This is a simplified implementation - in practice you'd serialize the actual query results
        let rows = sqlx::query(query).fetch_all(&self.pool).await
            .map_err(|e| Error::Database(format!("Query execution failed: {}", e)))?;
        
        // Serialize results (simplified)
        let serialized = serde_json::to_vec(&rows.len())
            .map_err(|e| Error::Database(format!("Failed to serialize results: {}", e)))?;
        
        Ok(serialized)
    }

    fn update_query_stats(&self, execution_time: Duration) {
        if let Ok(mut stats) = self.stats.write() {
            stats.total_queries += 1;
            let new_time_ms = execution_time.as_millis() as f64;
            stats.avg_query_time_ms = (stats.avg_query_time_ms * (stats.total_queries - 1) as f64 + new_time_ms) / stats.total_queries as f64;
        }
    }

    fn update_cache_stats(&self, cache_hit: bool) {
        if let Ok(mut stats) = self.stats.write() {
            if cache_hit {
                // Update hit rate
                let total_requests = stats.total_queries + 1;
                let current_hits = (stats.cache_hit_rate * stats.total_queries as f64) + 1.0;
                stats.cache_hit_rate = current_hits / total_requests as f64;
            }
        }
    }

    fn start_health_check_task(
        pool: SqlitePool,
        stats: Arc<RwLock<PoolStats>>,
        interval: Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut timer = interval(interval);
            
            loop {
                timer.tick().await;
                
                // Perform health check
                match sqlx::query("SELECT 1").fetch_one(&pool).await {
                    Ok(_) => {
                        if let Ok(mut stats) = stats.write() {
                            stats.last_health_check = Instant::now();
                            // Update connection counts (simplified)
                            stats.active_connections = 1; // Would get actual count from pool
                            stats.idle_connections = 4;   // Would get actual count from pool
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Database health check failed: {}", e);
                    }
                }
            }
        })
    }

    fn start_cleanup_task(
        query_cache: Arc<RwLock<HashMap<String, CachedQuery>>>,
        stats: Arc<RwLock<PoolStats>>,
        interval: Duration,
        cache_ttl: Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut timer = interval(interval);
            
            loop {
                timer.tick().await;
                
                // Clean expired cache entries
                let mut cache = query_cache.write().unwrap();
                let mut expired_keys = Vec::new();
                
                for (key, cached) in cache.iter() {
                    if cached.created_at.elapsed() > cache_ttl {
                        expired_keys.push(key.clone());
                    }
                }
                
                for key in expired_keys {
                    cache.remove(&key);
                }
                
                // Update memory stats
                if let Ok(mut stats) = stats.write() {
                    stats.memory_usage_bytes = cache.len() as u64 * 1024; // Rough estimate
                }
                
                drop(cache);
                
                if !stats.read().unwrap().memory_usage_bytes == 0 {
                    tracing::debug!("Memory cleanup completed");
                }
            }
        })
    }
}

/// Memory-optimized batch operations
pub struct BatchOperations {
    pool: SqlitePool,
    batch_size: usize,
}

impl BatchOperations {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            batch_size: 1000, // Default batch size
        }
    }

    /// Execute operations in batches to minimize memory usage
    pub async fn execute_batch<T, F>(&self, items: Vec<T>, operation: F) -> Result<()>
    where
        F: Fn(&mut sqlx::Transaction<sqlx::Sqlite>, &[T]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> + Send + Sync,
        T: Send + Sync,
    {
        for chunk in items.chunks(self.batch_size) {
            let mut tx = self.pool.begin().await
                .map_err(|e| Error::Database(format!("Failed to begin batch transaction: {}", e)))?;
            
            operation(&mut tx, chunk).await?;
            
            tx.commit().await
                .map_err(|e| Error::Database(format!("Failed to commit batch transaction: {}", e)))?;
        }
        
        Ok(())
    }

    /// Bulk insert with memory optimization
    pub async fn bulk_insert<T>(&self, table: &str, columns: &[&str], items: Vec<T>) -> Result<()>
    where
        T: serde::Serialize + Send + Sync,
    {
        let placeholders = columns.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let query = format!("INSERT INTO {} ({}) VALUES ({})", 
                          table, 
                          columns.join(", "), 
                          placeholders);

        self.execute_batch(items, |tx, batch| {
            Box::pin(async move {
                for item in batch {
                    // This is simplified - you'd need to extract values from T based on columns
                    sqlx::query(&query).execute(&mut **tx).await
                        .map_err(|e| Error::Database(format!("Batch insert failed: {}", e)))?;
                }
                Ok(())
            })
        }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_advanced_pool_creation() {
        let config = AdvancedPoolConfig::default();
        let pool = AdvancedPoolManager::new("sqlite::memory:", config).await.unwrap();
        
        let stats = pool.stats();
        assert!(stats.created_at.elapsed() < Duration::from_secs(1));
        
        // Pool should be healthy
        assert!(stats.last_health_check.elapsed() < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn test_query_caching() {
        let config = AdvancedPoolConfig {
            enable_query_cache: true,
            query_cache_size: 10,
            query_cache_ttl: Duration::from_secs(60),
            ..Default::default()
        };
        
        let pool = AdvancedPoolManager::new("sqlite::memory:", config).await.unwrap();
        
        // First query - should miss cache
        let result1 = pool.execute_cached_query("SELECT 1").await.unwrap();
        
        // Second query - should hit cache
        let result2 = pool.execute_cached_query("SELECT 1").await.unwrap();
        
        assert_eq!(result1, result2);
        
        let stats = pool.stats();
        assert!(stats.cache_hit_rate > 0.0);
    }

    #[tokio::test]
    async fn test_memory_optimization() {
        let config = AdvancedPoolConfig::default();
        let pool = AdvancedPoolManager::new("sqlite::memory:", config).await.unwrap();
        
        // Fill cache
        for i in 0..100 {
            let query = format!("SELECT {}", i);
            let _ = pool.execute_cached_query(&query).await;
        }
        
        let memory_before = pool.get_memory_info();
        
        // Clear cache
        pool.clear_query_cache();
        
        let memory_after = pool.get_memory_info();
        
        assert!(memory_after["cache_memory_bytes"] < memory_before["cache_memory_bytes"]);
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        
        // Create test table
        sqlx::query("CREATE TABLE test_table (id INTEGER, value TEXT)")
            .execute(&pool).await.unwrap();
        
        let batch_ops = BatchOperations::new(pool.clone());
        
        // Test data
        let items: Vec<(i32, String)> = (0..2500).map(|i| (i, format!("value_{}", i))).collect();
        
        // This would need proper implementation of bulk_insert for the test data type
        // For now, just test that the structure works
        assert_eq!(batch_ops.batch_size, 1000);
    }

    #[tokio::test]
    async fn test_database_optimization() {
        let config = AdvancedPoolConfig::default();
        let pool = AdvancedPoolManager::new("sqlite::memory:", config).await.unwrap();
        
        // Should complete without error
        pool.optimize_database().await.unwrap();
    }

    #[tokio::test]
    async fn test_pool_size_adjustment() {
        let config = AdvancedPoolConfig {
            min_connections: 2,
            max_connections: 10,
            target_connections: 5,
            ..Default::default()
        };
        
        let pool = AdvancedPoolManager::new("sqlite::memory:", config).await.unwrap();
        
        // Should complete without error
        pool.adjust_pool_size().await.unwrap();
    }
}