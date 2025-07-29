/// App State manager for coordinating all app state synchronization
/// 
/// This is the main orchestrator for WhatsApp app state synchronization,
/// coordinating contacts, chat metadata, settings, and protocol handling.

use crate::{
    appstate::{
        AppStateDataType, SyncContext, ContactSync, ChatMetadataSync, 
        SettingsSync, AppStateSyncProtocol, SyncStatistics
    },
    database::Database,
    error::{Error, Result},
    types::JID,
};
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::{
    sync::RwLock,
    time::interval,
};
use tracing::{debug, info, warn, error};

/// App State manager configuration
#[derive(Debug, Clone)]
pub struct AppStateManagerConfig {
    /// Enable automatic periodic sync
    pub enable_periodic_sync: bool,
    /// Sync interval in seconds
    pub sync_interval_seconds: u64,
    /// Enable background cleanup
    pub enable_background_cleanup: bool,
    /// Cleanup interval in seconds
    pub cleanup_interval_seconds: u64,
    /// Maximum concurrent sync sessions
    pub max_concurrent_syncs: u32,
    /// Sync timeout in seconds
    pub sync_timeout_seconds: u64,
}

/// App State manager
pub struct AppStateManager {
    /// Configuration
    config: AppStateManagerConfig,
    /// Database reference
    database: Arc<Database>,
    /// Sync context
    sync_context: Arc<SyncContext>,
    /// Contact synchronization handler
    contact_sync: Arc<ContactSync>,
    /// Chat metadata synchronization handler
    chat_metadata_sync: Arc<ChatMetadataSync>,
    /// Settings synchronization handler
    settings_sync: Arc<SettingsSync>,
    /// Protocol handler
    sync_protocol: Arc<AppStateSyncProtocol>,
    /// Manager state
    state: Arc<RwLock<AppStateManagerState>>,
}

/// App State manager internal state
#[derive(Debug, Clone)]
pub struct AppStateManagerState {
    /// Manager is running
    pub running: bool,
    /// Periodic sync task handle
    pub periodic_sync_handle: Option<tokio::task::JoinHandle<()>>,
    /// Cleanup task handle
    pub cleanup_handle: Option<tokio::task::JoinHandle<()>>,
    /// Active sync sessions count
    pub active_sync_count: u32,
    /// Last full sync time
    pub last_full_sync: Option<SystemTime>,
    /// Manager start time
    pub started_at: Option<SystemTime>,
}

/// App State sync request
#[derive(Debug, Clone)]
pub struct SyncRequest {
    /// Data types to sync
    pub data_types: Vec<AppStateDataType>,
    /// Force full sync (ignore incremental)
    pub force_full_sync: bool,
    /// Sync priority
    pub priority: SyncPriority,
    /// Timeout for this sync
    pub timeout: Option<Duration>,
}

/// Sync priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SyncPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// App State manager status
#[derive(Debug, Clone)]
pub struct AppStateManagerStatus {
    /// Manager is running
    pub running: bool,
    /// Active sync sessions
    pub active_syncs: u32,
    /// Last sync time per data type
    pub last_sync_times: std::collections::HashMap<AppStateDataType, SystemTime>,
    /// Sync statistics
    pub sync_statistics: SyncStatistics,
    /// Manager uptime
    pub uptime: Option<Duration>,
}

impl Default for AppStateManagerConfig {
    fn default() -> Self {
        Self {
            enable_periodic_sync: true,
            sync_interval_seconds: 3600, // 1 hour
            enable_background_cleanup: true,
            cleanup_interval_seconds: 300, // 5 minutes
            max_concurrent_syncs: 5,
            sync_timeout_seconds: 300, // 5 minutes
        }
    }
}

impl Default for AppStateManagerState {
    fn default() -> Self {
        Self {
            running: false,
            periodic_sync_handle: None,
            cleanup_handle: None,
            active_sync_count: 0,
            last_full_sync: None,
            started_at: None,
        }
    }
}

impl AppStateManager {
    /// Create new app state manager
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        Self::with_config(database, AppStateManagerConfig::default()).await
    }

    /// Create new app state manager with custom configuration
    pub async fn with_config(database: Arc<Database>, config: AppStateManagerConfig) -> Result<Self> {
        let sync_context = Arc::new(SyncContext::new(database.clone()));
        
        // Create sync handlers
        let contact_sync = Arc::new(ContactSync::new());
        let chat_metadata_sync = Arc::new(ChatMetadataSync::new());
        let settings_sync = Arc::new(SettingsSync::new());
        
        // Create protocol handler
        let sync_protocol = Arc::new(AppStateSyncProtocol::new(
            contact_sync.clone(),
            chat_metadata_sync.clone(),
            settings_sync.clone(),
        ));

        Ok(Self {
            config,
            database,
            sync_context,
            contact_sync,
            chat_metadata_sync,
            settings_sync,
            sync_protocol,
            state: Arc::new(RwLock::new(AppStateManagerState::default())),
        })
    }

    /// Start the app state manager
    pub async fn start(&self) -> Result<()> {
        let mut state = self.state.write().await;
        
        if state.running {
            return Err(Error::Protocol("App state manager is already running".to_string()));
        }

        info!("Starting App State Manager");
        
        state.running = true;
        state.started_at = Some(SystemTime::now());

        // Start periodic sync task if enabled
        if self.config.enable_periodic_sync {
            let periodic_handle = self.start_periodic_sync_task().await;
            state.periodic_sync_handle = Some(periodic_handle);
        }

        // Start cleanup task if enabled
        if self.config.enable_background_cleanup {
            let cleanup_handle = self.start_cleanup_task().await;
            state.cleanup_handle = Some(cleanup_handle);
        }

        info!("App State Manager started successfully");
        Ok(())
    }

    /// Stop the app state manager
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.state.write().await;
        
        if !state.running {
            return Ok(());
        }

        info!("Stopping App State Manager");
        
        state.running = false;

        // Cancel periodic sync task
        if let Some(handle) = state.periodic_sync_handle.take() {
            handle.abort();
        }

        // Cancel cleanup task
        if let Some(handle) = state.cleanup_handle.take() {
            handle.abort();
        }

        info!("App State Manager stopped");
        Ok(())
    }

    /// Request synchronization
    pub async fn request_sync(&self, request: SyncRequest) -> Result<Vec<String>> {
        let state = self.state.read().await;
        
        if !state.running {
            return Err(Error::Protocol("App state manager is not running".to_string()));
        }

        if state.active_sync_count >= self.config.max_concurrent_syncs {
            return Err(Error::Protocol("Maximum concurrent sync sessions reached".to_string()));
        }
        
        drop(state);

        debug!("Processing sync request for data types: {:?}", request.data_types);

        let mut session_ids = Vec::new();

        for data_type in request.data_types {
            // Start sync session
            let session_id = self.sync_protocol.start_sync(data_type.clone(), &self.sync_context).await?;
            session_ids.push(session_id);

            // Update active sync count
            {
                let mut state = self.state.write().await;
                state.active_sync_count += 1;
            }
        }

        info!("Started {} sync sessions", session_ids.len());
        Ok(session_ids)
    }

    /// Request full sync of all data types
    pub async fn request_full_sync(&self) -> Result<Vec<String>> {
        let request = SyncRequest {
            data_types: vec![
                AppStateDataType::Contacts,
                AppStateDataType::ChatMetadata,
                AppStateDataType::Settings,
            ],
            force_full_sync: true,
            priority: SyncPriority::High,
            timeout: Some(Duration::from_secs(self.config.sync_timeout_seconds)),
        };

        let session_ids = self.request_sync(request).await?;
        
        // Update last full sync time
        {
            let mut state = self.state.write().await;
            state.last_full_sync = Some(SystemTime::now());
        }

        Ok(session_ids)
    }

    /// Request sync for specific data type
    pub async fn request_sync_for_type(&self, data_type: AppStateDataType) -> Result<String> {
        let request = SyncRequest {
            data_types: vec![data_type],
            force_full_sync: false,
            priority: SyncPriority::Normal,
            timeout: None,
        };

        let session_ids = self.request_sync(request).await?;
        Ok(session_ids.into_iter().next().unwrap_or_default())
    }

    /// Get manager status
    pub async fn get_status(&self) -> AppStateManagerStatus {
        let state = self.state.read().await;
        
        // Get last sync times
        let mut last_sync_times = std::collections::HashMap::new();
        for data_type in &[AppStateDataType::Contacts, AppStateDataType::ChatMetadata, AppStateDataType::Settings] {
            if let Some(time) = self.sync_context.get_last_sync(data_type).await {
                last_sync_times.insert(data_type.clone(), time);
            }
        }

        // Calculate uptime
        let uptime = state.started_at.map(|start| {
            SystemTime::now().duration_since(start).unwrap_or_default()
        });

        // Get sync statistics
        let sync_statistics = self.sync_protocol.get_sync_statistics().await;

        AppStateManagerStatus {
            running: state.running,
            active_syncs: state.active_sync_count,
            last_sync_times,
            sync_statistics,
            uptime,
        }
    }

    /// Get contact sync handler
    pub fn contact_sync(&self) -> Arc<ContactSync> {
        self.contact_sync.clone()
    }

    /// Get chat metadata sync handler
    pub fn chat_metadata_sync(&self) -> Arc<ChatMetadataSync> {
        self.chat_metadata_sync.clone()
    }

    /// Get settings sync handler
    pub fn settings_sync(&self) -> Arc<SettingsSync> {
        self.settings_sync.clone()
    }

    /// Get sync protocol handler
    pub fn sync_protocol(&self) -> Arc<AppStateSyncProtocol> {
        self.sync_protocol.clone()
    }

    /// Start periodic sync background task
    async fn start_periodic_sync_task(&self) -> tokio::task::JoinHandle<()> {
        let manager = self.clone();
        let interval_duration = Duration::from_secs(self.config.sync_interval_seconds);

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);
            
            loop {
                interval.tick().await;
                
                let state = manager.state.read().await;
                if !state.running {
                    break;
                }
                drop(state);

                debug!("Running periodic app state sync");
                
                // Perform incremental sync for all data types
                let request = SyncRequest {
                    data_types: vec![
                        AppStateDataType::Contacts,
                        AppStateDataType::ChatMetadata,
                        AppStateDataType::Settings,
                    ],
                    force_full_sync: false,
                    priority: SyncPriority::Low,
                    timeout: None,
                };

                if let Err(e) = manager.request_sync(request).await {
                    warn!("Periodic sync failed: {}", e);
                } else {
                    debug!("Periodic sync completed successfully");
                }
            }
        })
    }

    /// Start cleanup background task
    async fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let protocol = self.sync_protocol.clone();
        let state = self.state.clone();
        let interval_duration = Duration::from_secs(self.config.cleanup_interval_seconds);

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);
            
            loop {
                interval.tick().await;
                
                let state_guard = state.read().await;
                if !state_guard.running {
                    break;
                }
                drop(state_guard);

                debug!("Running app state cleanup");
                
                // Clean up expired sync sessions
                match protocol.cleanup_expired_sessions().await {
                    Ok(cleaned_count) => {
                        if cleaned_count > 0 {
                            debug!("Cleaned up {} expired sync sessions", cleaned_count);
                        }
                    }
                    Err(e) => {
                        warn!("Cleanup failed: {}", e);
                    }
                }

                // Update active sync count based on actual sessions
                let stats = protocol.get_sync_statistics().await;
                {
                    let mut state_guard = state.write().await;
                    state_guard.active_sync_count = stats.active_sessions as u32;
                }
            }
        })
    }

    /// Check if manager needs full sync
    pub async fn needs_full_sync(&self) -> bool {
        let state = self.state.read().await;
        
        // Check if we've never done a full sync
        if state.last_full_sync.is_none() {
            return true;
        }

        // Check if it's been too long since last full sync
        if let Some(last_sync) = state.last_full_sync {
            let full_sync_interval = Duration::from_secs(86400); // 24 hours
            if SystemTime::now().duration_since(last_sync).unwrap_or_default() > full_sync_interval {
                return true;
            }
        }

        false
    }

    /// Force full sync if needed
    pub async fn sync_if_needed(&self) -> Result<Option<Vec<String>>> {
        if self.needs_full_sync().await {
            info!("Full sync needed, initiating...");
            let session_ids = self.request_full_sync().await?;
            Ok(Some(session_ids))
        } else {
            debug!("No full sync needed");
            Ok(None)
        }
    }
}

// Implement Clone for AppStateManager to enable use in background tasks
impl Clone for AppStateManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            database: self.database.clone(),
            sync_context: self.sync_context.clone(),
            contact_sync: self.contact_sync.clone(),
            chat_metadata_sync: self.chat_metadata_sync.clone(),
            settings_sync: self.settings_sync.clone(),
            sync_protocol: self.sync_protocol.clone(),
            state: self.state.clone(),
        }
    }
}

impl Default for SyncRequest {
    fn default() -> Self {
        Self {
            data_types: vec![AppStateDataType::Contacts],
            force_full_sync: false,
            priority: SyncPriority::Normal,
            timeout: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{Database, DatabaseConfig};

    #[tokio::test]
    async fn test_app_state_manager_creation() {
        let db = Arc::new(Database::new(DatabaseConfig::in_memory()).await.unwrap());
        let manager = AppStateManager::new(db).await.unwrap();
        
        let status = manager.get_status().await;
        assert!(!status.running);
        assert_eq!(status.active_syncs, 0);
    }

    #[tokio::test]
    async fn test_manager_lifecycle() {
        let db = Arc::new(Database::new(DatabaseConfig::in_memory()).await.unwrap());
        let manager = AppStateManager::new(db).await.unwrap();
        
        // Start manager
        manager.start().await.unwrap();
        
        let status = manager.get_status().await;
        assert!(status.running);
        
        // Stop manager
        manager.stop().await.unwrap();
        
        let status = manager.get_status().await;
        assert!(!status.running);
    }

    #[tokio::test]
    async fn test_sync_request() {
        let db = Arc::new(Database::new(DatabaseConfig::in_memory()).await.unwrap());
        let manager = AppStateManager::new(db).await.unwrap();
        
        // Start manager
        manager.start().await.unwrap();
        
        // Request sync
        let session_id = manager.request_sync_for_type(AppStateDataType::Contacts).await.unwrap();
        assert!(!session_id.is_empty());
        
        // Check status
        let status = manager.get_status().await;
        assert!(status.active_syncs > 0);
        
        // Stop manager
        manager.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_full_sync_detection() {
        let db = Arc::new(Database::new(DatabaseConfig::in_memory()).await.unwrap());
        let manager = AppStateManager::new(db).await.unwrap();
        
        // Should need full sync initially
        assert!(manager.needs_full_sync().await);
        
        // Start manager and perform full sync
        manager.start().await.unwrap();
        let _session_ids = manager.request_full_sync().await.unwrap();
        
        // Should no longer need full sync immediately after
        assert!(!manager.needs_full_sync().await);
        
        // Stop manager
        manager.stop().await.unwrap();
    }
}