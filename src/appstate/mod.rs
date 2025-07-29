/// WhatsApp App State Synchronization System
/// 
/// This module handles synchronization of app state between devices including:
/// - Contact synchronization
/// - Chat metadata (muted, archived, pinned status)
/// - User preferences and settings
/// - Profile information
/// - History sync handling

pub mod contacts;
pub mod chat_metadata;
pub mod settings;
pub mod sync_protocol;
pub mod state_manager;

use crate::{
    error::{Error, Result},
    types::JID,
    database::Database,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::SystemTime,
};
use tokio::sync::RwLock;

pub use contacts::*;
pub use chat_metadata::*;
pub use settings::*;
pub use sync_protocol::*;
pub use state_manager::*;

/// App State data types that can be synchronized
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AppStateDataType {
    /// Contact information and metadata
    Contacts,
    /// Chat metadata (muted, archived, pinned)
    ChatMetadata,
    /// User settings and preferences
    Settings,
    /// User profile information
    Profile,
    /// Chat history data
    History,
    /// Block list
    BlockList,
    /// Privacy settings
    Privacy,
    /// Group settings
    GroupSettings,
    /// Status privacy
    StatusPrivacy,
    /// Unknown state type
    Unknown(String),
}

/// App State synchronization event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateEvent {
    /// Type of state being synchronized
    pub data_type: AppStateDataType,
    /// Operation performed (update, delete, etc.)
    pub operation: AppStateOperation,
    /// Timestamp of the event
    pub timestamp: SystemTime,
    /// Key identifier for the state item
    pub key: String,
    /// Optional data payload
    pub data: Option<Vec<u8>>,
}

/// Operations that can be performed on app state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AppStateOperation {
    /// Update or create state
    Update,
    /// Delete state
    Delete,
    /// Bulk update
    BulkUpdate,
    /// Full sync
    FullSync,
    /// Incremental sync
    IncrementalSync,
}

/// App State sync status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncStatus {
    /// Not synchronized
    NotSynced,
    /// Sync in progress
    Syncing,
    /// Successfully synchronized
    Synced,
    /// Sync failed
    Failed { error: String },
    /// Conflict detected
    Conflict,
}

/// App State key for identifying state items
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AppStateKey {
    /// Data type
    pub data_type: AppStateDataType,
    /// Unique identifier within the data type
    pub identifier: String,
}

impl AppStateKey {
    /// Create a new app state key
    pub fn new(data_type: AppStateDataType, identifier: String) -> Self {
        Self {
            data_type,
            identifier,
        }
    }

    /// Create a contact key
    pub fn contact(jid: &JID) -> Self {
        Self::new(AppStateDataType::Contacts, jid.to_string())
    }

    /// Create a chat metadata key
    pub fn chat_metadata(jid: &JID) -> Self {
        Self::new(AppStateDataType::ChatMetadata, jid.to_string())
    }

    /// Create a settings key
    pub fn settings(setting_name: &str) -> Self {
        Self::new(AppStateDataType::Settings, setting_name.to_string())
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        format!("{}:{}", self.data_type_string(), self.identifier)
    }

    /// Get data type as string
    pub fn data_type_string(&self) -> String {
        match &self.data_type {
            AppStateDataType::Contacts => "contacts".to_string(),
            AppStateDataType::ChatMetadata => "chat_metadata".to_string(),
            AppStateDataType::Settings => "settings".to_string(),
            AppStateDataType::Profile => "profile".to_string(),
            AppStateDataType::History => "history".to_string(),
            AppStateDataType::BlockList => "block_list".to_string(),
            AppStateDataType::Privacy => "privacy".to_string(),
            AppStateDataType::GroupSettings => "group_settings".to_string(),
            AppStateDataType::StatusPrivacy => "status_privacy".to_string(),
            AppStateDataType::Unknown(name) => name.clone(),
        }
    }
}

/// App State version for conflict resolution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppStateVersion {
    /// Timestamp of the version
    pub timestamp: SystemTime,
    /// Version hash for integrity
    pub hash: String,
    /// Device that created this version
    pub device_id: String,
}

/// App State sync context
#[derive(Debug, Clone)]
pub struct SyncContext {
    /// Database connection
    pub database: Arc<Database>,
    /// Current sync status
    pub sync_status: Arc<RwLock<HashMap<AppStateKey, SyncStatus>>>,
    /// Last sync timestamps
    pub last_sync: Arc<RwLock<HashMap<AppStateDataType, SystemTime>>>,
    /// Sync conflicts
    pub conflicts: Arc<RwLock<Vec<SyncConflict>>>,
}

/// Sync conflict information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflict {
    /// Key that has conflict
    pub key: AppStateKey,
    /// Local version
    pub local_version: AppStateVersion,
    /// Remote version
    pub remote_version: AppStateVersion,
    /// Conflict data
    pub local_data: Option<Vec<u8>>,
    pub remote_data: Option<Vec<u8>>,
    /// When conflict was detected
    pub detected_at: SystemTime,
}

impl SyncContext {
    /// Create a new sync context
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            sync_status: Arc::new(RwLock::new(HashMap::new())),
            last_sync: Arc::new(RwLock::new(HashMap::new())),
            conflicts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Update sync status for a key
    pub async fn update_sync_status(&self, key: AppStateKey, status: SyncStatus) {
        let mut sync_status = self.sync_status.write().await;
        sync_status.insert(key, status);
    }

    /// Get sync status for a key
    pub async fn get_sync_status(&self, key: &AppStateKey) -> SyncStatus {
        let sync_status = self.sync_status.read().await;
        sync_status.get(key).cloned().unwrap_or(SyncStatus::NotSynced)
    }

    /// Update last sync time for a data type
    pub async fn update_last_sync(&self, data_type: AppStateDataType) {
        let mut last_sync = self.last_sync.write().await;
        last_sync.insert(data_type, SystemTime::now());
    }

    /// Get last sync time for a data type
    pub async fn get_last_sync(&self, data_type: &AppStateDataType) -> Option<SystemTime> {
        let last_sync = self.last_sync.read().await;
        last_sync.get(data_type).cloned()
    }

    /// Add a sync conflict
    pub async fn add_conflict(&self, conflict: SyncConflict) {
        let mut conflicts = self.conflicts.write().await;
        conflicts.push(conflict);
    }

    /// Get all sync conflicts
    pub async fn get_conflicts(&self) -> Vec<SyncConflict> {
        let conflicts = self.conflicts.read().await;
        conflicts.clone()
    }

    /// Resolve conflict by key
    pub async fn resolve_conflict(&self, key: &AppStateKey, use_remote: bool) -> Result<()> {
        let mut conflicts = self.conflicts.write().await;
        if let Some(pos) = conflicts.iter().position(|c| &c.key == key) {
            let conflict = conflicts.remove(pos);
            
            // Apply resolution
            if use_remote {
                // Use remote version
                self.update_sync_status(key.clone(), SyncStatus::Synced).await;
            } else {
                // Keep local version, mark as needing sync
                self.update_sync_status(key.clone(), SyncStatus::NotSynced).await;
            }
        }
        Ok(())
    }
}

/// Trait for app state synchronization
#[async_trait::async_trait]
pub trait AppStateSync {
    /// Get the data type this sync handler manages
    fn data_type(&self) -> AppStateDataType;

    /// Sync state from remote
    async fn sync_from_remote(&self, ctx: &SyncContext, events: Vec<AppStateEvent>) -> Result<()>;

    /// Sync state to remote
    async fn sync_to_remote(&self, ctx: &SyncContext) -> Result<Vec<AppStateEvent>>;

    /// Handle incremental sync
    async fn incremental_sync(&self, ctx: &SyncContext, since: SystemTime) -> Result<Vec<AppStateEvent>>;

    /// Handle full sync
    async fn full_sync(&self, ctx: &SyncContext) -> Result<Vec<AppStateEvent>>;

    /// Resolve conflicts
    async fn resolve_conflicts(&self, ctx: &SyncContext, conflicts: Vec<SyncConflict>) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_key_creation() {
        let contact_key = AppStateKey::contact(&JID::new("test".to_string(), "s.whatsapp.net".to_string()));
        assert_eq!(contact_key.data_type, AppStateDataType::Contacts);
        assert_eq!(contact_key.identifier, "test@s.whatsapp.net");

        let settings_key = AppStateKey::settings("theme");
        assert_eq!(settings_key.data_type, AppStateDataType::Settings);
        assert_eq!(settings_key.identifier, "theme");
    }

    #[test]
    fn test_app_state_key_string() {
        let key = AppStateKey::settings("notification_tone");
        assert_eq!(key.to_string(), "settings:notification_tone");
    }

    #[tokio::test]
    async fn test_sync_context_status() {
        use crate::database::DatabaseConfig;
        let db = Arc::new(Database::new(DatabaseConfig::in_memory()).await.unwrap());
        let ctx = SyncContext::new(db);

        let key = AppStateKey::settings("test");
        
        // Initial status should be NotSynced  
        assert_eq!(ctx.get_sync_status(&key).await, SyncStatus::NotSynced);

        // Update status
        ctx.update_sync_status(key.clone(), SyncStatus::Syncing).await;
        assert_eq!(ctx.get_sync_status(&key).await, SyncStatus::Syncing);
    }
}