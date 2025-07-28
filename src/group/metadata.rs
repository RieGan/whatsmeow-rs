/// Group metadata management for WhatsApp groups

use crate::{
    error::{Error, Result},
    types::JID,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Group metadata manager
pub struct GroupMetadataManager {
    /// Cached metadata
    metadata_cache: HashMap<JID, CachedGroupMetadata>,
    /// Configuration
    config: MetadataManagerConfig,
}

/// Configuration for metadata manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataManagerConfig {
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Cache TTL in seconds
    pub cache_ttl: u64,
    /// Whether to auto-refresh metadata
    pub auto_refresh: bool,
    /// Refresh interval in seconds
    pub refresh_interval: u64,
}

impl Default for MetadataManagerConfig {
    fn default() -> Self {
        Self {
            max_cache_size: 1000,
            cache_ttl: 3600, // 1 hour
            auto_refresh: true,
            refresh_interval: 300, // 5 minutes
        }
    }
}

/// Cached group metadata with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedGroupMetadata {
    /// The metadata
    pub metadata: GroupMetadata,
    /// Cache timestamp
    pub cached_at: SystemTime,
    /// Last updated timestamp
    pub last_updated: SystemTime,
}

/// Complete group metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupMetadata {
    /// Group JID
    pub jid: JID,
    /// Group name/subject
    pub name: String,
    /// Group description
    pub description: Option<String>,
    /// Group creation timestamp
    pub created_at: SystemTime,
    /// Group creator
    pub creator: JID,
    /// Current participant count
    pub participant_count: usize,
    /// Current admin count
    pub admin_count: usize,
    /// Group avatar/picture info
    pub avatar: Option<GroupAvatar>,
    /// Whether group is announcement-only
    pub announcement_only: bool,
    /// Whether group history is visible to new members
    pub history_visible: bool,
    /// Group restrictions
    pub restrictions: GroupRestrictions,
    /// Disappearing message settings
    pub disappearing_messages: Option<DisappearingMessageInfo>,
    /// Group statistics
    pub statistics: GroupStatistics,
    /// Custom group attributes
    pub custom_attributes: HashMap<String, String>,
}

/// Group avatar/picture information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupAvatar {
    /// Avatar URL
    pub url: String,
    /// Avatar ID/hash
    pub id: String,
    /// Avatar file size
    pub file_size: u64,
    /// Avatar dimensions
    pub dimensions: Option<(u32, u32)>,
    /// Last updated timestamp
    pub updated_at: SystemTime,
    /// Who set the avatar
    pub set_by: JID,
}

/// Group restrictions and permissions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupRestrictions {
    /// Who can add participants
    pub add_participants: ParticipantPermission,
    /// Who can edit group info
    pub edit_group_info: ParticipantPermission,
    /// Who can send messages
    pub send_messages: ParticipantPermission,
    /// Whether group is locked (no new members)
    pub locked: bool,
    /// Maximum participant limit (if different from default)
    pub max_participants: Option<usize>,
    /// Minimum age requirement
    pub min_age: Option<u8>,
    /// Whether external sharing is allowed
    pub allow_external_sharing: bool,
}

/// Permission levels for group operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantPermission {
    /// Only group creator
    CreatorOnly,
    /// Only administrators
    AdminsOnly,
    /// All participants
    Everyone,
}

/// Disappearing message information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisappearingMessageInfo {
    /// Whether disappearing messages are enabled
    pub enabled: bool,
    /// Duration in seconds
    pub duration: u64,
    /// Who enabled/disabled it
    pub set_by: JID,
    /// When it was set
    pub set_at: SystemTime,
}

/// Group statistics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupStatistics {
    /// Total messages sent in group
    pub total_messages: u64,
    /// Total media messages
    pub media_messages: u64,
    /// Most active participant
    pub most_active_participant: Option<JID>,
    /// Average messages per day
    pub avg_messages_per_day: f64,
    /// Peak activity time (hour of day)
    pub peak_activity_hour: Option<u8>,
    /// Last activity timestamp
    pub last_activity: SystemTime,
}

impl Default for GroupStatistics {
    fn default() -> Self {
        Self {
            total_messages: 0,
            media_messages: 0,
            most_active_participant: None,
            avg_messages_per_day: 0.0,
            peak_activity_hour: None,
            last_activity: SystemTime::now(),
        }
    }
}

impl Default for GroupRestrictions {
    fn default() -> Self {
        Self {
            add_participants: ParticipantPermission::AdminsOnly,
            edit_group_info: ParticipantPermission::AdminsOnly,
            send_messages: ParticipantPermission::Everyone,
            locked: false,
            max_participants: None,
            min_age: None,
            allow_external_sharing: true,
        }
    }
}

impl GroupMetadataManager {
    /// Create new metadata manager
    pub fn new() -> Self {
        Self::with_config(MetadataManagerConfig::default())
    }
    
    /// Create metadata manager with custom config
    pub fn with_config(config: MetadataManagerConfig) -> Self {
        Self {
            metadata_cache: HashMap::new(),
            config,
        }
    }
    
    /// Get group metadata
    pub async fn get_metadata(&mut self, group_jid: &JID) -> Result<GroupMetadata> {
        // Check cache first
        if let Some(cached) = self.metadata_cache.get(group_jid) {
            if !self.is_cache_expired(cached) {
                return Ok(cached.metadata.clone());
            }
        }
        
        // Fetch fresh metadata
        let metadata = self.fetch_metadata(group_jid).await?;
        
        // Cache the result
        self.cache_metadata(group_jid.clone(), metadata.clone());
        
        Ok(metadata)
    }
    
    /// Fetch metadata from server (placeholder implementation)
    async fn fetch_metadata(&self, group_jid: &JID) -> Result<GroupMetadata> {
        // In a real implementation, this would fetch from WhatsApp servers
        let creator = JID::new("creator".to_string(), "s.whatsapp.net".to_string());
        
        let metadata = GroupMetadata {
            jid: group_jid.clone(),
            name: "Sample Group".to_string(),
            description: Some("A sample group for testing".to_string()),
            created_at: SystemTime::now(),
            creator: creator.clone(),
            participant_count: 5,
            admin_count: 1,
            avatar: Some(GroupAvatar {
                url: "https://example.com/avatar.jpg".to_string(),
                id: "avatar_123".to_string(),
                file_size: 1024,
                dimensions: Some((256, 256)),
                updated_at: SystemTime::now(),
                set_by: creator,
            }),
            announcement_only: false,
            history_visible: true,
            restrictions: GroupRestrictions::default(),
            disappearing_messages: None,
            statistics: GroupStatistics::default(),
            custom_attributes: HashMap::new(),
        };
        
        Ok(metadata)
    }
    
    /// Update group metadata
    pub async fn update_metadata(
        &mut self,
        group_jid: &JID,
        updates: MetadataUpdate,
    ) -> Result<GroupMetadata> {
        // Get current metadata
        let mut metadata = self.get_metadata(group_jid).await?;
        
        // Apply updates
        if let Some(name) = updates.name {
            metadata.name = name;
        }
        
        if let Some(description) = updates.description {
            metadata.description = Some(description);
        }
        
        if let Some(announcement_only) = updates.announcement_only {
            metadata.announcement_only = announcement_only;
        }
        
        if let Some(history_visible) = updates.history_visible {
            metadata.history_visible = history_visible;
        }
        
        if let Some(restrictions) = updates.restrictions {
            metadata.restrictions = restrictions;
        }
        
        if let Some(disappearing_messages) = updates.disappearing_messages {
            metadata.disappearing_messages = Some(disappearing_messages);
        }
        
        // Update custom attributes
        for (key, value) in updates.custom_attributes {
            metadata.custom_attributes.insert(key, value);
        }
        
        // Cache updated metadata
        self.cache_metadata(group_jid.clone(), metadata.clone());
        
        tracing::info!("Updated metadata for group {}", group_jid);
        
        Ok(metadata)
    }
    
    /// Set group avatar
    pub async fn set_avatar(
        &mut self,
        group_jid: &JID,
        avatar_data: Vec<u8>,
        set_by: &JID,
    ) -> Result<GroupAvatar> {
        // Validate avatar data
        if avatar_data.len() > 2 * 1024 * 1024 {
            return Err(Error::Protocol("Avatar too large".to_string()));
        }
        
        // In a real implementation, this would upload the avatar to WhatsApp servers
        let avatar = GroupAvatar {
            url: "https://example.com/new_avatar.jpg".to_string(),
            id: format!("avatar_{}", uuid::Uuid::new_v4().simple()),
            file_size: avatar_data.len() as u64,
            dimensions: Some((256, 256)), // Would be extracted from image
            updated_at: SystemTime::now(),
            set_by: set_by.clone(),
        };
        
        // Update cached metadata
        if let Some(cached) = self.metadata_cache.get_mut(group_jid) {
            cached.metadata.avatar = Some(avatar.clone());
            cached.last_updated = SystemTime::now();
        }
        
        tracing::info!("Set avatar for group {} by {}", group_jid, set_by);
        
        Ok(avatar)
    }
    
    /// Remove group avatar
    pub async fn remove_avatar(&mut self, group_jid: &JID) -> Result<()> {
        // Update cached metadata
        if let Some(cached) = self.metadata_cache.get_mut(group_jid) {
            cached.metadata.avatar = None;
            cached.last_updated = SystemTime::now();
        }
        
        tracing::info!("Removed avatar for group {}", group_jid);
        
        Ok(())
    }
    
    /// Get group statistics
    pub async fn get_statistics(&mut self, group_jid: &JID) -> Result<GroupStatistics> {
        let metadata = self.get_metadata(group_jid).await?;
        Ok(metadata.statistics)
    }
    
    /// Update group statistics
    pub async fn update_statistics(
        &mut self,
        group_jid: &JID,
        stats_update: StatisticsUpdate,
    ) -> Result<GroupStatistics> {
        let mut metadata = self.get_metadata(group_jid).await?;
        
        // Apply statistics updates
        if let Some(total_messages) = stats_update.total_messages {
            metadata.statistics.total_messages = total_messages;
        }
        
        if let Some(media_messages) = stats_update.media_messages {
            metadata.statistics.media_messages = media_messages;
        }
        
        if let Some(most_active) = stats_update.most_active_participant {
            metadata.statistics.most_active_participant = Some(most_active);
        }
        
        if let Some(avg_messages) = stats_update.avg_messages_per_day {
            metadata.statistics.avg_messages_per_day = avg_messages;
        }
        
        if let Some(peak_hour) = stats_update.peak_activity_hour {
            metadata.statistics.peak_activity_hour = Some(peak_hour);
        }
        
        metadata.statistics.last_activity = SystemTime::now();
        
        // Cache updated metadata
        self.cache_metadata(group_jid.clone(), metadata.clone());
        
        Ok(metadata.statistics)
    }
    
    /// Cache metadata
    fn cache_metadata(&mut self, group_jid: JID, metadata: GroupMetadata) {
        // Enforce cache size limit
        if self.metadata_cache.len() >= self.config.max_cache_size {
            self.evict_oldest_cache_entry();
        }
        
        let cached = CachedGroupMetadata {
            metadata,
            cached_at: SystemTime::now(),
            last_updated: SystemTime::now(),
        };
        
        self.metadata_cache.insert(group_jid, cached);
    }
    
    /// Check if cache entry is expired
    fn is_cache_expired(&self, cached: &CachedGroupMetadata) -> bool {
        if let Ok(elapsed) = cached.cached_at.elapsed() {
            elapsed.as_secs() > self.config.cache_ttl
        } else {
            true // If we can't determine age, consider it expired
        }
    }
    
    /// Evict oldest cache entry
    fn evict_oldest_cache_entry(&mut self) {
        if let Some((oldest_key, _)) = self.metadata_cache
            .iter()
            .min_by_key(|(_, cached)| cached.cached_at)
            .map(|(key, cached)| (key.clone(), cached.clone()))
        {
            self.metadata_cache.remove(&oldest_key);
        }
    }
    
    /// Clear cache
    pub fn clear_cache(&mut self) {
        self.metadata_cache.clear();
    }
    
    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        CacheStats {
            total_entries: self.metadata_cache.len(),
            max_entries: self.config.max_cache_size,
            hit_ratio: 0.0, // Would need to track hits/misses for real ratio
        }
    }
    
    /// Refresh all cached metadata
    pub async fn refresh_all_cache(&mut self) -> Result<usize> {
        let group_jids: Vec<JID> = self.metadata_cache.keys().cloned().collect();
        let mut refreshed_count = 0;
        
        for group_jid in group_jids {
            if let Ok(metadata) = self.fetch_metadata(&group_jid).await {
                self.cache_metadata(group_jid, metadata);
                refreshed_count += 1;
            }
        }
        
        tracing::info!("Refreshed {} cached metadata entries", refreshed_count);
        
        Ok(refreshed_count)
    }
}

/// Metadata update request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataUpdate {
    /// New group name
    pub name: Option<String>,
    /// New description
    pub description: Option<String>,
    /// New announcement-only setting
    pub announcement_only: Option<bool>,
    /// New history visibility setting
    pub history_visible: Option<bool>,
    /// New restrictions
    pub restrictions: Option<GroupRestrictions>,
    /// New disappearing message settings
    pub disappearing_messages: Option<DisappearingMessageInfo>,
    /// Custom attributes to update
    pub custom_attributes: HashMap<String, String>,
}

/// Statistics update request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatisticsUpdate {
    /// New total message count
    pub total_messages: Option<u64>,
    /// New media message count
    pub media_messages: Option<u64>,
    /// New most active participant
    pub most_active_participant: Option<JID>,
    /// New average messages per day
    pub avg_messages_per_day: Option<f64>,
    /// New peak activity hour
    pub peak_activity_hour: Option<u8>,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total cached entries
    pub total_entries: usize,
    /// Maximum cache entries allowed
    pub max_entries: usize,
    /// Cache hit ratio (0.0 to 1.0)
    pub hit_ratio: f64,
}

impl Default for GroupMetadataManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_jid(user: &str) -> JID {
        JID::new(user.to_string(), "s.whatsapp.net".to_string())
    }
    
    fn create_test_group_jid() -> JID {
        JID::new("1234567890".to_string(), "g.us".to_string())
    }
    
    #[tokio::test]
    async fn test_metadata_manager_creation() {
        let manager = GroupMetadataManager::new();
        assert!(manager.metadata_cache.is_empty());
        assert_eq!(manager.config.max_cache_size, 1000);
    }
    
    #[tokio::test]
    async fn test_get_metadata() {
        let mut manager = GroupMetadataManager::new();
        let group_jid = create_test_group_jid();
        
        let metadata = manager.get_metadata(&group_jid).await.unwrap();
        
        assert_eq!(metadata.jid, group_jid);
        assert_eq!(metadata.name, "Sample Group");
        assert_eq!(metadata.participant_count, 5);
        assert_eq!(metadata.admin_count, 1);
        
        // Should be cached now
        assert_eq!(manager.metadata_cache.len(), 1);
    }
    
    #[tokio::test]
    async fn test_update_metadata() {
        let mut manager = GroupMetadataManager::new();
        let group_jid = create_test_group_jid();
        
        let mut update = MetadataUpdate::default();
        update.name = Some("Updated Group Name".to_string());
        update.description = Some("Updated description".to_string());
        update.announcement_only = Some(true);
        
        let updated_metadata = manager.update_metadata(&group_jid, update).await.unwrap();
        
        assert_eq!(updated_metadata.name, "Updated Group Name");
        assert_eq!(updated_metadata.description, Some("Updated description".to_string()));
        assert!(updated_metadata.announcement_only);
    }
    
    #[tokio::test]
    async fn test_set_avatar() {
        let mut manager = GroupMetadataManager::new();
        let group_jid = create_test_group_jid();
        let user_jid = create_test_jid("user");
        
        let avatar_data = vec![0u8; 1024]; // 1KB of fake avatar data
        
        let avatar = manager.set_avatar(&group_jid, avatar_data, &user_jid).await.unwrap();
        
        assert_eq!(avatar.file_size, 1024);
        assert_eq!(avatar.set_by, user_jid);
        assert!(avatar.url.starts_with("https://"));
        assert!(avatar.dimensions.is_some());
    }
    
    #[tokio::test]
    async fn test_avatar_size_validation() {
        let mut manager = GroupMetadataManager::new();
        let group_jid = create_test_group_jid();
        let user_jid = create_test_jid("user");
        
        // Avatar too large
        let large_avatar = vec![0u8; 3 * 1024 * 1024]; // 3MB
        let result = manager.set_avatar(&group_jid, large_avatar, &user_jid).await;
        
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_remove_avatar() {
        let mut manager = GroupMetadataManager::new();
        let group_jid = create_test_group_jid();
        let user_jid = create_test_jid("user");
        
        // First set an avatar
        let avatar_data = vec![0u8; 1024];
        manager.set_avatar(&group_jid, avatar_data, &user_jid).await.unwrap();
        
        // Then remove it
        manager.remove_avatar(&group_jid).await.unwrap();
        
        // Get metadata to verify avatar is removed
        let metadata = manager.get_metadata(&group_jid).await.unwrap();
        assert!(metadata.avatar.is_none());
    }
    
    #[tokio::test]
    async fn test_statistics_update() {
        let mut manager = GroupMetadataManager::new();
        let group_jid = create_test_group_jid();
        let user_jid = create_test_jid("most_active");
        
        let mut stats_update = StatisticsUpdate::default();
        stats_update.total_messages = Some(1000);
        stats_update.media_messages = Some(250);
        stats_update.most_active_participant = Some(user_jid.clone());
        stats_update.avg_messages_per_day = Some(50.5);
        stats_update.peak_activity_hour = Some(14); // 2 PM
        
        let updated_stats = manager.update_statistics(&group_jid, stats_update).await.unwrap();
        
        assert_eq!(updated_stats.total_messages, 1000);
        assert_eq!(updated_stats.media_messages, 250);
        assert_eq!(updated_stats.most_active_participant, Some(user_jid));
        assert_eq!(updated_stats.avg_messages_per_day, 50.5);
        assert_eq!(updated_stats.peak_activity_hour, Some(14));
    }
    
    #[tokio::test]
    async fn test_cache_operations() {
        let mut manager = GroupMetadataManager::with_config(MetadataManagerConfig {
            max_cache_size: 2,
            cache_ttl: 1,
            auto_refresh: false,
            refresh_interval: 60,
        });
        
        let group1 = create_test_group_jid();
        let group2 = JID::new("2234567890".to_string(), "g.us".to_string());
        let group3 = JID::new("3234567890".to_string(), "g.us".to_string());
        
        // Fill cache
        manager.get_metadata(&group1).await.unwrap();
        manager.get_metadata(&group2).await.unwrap();
        assert_eq!(manager.metadata_cache.len(), 2);
        
        // Adding third should evict oldest
        manager.get_metadata(&group3).await.unwrap();
        assert_eq!(manager.metadata_cache.len(), 2);
        
        // Clear cache
        manager.clear_cache();
        assert!(manager.metadata_cache.is_empty());
        
        // Get cache stats
        let stats = manager.get_cache_stats();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.max_entries, 2);
    }
    
    #[tokio::test]
    async fn test_refresh_cache() {
        let mut manager = GroupMetadataManager::new();
        let group_jid = create_test_group_jid();
        
        // Get initial metadata to populate cache
        manager.get_metadata(&group_jid).await.unwrap();
        assert_eq!(manager.metadata_cache.len(), 1);
        
        // Refresh all cache
        let refreshed = manager.refresh_all_cache().await.unwrap();
        assert_eq!(refreshed, 1);
    }
    
    #[test]
    fn test_group_restrictions() {
        let restrictions = GroupRestrictions::default();
        
        assert_eq!(restrictions.add_participants, ParticipantPermission::AdminsOnly);
        assert_eq!(restrictions.edit_group_info, ParticipantPermission::AdminsOnly);
        assert_eq!(restrictions.send_messages, ParticipantPermission::Everyone);
        assert!(!restrictions.locked);
        assert!(restrictions.max_participants.is_none());
        assert!(restrictions.allow_external_sharing);
    }
    
    #[test]
    fn test_disappearing_message_info() {
        let user_jid = create_test_jid("user");
        
        let disappearing_info = DisappearingMessageInfo {
            enabled: true,
            duration: 86400, // 24 hours
            set_by: user_jid.clone(),
            set_at: SystemTime::now(),
        };
        
        assert!(disappearing_info.enabled);
        assert_eq!(disappearing_info.duration, 86400);
        assert_eq!(disappearing_info.set_by, user_jid);
    }
    
    #[test]
    fn test_group_avatar() {
        let user_jid = create_test_jid("user");
        
        let avatar = GroupAvatar {
            url: "https://example.com/avatar.jpg".to_string(),
            id: "avatar_123".to_string(),
            file_size: 2048,
            dimensions: Some((512, 512)),
            updated_at: SystemTime::now(),
            set_by: user_jid.clone(),
        };
        
        assert_eq!(avatar.url, "https://example.com/avatar.jpg");
        assert_eq!(avatar.file_size, 2048);
        assert_eq!(avatar.dimensions, Some((512, 512)));
        assert_eq!(avatar.set_by, user_jid);
    }
}