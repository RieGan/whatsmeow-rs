/// Chat metadata synchronization system for WhatsApp App State
/// 
/// Handles synchronization of chat-level metadata including:
/// - Chat settings (muted, archived, pinned status)
/// - Chat appearance preferences
/// - Chat notification settings
/// - Chat ephemeral settings
/// - Chat wallpaper and theme settings

use crate::{
    appstate::{
        AppStateSync, AppStateEvent, AppStateOperation, AppStateDataType, 
        AppStateKey, SyncContext, SyncStatus, SyncConflict, AppStateVersion
    },
    error::{Error, Result},
    types::JID,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::SystemTime,
};
use tokio::sync::RwLock;

/// Chat metadata information synchronized with WhatsApp
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMetadata {
    /// Chat JID
    pub jid: JID,
    /// Chat is archived
    pub archived: bool,
    /// Chat is pinned
    pub pinned: bool,
    /// Chat is muted (with expiration time)
    pub muted_until: Option<SystemTime>,
    /// Chat notification settings
    pub notifications: ChatNotificationSettings,
    /// Ephemeral message settings
    pub ephemeral_setting: EphemeralSetting,
    /// Chat wallpaper settings
    pub wallpaper: Option<ChatWallpaper>,
    /// Chat theme settings
    pub theme: Option<ChatTheme>,
    /// Last message timestamp for sorting
    pub last_message_timestamp: Option<SystemTime>,
    /// Unread message count
    pub unread_count: u32,
    /// Chat marked as read timestamp
    pub last_read_timestamp: Option<SystemTime>,
    /// Chat display name override
    pub display_name_override: Option<String>,
    /// Chat labels/tags
    pub labels: Vec<String>,
    /// Last time metadata was updated
    pub last_updated: SystemTime,
    /// Sync version for conflict resolution
    pub version: AppStateVersion,
}

/// Chat notification settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatNotificationSettings {
    /// Notifications enabled
    pub enabled: bool,
    /// Sound enabled
    pub sound_enabled: bool,
    /// Vibration enabled
    pub vibration_enabled: bool,
    /// Show preview in notifications
    pub show_preview: bool,
    /// Custom notification sound
    pub custom_sound: Option<String>,
    /// High priority notifications
    pub high_priority: bool,
}

/// Ephemeral (disappearing) message settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EphemeralSetting {
    /// Ephemeral messages enabled
    pub enabled: bool,
    /// Message expiration time in seconds
    pub expiration_seconds: Option<u32>,
    /// Setting applied by (JID)
    pub set_by: Option<JID>,
    /// Timestamp when setting was applied
    pub set_timestamp: Option<SystemTime>,
}

/// Chat wallpaper configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatWallpaper {
    /// Wallpaper type
    pub wallpaper_type: WallpaperType,
    /// Wallpaper data (image, color, etc.)
    pub data: Vec<u8>,
    /// Wallpaper opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Blur effect enabled
    pub blur_enabled: bool,
}

/// Chat wallpaper types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WallpaperType {
    /// Solid color wallpaper
    SolidColor,
    /// Gradient wallpaper
    Gradient,
    /// Image wallpaper
    Image,
    /// Default WhatsApp wallpaper
    Default,
    /// No wallpaper
    None,
}

/// Chat theme settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatTheme {
    /// Theme name/identifier
    pub theme_id: String,
    /// Primary color
    pub primary_color: Option<String>,
    /// Secondary color
    pub secondary_color: Option<String>,
    /// Text color
    pub text_color: Option<String>,
    /// Background color
    pub background_color: Option<String>,
    /// Dark mode enabled
    pub dark_mode: bool,
}

/// Chat metadata synchronization manager
pub struct ChatMetadataSync {
    /// Chat metadata storage
    chat_metadata: Arc<RwLock<HashMap<JID, ChatMetadata>>>,
    /// Chat metadata cache for quick access
    metadata_cache: Arc<RwLock<HashMap<JID, ChatMetadataCache>>>,
}

/// Cached chat metadata for performance
#[derive(Debug, Clone)]
pub struct ChatMetadataCache {
    /// Cached metadata
    pub metadata: ChatMetadata,
    /// Cache timestamp
    pub cached_at: SystemTime,
    /// Cache expiry time
    pub expires_at: SystemTime,
}

/// Chat filtering and search options
#[derive(Debug, Clone)]
pub struct ChatFilter {
    /// Filter by archived status
    pub archived: Option<bool>,
    /// Filter by pinned status
    pub pinned: Option<bool>,
    /// Filter by muted status
    pub muted: Option<bool>,
    /// Filter by unread messages
    pub has_unread: Option<bool>,
    /// Filter by labels
    pub has_labels: Vec<String>,
    /// Filter by last message timestamp (after)
    pub after_timestamp: Option<SystemTime>,
    /// Filter by last message timestamp (before)
    pub before_timestamp: Option<SystemTime>,
}

impl ChatMetadata {
    /// Create new chat metadata
    pub fn new(jid: JID) -> Self {
        Self {
            jid,
            archived: false,
            pinned: false,
            muted_until: None,
            notifications: ChatNotificationSettings::default(),
            ephemeral_setting: EphemeralSetting::default(),
            wallpaper: None,
            theme: None,
            last_message_timestamp: None,
            unread_count: 0,
            last_read_timestamp: None,
            display_name_override: None,
            labels: Vec::new(),
            last_updated: SystemTime::now(),
            version: AppStateVersion {
                timestamp: SystemTime::now(),
                hash: String::new(),
                device_id: "local".to_string(),
            },
        }
    }

    /// Check if chat is muted
    pub fn is_muted(&self) -> bool {
        self.muted_until
            .map(|until| until > SystemTime::now())
            .unwrap_or(false)
    }

    /// Check if chat has unread messages
    pub fn has_unread(&self) -> bool {
        self.unread_count > 0
    }

    /// Get mute duration in seconds
    pub fn mute_duration_seconds(&self) -> Option<u64> {
        self.muted_until.and_then(|until| {
            until.duration_since(SystemTime::now()).ok().map(|d| d.as_secs())
        })
    }

    /// Update unread count
    pub fn update_unread_count(&mut self, count: u32) {
        self.unread_count = count;
        self.last_updated = SystemTime::now();
        self.version.timestamp = SystemTime::now();
    }

    /// Mark chat as read
    pub fn mark_as_read(&mut self) {
        self.unread_count = 0;
        self.last_read_timestamp = Some(SystemTime::now());
        self.last_updated = SystemTime::now();
        self.version.timestamp = SystemTime::now();
    }
}

impl Default for ChatNotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sound_enabled: true,
            vibration_enabled: true,
            show_preview: true,
            custom_sound: None,
            high_priority: false,
        }
    }
}

impl Default for EphemeralSetting {
    fn default() -> Self {
        Self {
            enabled: false,
            expiration_seconds: None,
            set_by: None,
            set_timestamp: None,
        }
    }
}

impl ChatMetadataSync {
    /// Create a new chat metadata sync manager
    pub fn new() -> Self {
        Self {
            chat_metadata: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update chat metadata
    pub async fn update_chat_metadata(&self, metadata: ChatMetadata) -> Result<()> {
        let jid = metadata.jid.clone();

        // Update storage
        {
            let mut storage = self.chat_metadata.write().await;
            storage.insert(jid.clone(), metadata.clone());
        }

        // Update cache
        self.update_cache(&jid, metadata).await;

        Ok(())
    }

    /// Get chat metadata by JID
    pub async fn get_chat_metadata(&self, jid: &JID) -> Option<ChatMetadata> {
        // Try cache first
        if let Some(cached) = self.get_from_cache(jid).await {
            return Some(cached);
        }

        // Fall back to storage
        let storage = self.chat_metadata.read().await;
        let metadata = storage.get(jid).cloned();

        // Update cache if found
        if let Some(ref metadata) = metadata {
            self.update_cache(jid, metadata.clone()).await;
        }

        metadata
    }

    /// Get all chat metadata
    pub async fn get_all_chat_metadata(&self) -> Vec<ChatMetadata> {
        let storage = self.chat_metadata.read().await;
        storage.values().cloned().collect()
    }

    /// Search chats with filter
    pub async fn search_chats(&self, filter: ChatFilter) -> Vec<ChatMetadata> {
        let storage = self.chat_metadata.read().await;

        storage.values()
            .filter(|metadata| {
                // Archived filter
                if let Some(archived) = filter.archived {
                    if metadata.archived != archived {
                        return false;
                    }
                }

                // Pinned filter
                if let Some(pinned) = filter.pinned {
                    if metadata.pinned != pinned {
                        return false;
                    }
                }

                // Muted filter
                if let Some(muted) = filter.muted {
                    if metadata.is_muted() != muted {
                        return false;
                    }
                }

                // Unread filter
                if let Some(has_unread) = filter.has_unread {
                    if metadata.has_unread() != has_unread {
                        return false;
                    }
                }

                // Labels filter
                if !filter.has_labels.is_empty() {
                    let has_any_label = filter.has_labels.iter()
                        .any(|label| metadata.labels.contains(label));
                    if !has_any_label {
                        return false;
                    }
                }

                // Timestamp filters
                if let Some(after) = filter.after_timestamp {
                    if metadata.last_message_timestamp.map_or(true, |ts| ts <= after) {
                        return false;
                    }
                }

                if let Some(before) = filter.before_timestamp {
                    if metadata.last_message_timestamp.map_or(true, |ts| ts >= before) {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect()
    }

    /// Archive a chat
    pub async fn archive_chat(&self, jid: &JID) -> Result<()> {
        let mut storage = self.chat_metadata.write().await;
        if let Some(metadata) = storage.get_mut(jid) {
            metadata.archived = true;
            metadata.last_updated = SystemTime::now();
            metadata.version.timestamp = SystemTime::now();
            metadata.version.hash = self.calculate_metadata_hash(metadata);
        }
        Ok(())
    }

    /// Unarchive a chat
    pub async fn unarchive_chat(&self, jid: &JID) -> Result<()> {
        let mut storage = self.chat_metadata.write().await;
        if let Some(metadata) = storage.get_mut(jid) {
            metadata.archived = false;
            metadata.last_updated = SystemTime::now();
            metadata.version.timestamp = SystemTime::now();
            metadata.version.hash = self.calculate_metadata_hash(metadata);
        }
        Ok(())
    }

    /// Pin a chat
    pub async fn pin_chat(&self, jid: &JID) -> Result<()> {
        let mut storage = self.chat_metadata.write().await;
        if let Some(metadata) = storage.get_mut(jid) {
            metadata.pinned = true;
            metadata.last_updated = SystemTime::now();
            metadata.version.timestamp = SystemTime::now();
            metadata.version.hash = self.calculate_metadata_hash(metadata);
        }
        Ok(())
    }

    /// Unpin a chat
    pub async fn unpin_chat(&self, jid: &JID) -> Result<()> {
        let mut storage = self.chat_metadata.write().await;
        if let Some(metadata) = storage.get_mut(jid) {
            metadata.pinned = false;
            metadata.last_updated = SystemTime::now();
            metadata.version.timestamp = SystemTime::now();
            metadata.version.hash = self.calculate_metadata_hash(metadata);
        }
        Ok(())
    }

    /// Mute a chat
    pub async fn mute_chat(&self, jid: &JID, duration_seconds: Option<u64>) -> Result<()> {
        let mut storage = self.chat_metadata.write().await;
        if let Some(metadata) = storage.get_mut(jid) {
            metadata.muted_until = duration_seconds.map(|dur| {
                SystemTime::now() + std::time::Duration::from_secs(dur)
            });
            metadata.last_updated = SystemTime::now();
            metadata.version.timestamp = SystemTime::now();
            metadata.version.hash = self.calculate_metadata_hash(metadata);
        }
        Ok(())
    }

    /// Unmute a chat
    pub async fn unmute_chat(&self, jid: &JID) -> Result<()> {
        let mut storage = self.chat_metadata.write().await;
        if let Some(metadata) = storage.get_mut(jid) {
            metadata.muted_until = None;
            metadata.last_updated = SystemTime::now();
            metadata.version.timestamp = SystemTime::now();
            metadata.version.hash = self.calculate_metadata_hash(metadata);
        }
        Ok(())
    }

    /// Set ephemeral messages for a chat
    pub async fn set_ephemeral_setting(&self, jid: &JID, setting: EphemeralSetting) -> Result<()> {
        let mut storage = self.chat_metadata.write().await;
        if let Some(metadata) = storage.get_mut(jid) {
            metadata.ephemeral_setting = setting;
            metadata.last_updated = SystemTime::now();
            metadata.version.timestamp = SystemTime::now();
            metadata.version.hash = self.calculate_metadata_hash(metadata);
        }
        Ok(())
    }

    /// Add label to chat
    pub async fn add_label_to_chat(&self, jid: &JID, label: String) -> Result<()> {
        let mut storage = self.chat_metadata.write().await;
        if let Some(metadata) = storage.get_mut(jid) {
            if !metadata.labels.contains(&label) {
                metadata.labels.push(label);
                metadata.last_updated = SystemTime::now();
                metadata.version.timestamp = SystemTime::now();
                metadata.version.hash = self.calculate_metadata_hash(metadata);
            }
        }
        Ok(())
    }

    /// Remove label from chat
    pub async fn remove_label_from_chat(&self, jid: &JID, label: &str) -> Result<()> {
        let mut storage = self.chat_metadata.write().await;
        if let Some(metadata) = storage.get_mut(jid) {
            metadata.labels.retain(|l| l != label);
            metadata.last_updated = SystemTime::now();
            metadata.version.timestamp = SystemTime::now();
            metadata.version.hash = self.calculate_metadata_hash(metadata);
        }
        Ok(())
    }

    /// Delete chat metadata
    pub async fn delete_chat_metadata(&self, jid: &JID) -> Result<Option<ChatMetadata>> {
        let mut storage = self.chat_metadata.write().await;
        let metadata = storage.remove(jid);

        // Remove from cache
        {
            let mut cache = self.metadata_cache.write().await;
            cache.remove(jid);
        }

        Ok(metadata)
    }

    /// Get chat statistics
    pub async fn get_chat_stats(&self) -> ChatStats {
        let storage = self.chat_metadata.read().await;
        let total = storage.len();
        let archived = storage.values().filter(|m| m.archived).count();
        let pinned = storage.values().filter(|m| m.pinned).count();
        let muted = storage.values().filter(|m| m.is_muted()).count();
        let unread = storage.values().filter(|m| m.has_unread()).count();
        let total_unread_count: u32 = storage.values().map(|m| m.unread_count).sum();

        ChatStats {
            total_chats: total,
            archived_chats: archived,
            pinned_chats: pinned,
            muted_chats: muted,
            chats_with_unread: unread,
            total_unread_messages: total_unread_count,
        }
    }

    /// Get cached metadata if valid
    async fn get_from_cache(&self, jid: &JID) -> Option<ChatMetadata> {
        let cache = self.metadata_cache.read().await;
        if let Some(cached) = cache.get(jid) {
            if cached.expires_at > SystemTime::now() {
                return Some(cached.metadata.clone());
            }
        }
        None
    }

    /// Update cache with metadata
    async fn update_cache(&self, jid: &JID, metadata: ChatMetadata) {
        let cache_entry = ChatMetadataCache {
            metadata,
            cached_at: SystemTime::now(),
            expires_at: SystemTime::now() + std::time::Duration::from_secs(300), // 5 minutes
        };

        let mut cache = self.metadata_cache.write().await;
        cache.insert(jid.clone(), cache_entry);
    }

    /// Calculate hash for metadata version
    fn calculate_metadata_hash(&self, metadata: &ChatMetadata) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        metadata.archived.hash(&mut hasher);
        metadata.pinned.hash(&mut hasher);
        metadata.muted_until.hash(&mut hasher);
        metadata.unread_count.hash(&mut hasher);
        metadata.labels.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }

    /// Merge metadata for conflict resolution
    pub fn merge_metadata(&self, local: &ChatMetadata, remote: &ChatMetadata) -> ChatMetadata {
        let mut merged = local.clone();

        // Use the most recent version for most fields
        if remote.version.timestamp > local.version.timestamp {
            merged.archived = remote.archived;
            merged.pinned = remote.pinned;
            merged.muted_until = remote.muted_until;
            merged.notifications = remote.notifications.clone();
            merged.ephemeral_setting = remote.ephemeral_setting.clone();
            merged.wallpaper = remote.wallpaper.clone();
            merged.theme = remote.theme.clone();
            merged.display_name_override = remote.display_name_override.clone();
            merged.version = remote.version.clone();
        }

        // Merge labels (union)
        for label in &remote.labels {
            if !merged.labels.contains(label) {
                merged.labels.push(label.clone());
            }
        }

        // Use the most recent unread count and timestamps
        if remote.last_message_timestamp > local.last_message_timestamp {
            merged.last_message_timestamp = remote.last_message_timestamp;
            merged.unread_count = remote.unread_count;
        }

        if remote.last_read_timestamp > local.last_read_timestamp {
            merged.last_read_timestamp = remote.last_read_timestamp;
        }

        // Update timestamp
        merged.last_updated = SystemTime::now();

        merged
    }
}

/// Chat statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStats {
    pub total_chats: usize,
    pub archived_chats: usize,
    pub pinned_chats: usize,
    pub muted_chats: usize,
    pub chats_with_unread: usize,
    pub total_unread_messages: u32,
}

#[async_trait::async_trait]
impl AppStateSync for ChatMetadataSync {
    fn data_type(&self) -> AppStateDataType {
        AppStateDataType::ChatMetadata
    }

    async fn sync_from_remote(&self, ctx: &SyncContext, events: Vec<AppStateEvent>) -> Result<()> {
        for event in events {
            match event.operation {
                AppStateOperation::Update => {
                    if let Some(data) = event.data {
                        let metadata: ChatMetadata = serde_json::from_slice(&data)
                            .map_err(|e| Error::Protocol(format!("Failed to deserialize chat metadata: {}", e)))?;

                        let key = AppStateKey::chat_metadata(&metadata.jid);

                        // Check for conflicts
                        if let Some(existing) = self.get_chat_metadata(&metadata.jid).await {
                            if existing.version.timestamp > metadata.version.timestamp {
                                // Local version is newer, create conflict
                                let conflict = SyncConflict {
                                    key: key.clone(),
                                    local_version: existing.version,
                                    remote_version: metadata.version,
                                    local_data: Some(serde_json::to_vec(&existing).unwrap()),
                                    remote_data: Some(data),
                                    detected_at: SystemTime::now(),
                                };
                                ctx.add_conflict(conflict).await;
                                ctx.update_sync_status(key, SyncStatus::Conflict).await;
                                continue;
                            }
                        }

                        self.update_chat_metadata(metadata).await?;
                        ctx.update_sync_status(key, SyncStatus::Synced).await;
                    }
                }
                AppStateOperation::Delete => {
                    if let Ok(jid) = event.key.parse::<JID>() {
                        self.delete_chat_metadata(&jid).await?;
                        let key = AppStateKey::chat_metadata(&jid);
                        ctx.update_sync_status(key, SyncStatus::Synced).await;
                    }
                }
                _ => {
                    // Handle other operations as needed
                }
            }
        }

        ctx.update_last_sync(AppStateDataType::ChatMetadata).await;
        Ok(())
    }

    async fn sync_to_remote(&self, ctx: &SyncContext) -> Result<Vec<AppStateEvent>> {
        let mut events = Vec::new();
        let all_metadata = self.get_all_chat_metadata().await;

        for metadata in all_metadata {
            let key = AppStateKey::chat_metadata(&metadata.jid);
            let status = ctx.get_sync_status(&key).await;

            if status == SyncStatus::NotSynced {
                let data = serde_json::to_vec(&metadata)
                    .map_err(|e| Error::Protocol(format!("Failed to serialize chat metadata: {}", e)))?;

                events.push(AppStateEvent {
                    data_type: AppStateDataType::ChatMetadata,
                    operation: AppStateOperation::Update,
                    timestamp: metadata.last_updated,
                    key: metadata.jid.to_string(),
                    data: Some(data),
                });

                ctx.update_sync_status(key, SyncStatus::Syncing).await;
            }
        }

        Ok(events)
    }

    async fn incremental_sync(&self, ctx: &SyncContext, since: SystemTime) -> Result<Vec<AppStateEvent>> {
        let mut events = Vec::new();
        let all_metadata = self.get_all_chat_metadata().await;

        for metadata in all_metadata {
            if metadata.last_updated > since {
                let data = serde_json::to_vec(&metadata)
                    .map_err(|e| Error::Protocol(format!("Failed to serialize chat metadata: {}", e)))?;

                events.push(AppStateEvent {
                    data_type: AppStateDataType::ChatMetadata,
                    operation: AppStateOperation::Update,
                    timestamp: metadata.last_updated,
                    key: metadata.jid.to_string(),
                    data: Some(data),
                });
            }
        }

        Ok(events)
    }

    async fn full_sync(&self, ctx: &SyncContext) -> Result<Vec<AppStateEvent>> {
        self.sync_to_remote(ctx).await
    }

    async fn resolve_conflicts(&self, ctx: &SyncContext, conflicts: Vec<SyncConflict>) -> Result<()> {
        for conflict in conflicts {
            if let (Some(local_data), Some(remote_data)) = (&conflict.local_data, &conflict.remote_data) {
                let local_metadata: ChatMetadata = serde_json::from_slice(local_data)
                    .map_err(|e| Error::Protocol(format!("Failed to deserialize local chat metadata: {}", e)))?;
                let remote_metadata: ChatMetadata = serde_json::from_slice(remote_data)
                    .map_err(|e| Error::Protocol(format!("Failed to deserialize remote chat metadata: {}", e)))?;

                // Merge metadata
                let merged = self.merge_metadata(&local_metadata, &remote_metadata);
                self.update_chat_metadata(merged).await?;

                ctx.update_sync_status(conflict.key, SyncStatus::Synced).await;
            }
        }
        Ok(())
    }
}

impl Default for ChatFilter {
    fn default() -> Self {
        Self {
            archived: None,
            pinned: None,
            muted: None,
            has_unread: None,
            has_labels: Vec::new(),
            after_timestamp: None,
            before_timestamp: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chat_metadata_basic_operations() {
        let sync = ChatMetadataSync::new();
        
        let jid = JID::new("test".to_string(), "s.whatsapp.net".to_string());
        let mut metadata = ChatMetadata::new(jid.clone());
        metadata.unread_count = 5;
        metadata.archived = true;

        // Add metadata
        sync.update_chat_metadata(metadata.clone()).await.unwrap();

        // Get metadata
        let retrieved = sync.get_chat_metadata(&jid).await.unwrap();
        assert_eq!(retrieved.unread_count, 5);
        assert!(retrieved.archived);
        assert!(retrieved.has_unread());

        // Mark as read
        let mut updated = retrieved.clone();
        updated.mark_as_read();
        sync.update_chat_metadata(updated).await.unwrap();

        let after_read = sync.get_chat_metadata(&jid).await.unwrap();
        assert_eq!(after_read.unread_count, 0);
        assert!(!after_read.has_unread());
    }

    #[tokio::test]
    async fn test_chat_filtering() {
        let sync = ChatMetadataSync::new();
        
        // Add test metadata
        let jid1 = JID::new("chat1".to_string(), "g.us".to_string());
        let mut metadata1 = ChatMetadata::new(jid1);
        metadata1.archived = true;
        metadata1.pinned = false;
        metadata1.unread_count = 3;

        let jid2 = JID::new("chat2".to_string(), "g.us".to_string());
        let mut metadata2 = ChatMetadata::new(jid2);
        metadata2.archived = false;
        metadata2.pinned = true;
        metadata2.unread_count = 0;

        sync.update_chat_metadata(metadata1).await.unwrap();
        sync.update_chat_metadata(metadata2).await.unwrap();

        // Test archived filter
        let filter = ChatFilter {
            archived: Some(true),
            ..Default::default()
        };
        let results = sync.search_chats(filter).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].archived);

        // Test unread filter
        let filter = ChatFilter {
            has_unread: Some(true),
            ..Default::default()
        };
        let results = sync.search_chats(filter).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].has_unread());

        // Test pinned filter
        let filter = ChatFilter {
            pinned: Some(true),
            ..Default::default()
        };
        let results = sync.search_chats(filter).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].pinned);
    }

    #[tokio::test]
    async fn test_mute_functionality() {
        let sync = ChatMetadataSync::new();
        
        let jid = JID::new("test".to_string(), "s.whatsapp.net".to_string());
        let metadata = ChatMetadata::new(jid.clone());
        sync.update_chat_metadata(metadata).await.unwrap();

        // Mute chat for 1 hour
        sync.mute_chat(&jid, Some(3600)).await.unwrap();

        let muted_metadata = sync.get_chat_metadata(&jid).await.unwrap();
        assert!(muted_metadata.is_muted());
        assert!(muted_metadata.mute_duration_seconds().unwrap() > 3500); // Should be close to 3600

        // Unmute chat
        sync.unmute_chat(&jid).await.unwrap();

        let unmuted_metadata = sync.get_chat_metadata(&jid).await.unwrap();
        assert!(!unmuted_metadata.is_muted());
    }

    #[tokio::test]
    async fn test_ephemeral_settings() {
        let sync = ChatMetadataSync::new();
        
        let jid = JID::new("test".to_string(), "g.us".to_string());
        let metadata = ChatMetadata::new(jid.clone());
        sync.update_chat_metadata(metadata).await.unwrap();

        let ephemeral_setting = EphemeralSetting {
            enabled: true,
            expiration_seconds: Some(86400), // 24 hours
            set_by: Some(jid.clone()),
            set_timestamp: Some(SystemTime::now()),
        };

        sync.set_ephemeral_setting(&jid, ephemeral_setting.clone()).await.unwrap();

        let updated_metadata = sync.get_chat_metadata(&jid).await.unwrap();
        assert!(updated_metadata.ephemeral_setting.enabled);
        assert_eq!(updated_metadata.ephemeral_setting.expiration_seconds, Some(86400));
    }
}