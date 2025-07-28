/// Group participant management for WhatsApp groups

use crate::{
    error::{Error, Result},
    types::JID,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Group participant manager
pub struct ParticipantManager {
    /// Participant cache by group
    participant_cache: HashMap<JID, CachedParticipants>,
    /// Configuration
    config: ParticipantManagerConfig,
}

/// Configuration for participant manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantManagerConfig {
    /// Maximum cache size (number of groups)
    pub max_cache_size: usize,
    /// Cache TTL in seconds
    pub cache_ttl: u64,
    /// Whether to auto-sync participant changes
    pub auto_sync: bool,
    /// Batch size for participant operations
    pub batch_size: usize,
}

impl Default for ParticipantManagerConfig {
    fn default() -> Self {
        Self {
            max_cache_size: 500,
            cache_ttl: 1800, // 30 minutes
            auto_sync: true,
            batch_size: 50,
        }
    }
}

/// Cached participants with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedParticipants {
    /// List of participants
    pub participants: Vec<GroupParticipant>,
    /// Cache timestamp
    pub cached_at: SystemTime,
    /// Last sync timestamp
    pub last_sync: SystemTime,
}

/// Group participant with detailed information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupParticipant {
    /// Participant JID
    pub jid: JID,
    /// Display name in group
    pub display_name: Option<String>,
    /// Role in group
    pub role: ParticipantRole,
    /// When participant joined
    pub joined_at: SystemTime,
    /// Who added this participant
    pub added_by: Option<JID>,
    /// Participant status
    pub status: ParticipantStatus,
    /// Participant permissions
    pub permissions: ParticipantPermissions,
    /// Custom attributes
    pub attributes: HashMap<String, String>,
    /// Last seen timestamp
    pub last_seen: Option<SystemTime>,
    /// Message statistics
    pub message_stats: MessageStats,
}

/// Participant role in group
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParticipantRole {
    /// Group creator (super admin)
    Creator,
    /// Administrator
    Admin,
    /// Regular participant
    Member,
}

/// Participant status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantStatus {
    /// Active participant
    Active,
    /// Temporarily muted
    Muted,
    /// Kicked from group
    Kicked,
    /// Left the group
    Left,
    /// Banned from group
    Banned,
    /// Pending invitation
    Pending,
}

/// Participant permissions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticipantPermissions {
    /// Can send messages
    pub can_send_messages: bool,
    /// Can send media
    pub can_send_media: bool,
    /// Can add participants
    pub can_add_participants: bool,
    /// Can edit group info
    pub can_edit_group_info: bool,
    /// Can change permissions of others
    pub can_change_permissions: bool,
    /// Can delete messages of others
    pub can_delete_messages: bool,
    /// Can pin messages
    pub can_pin_messages: bool,
}

/// Message statistics for participant
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageStats {
    /// Total messages sent
    pub total_messages: u64,
    /// Media messages sent
    pub media_messages: u64,
    /// Text messages sent
    pub text_messages: u64,
    /// Messages in last 24 hours
    pub messages_today: u64,
    /// Average messages per day
    pub avg_messages_per_day: f64,
    /// Last message timestamp
    pub last_message_at: Option<SystemTime>,
}

impl Default for MessageStats {
    fn default() -> Self {
        Self {
            total_messages: 0,
            media_messages: 0,
            text_messages: 0,
            messages_today: 0,
            avg_messages_per_day: 0.0,
            last_message_at: None,
        }
    }
}

impl Default for ParticipantPermissions {
    fn default() -> Self {
        Self {
            can_send_messages: true,
            can_send_media: true,
            can_add_participants: false,
            can_edit_group_info: false,
            can_change_permissions: false,
            can_delete_messages: false,
            can_pin_messages: false,
        }
    }
}

/// Participant operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantOperation {
    /// Operation type
    pub operation: ParticipantOperationType,
    /// Target participants
    pub participants: Vec<JID>,
    /// Optional parameters
    pub parameters: HashMap<String, String>,
    /// Who requested the operation
    pub requested_by: JID,
}

/// Types of participant operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParticipantOperationType {
    /// Add participants to group
    Add,
    /// Remove participants from group
    Remove,
    /// Promote to admin
    Promote,
    /// Demote from admin
    Demote,
    /// Mute participant
    Mute,
    /// Unmute participant
    Unmute,
    /// Ban participant
    Ban,
    /// Unban participant
    Unban,
    /// Update permissions
    UpdatePermissions,
}

/// Participant operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantOperationResult {
    /// Operation that was performed
    pub operation: ParticipantOperationType,
    /// Successfully processed participants
    pub successful: Vec<JID>,
    /// Failed participants with reasons
    pub failed: Vec<(JID, String)>,
    /// Warnings (non-fatal issues)
    pub warnings: Vec<String>,
    /// Operation timestamp
    pub timestamp: SystemTime,
}

impl ParticipantOperationResult {
    /// Create new participant operation result
    pub fn new() -> Self {
        Self {
            operation: ParticipantOperationType::Add, // Default operation
            successful: Vec::new(),
            failed: Vec::new(),
            warnings: Vec::new(),
            timestamp: SystemTime::now(),
        }
    }
    
    /// Create new result with specific operation
    pub fn with_operation(operation: ParticipantOperationType) -> Self {
        Self {
            operation,
            successful: Vec::new(),
            failed: Vec::new(),
            warnings: Vec::new(),
            timestamp: SystemTime::now(),
        }
    }
    
    /// Add a successful participant
    pub fn add_success(&mut self, jid: JID) {
        self.successful.push(jid);
    }
    
    /// Add a failed participant with reason
    pub fn add_failure(&mut self, jid: JID, reason: String) {
        self.failed.push((jid, reason));
    }
    
    /// Add a warning
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
    
    /// Check if all operations were successful
    pub fn all_successful(&self) -> bool {
        self.failed.is_empty()
    }
    
    /// Check if any operations were successful
    pub fn any_successful(&self) -> bool {
        !self.successful.is_empty()
    }
    
    /// Get count of successful operations
    pub fn success_count(&self) -> usize {
        self.successful.len()
    }
    
    /// Get count of failed operations
    pub fn failure_count(&self) -> usize {
        self.failed.len()
    }
    
    /// Check if operation was completely successful
    pub fn is_complete_success(&self) -> bool {
        !self.successful.is_empty() && self.failed.is_empty()
    }
    
    /// Check if operation completely failed
    pub fn is_complete_failure(&self) -> bool {
        self.successful.is_empty() && !self.failed.is_empty()
    }
}

impl ParticipantManager {
    /// Create new participant manager
    pub fn new() -> Self {
        Self::with_config(ParticipantManagerConfig::default())
    }
    
    /// Create participant manager with custom config
    pub fn with_config(config: ParticipantManagerConfig) -> Self {
        Self {
            participant_cache: HashMap::new(),
            config,
        }
    }
    
    /// Get participants for a group
    pub async fn get_participants(&mut self, group_jid: &JID) -> Result<Vec<GroupParticipant>> {
        // Check cache first
        if let Some(cached) = self.participant_cache.get(group_jid) {
            if !self.is_cache_expired(cached) {
                return Ok(cached.participants.clone());
            }
        }
        
        // Fetch fresh participant list
        let participants = self.fetch_participants(group_jid).await?;
        
        // Cache the result
        self.cache_participants(group_jid.clone(), participants.clone());
        
        Ok(participants)
    }
    
    /// Fetch participants from server (placeholder implementation)
    async fn fetch_participants(&self, group_jid: &JID) -> Result<Vec<GroupParticipant>> {
        // In a real implementation, this would fetch from WhatsApp servers
        let creator = JID::new("creator".to_string(), "s.whatsapp.net".to_string());
        let member1 = JID::new("member1".to_string(), "s.whatsapp.net".to_string());
        let member2 = JID::new("member2".to_string(), "s.whatsapp.net".to_string());
        
        let participants = vec![
            GroupParticipant {
                jid: creator.clone(),
                display_name: Some("Group Creator".to_string()),
                role: ParticipantRole::Creator,
                joined_at: SystemTime::now(),
                added_by: None,
                status: ParticipantStatus::Active,
                permissions: ParticipantPermissions {
                    can_send_messages: true,
                    can_send_media: true,
                    can_add_participants: true,
                    can_edit_group_info: true,
                    can_change_permissions: true,
                    can_delete_messages: true,
                    can_pin_messages: true,
                },
                attributes: HashMap::new(),
                last_seen: Some(SystemTime::now()),
                message_stats: MessageStats {
                    total_messages: 50,
                    text_messages: 40,
                    media_messages: 10,
                    messages_today: 5,
                    avg_messages_per_day: 2.5,
                    last_message_at: Some(SystemTime::now()),
                },
            },
            GroupParticipant {
                jid: member1.clone(),
                display_name: Some("Member One".to_string()),
                role: ParticipantRole::Member,
                joined_at: SystemTime::now(),
                added_by: Some(creator.clone()),
                status: ParticipantStatus::Active,
                permissions: ParticipantPermissions::default(),
                attributes: HashMap::new(),
                last_seen: Some(SystemTime::now()),
                message_stats: MessageStats::default(),
            },
            GroupParticipant {
                jid: member2,
                display_name: Some("Member Two".to_string()),
                role: ParticipantRole::Member,
                joined_at: SystemTime::now(),
                added_by: Some(creator),
                status: ParticipantStatus::Active,
                permissions: ParticipantPermissions::default(),
                attributes: HashMap::new(),
                last_seen: None,
                message_stats: MessageStats::default(),
            },
        ];
        
        tracing::info!("Fetched {} participants for group {}", participants.len(), group_jid);
        
        Ok(participants)
    }
    
    /// Add participants to group
    pub async fn add_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
        added_by: &JID,
    ) -> Result<ParticipantOperationResult> {
        let mut result = ParticipantOperationResult {
            operation: ParticipantOperationType::Add,
            successful: Vec::new(),
            failed: Vec::new(),
            warnings: Vec::new(),
            timestamp: SystemTime::now(),
        };
        
        // Process in batches
        for batch in participants.chunks(self.config.batch_size) {
            for participant in batch {
                // Validate participant
                if let Err(e) = self.validate_participant(participant) {
                    result.failed.push((participant.clone(), e.to_string()));
                    continue;
                }
                
                // Check if already in group
                if let Ok(current_participants) = self.get_participants(group_jid).await {
                    if current_participants.iter().any(|p| p.jid == *participant) {
                        result.warnings.push(format!("Participant {} already in group", participant));
                        continue;
                    }
                }
                
                // Add participant (simulate success)
                result.successful.push(participant.clone());
                
                // Update cache
                if let Some(cached) = self.participant_cache.get_mut(group_jid) {
                    let new_participant = GroupParticipant {
                        jid: participant.clone(),
                        display_name: None,
                        role: ParticipantRole::Member,
                        joined_at: SystemTime::now(),
                        added_by: Some(added_by.clone()),
                        status: ParticipantStatus::Active,
                        permissions: ParticipantPermissions::default(),
                        attributes: HashMap::new(),
                        last_seen: None,
                        message_stats: MessageStats::default(),
                    };
                    cached.participants.push(new_participant);
                    cached.last_sync = SystemTime::now();
                }
            }
        }
        
        tracing::info!(
            "Added participants to group {}: {} successful, {} failed",
            group_jid,
            result.successful.len(),
            result.failed.len()
        );
        
        Ok(result)
    }
    
    /// Remove participants from group
    pub async fn remove_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
        removed_by: &JID,
    ) -> Result<ParticipantOperationResult> {
        let mut result = ParticipantOperationResult {
            operation: ParticipantOperationType::Remove,
            successful: Vec::new(),
            failed: Vec::new(),
            warnings: Vec::new(),
            timestamp: SystemTime::now(),
        };
        
        // Get current participants
        let current_participants = self.get_participants(group_jid).await?;
        
        for participant in participants {
            // Check if participant exists in group
            if !current_participants.iter().any(|p| p.jid == participant) {
                result.failed.push((participant, "Participant not in group".to_string()));
                continue;
            }
            
            // Check permissions (e.g., can't remove creator)
            if let Some(p) = current_participants.iter().find(|p| p.jid == participant) {
                if p.role == ParticipantRole::Creator {
                    result.failed.push((participant, "Cannot remove group creator".to_string()));
                    continue;
                }
            }
            
            // Remove participant (simulate success)
            result.successful.push(participant.clone());
            
            // Update cache
            if let Some(cached) = self.participant_cache.get_mut(group_jid) {
                cached.participants.retain(|p| p.jid != participant);
                cached.last_sync = SystemTime::now();
            }
        }
        
        tracing::info!(
            "Removed participants from group {}: {} successful, {} failed",
            group_jid,
            result.successful.len(),
            result.failed.len()
        );
        
        Ok(result)
    }
    
    /// Promote participants to admin
    pub async fn promote_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
        promoted_by: &JID,
    ) -> Result<ParticipantOperationResult> {
        let mut result = ParticipantOperationResult {
            operation: ParticipantOperationType::Promote,
            successful: Vec::new(),
            failed: Vec::new(),
            warnings: Vec::new(),
            timestamp: SystemTime::now(),
        };
        
        for participant in participants {
            // Update role in cache
            if let Some(cached) = self.participant_cache.get_mut(group_jid) {
                if let Some(p) = cached.participants.iter_mut().find(|p| p.jid == participant) {
                    if p.role != ParticipantRole::Admin && p.role != ParticipantRole::Creator {
                        p.role = ParticipantRole::Admin;
                        // Update permissions for admin
                        p.permissions.can_add_participants = true;
                        p.permissions.can_edit_group_info = true;
                        p.permissions.can_delete_messages = true;
                        p.permissions.can_pin_messages = true;
                        
                        result.successful.push(participant);
                        cached.last_sync = SystemTime::now();
                    } else {
                        result.warnings.push(format!("Participant {} is already admin or creator", participant));
                    }
                } else {
                    result.failed.push((participant, "Participant not in group".to_string()));
                }
            } else {
                result.failed.push((participant, "Group not found in cache".to_string()));
            }
        }
        
        tracing::info!(
            "Promoted participants in group {}: {} successful, {} failed",
            group_jid,
            result.successful.len(),
            result.failed.len()
        );
        
        Ok(result)
    }
    
    /// Demote participants from admin
    pub async fn demote_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
        demoted_by: &JID,
    ) -> Result<ParticipantOperationResult> {
        let mut result = ParticipantOperationResult {
            operation: ParticipantOperationType::Demote,
            successful: Vec::new(),
            failed: Vec::new(),
            warnings: Vec::new(),
            timestamp: SystemTime::now(),
        };
        
        for participant in participants {
            // Update role in cache
            if let Some(cached) = self.participant_cache.get_mut(group_jid) {
                if let Some(p) = cached.participants.iter_mut().find(|p| p.jid == participant) {
                    if p.role == ParticipantRole::Admin {
                        p.role = ParticipantRole::Member;
                        // Reset permissions to default
                        p.permissions = ParticipantPermissions::default();
                        
                        result.successful.push(participant);
                        cached.last_sync = SystemTime::now();
                    } else if p.role == ParticipantRole::Creator {
                        result.failed.push((participant, "Cannot demote group creator".to_string()));
                    } else {
                        result.warnings.push(format!("Participant {} is not an admin", participant));
                    }
                } else {
                    result.failed.push((participant, "Participant not in group".to_string()));
                }
            } else {
                result.failed.push((participant, "Group not found in cache".to_string()));
            }
        }
        
        tracing::info!(
            "Demoted participants in group {}: {} successful, {} failed",
            group_jid,
            result.successful.len(),
            result.failed.len()
        );
        
        Ok(result)
    }
    
    /// Update participant permissions
    pub async fn update_permissions(
        &mut self,
        group_jid: &JID,
        participant_jid: &JID,
        permissions: ParticipantPermissions,
    ) -> Result<()> {
        if let Some(cached) = self.participant_cache.get_mut(group_jid) {
            if let Some(p) = cached.participants.iter_mut().find(|p| p.jid == *participant_jid) {
                p.permissions = permissions;
                cached.last_sync = SystemTime::now();
                
                tracing::info!("Updated permissions for {} in group {}", participant_jid, group_jid);
                return Ok(());
            }
        }
        
        Err(Error::Protocol("Participant not found".to_string()))
    }
    
    /// Get participant by JID
    pub async fn get_participant(
        &mut self,
        group_jid: &JID,
        participant_jid: &JID,
    ) -> Result<GroupParticipant> {
        let participants = self.get_participants(group_jid).await?;
        
        participants
            .into_iter()
            .find(|p| p.jid == *participant_jid)
            .ok_or_else(|| Error::Protocol("Participant not found".to_string()))
    }
    
    /// Get participants by role
    pub async fn get_participants_by_role(
        &mut self,
        group_jid: &JID,
        role: ParticipantRole,
    ) -> Result<Vec<GroupParticipant>> {
        let participants = self.get_participants(group_jid).await?;
        
        Ok(participants
            .into_iter()
            .filter(|p| p.role == role)
            .collect())
    }
    
    /// Get active participants
    pub async fn get_active_participants(&mut self, group_jid: &JID) -> Result<Vec<GroupParticipant>> {
        let participants = self.get_participants(group_jid).await?;
        
        Ok(participants
            .into_iter()
            .filter(|p| p.status == ParticipantStatus::Active)
            .collect())
    }
    
    /// Update participant stats
    pub async fn update_participant_stats(
        &mut self,
        group_jid: &JID,
        participant_jid: &JID,
        stats_update: StatsUpdate,
    ) -> Result<()> {
        if let Some(cached) = self.participant_cache.get_mut(group_jid) {
            if let Some(p) = cached.participants.iter_mut().find(|p| p.jid == *participant_jid) {
                if let Some(total) = stats_update.total_messages {
                    p.message_stats.total_messages = total;
                }
                if let Some(media) = stats_update.media_messages {
                    p.message_stats.media_messages = media;
                }
                if let Some(text) = stats_update.text_messages {
                    p.message_stats.text_messages = text;
                }
                if let Some(today) = stats_update.messages_today {
                    p.message_stats.messages_today = today;
                }
                if let Some(avg) = stats_update.avg_messages_per_day {
                    p.message_stats.avg_messages_per_day = avg;
                }
                if stats_update.update_last_message_time {
                    p.message_stats.last_message_at = Some(SystemTime::now());
                    p.last_seen = Some(SystemTime::now());
                }
                
                cached.last_sync = SystemTime::now();
                return Ok(());
            }
        }
        
        Err(Error::Protocol("Participant not found".to_string()))
    }
    
    /// Cache participants
    fn cache_participants(&mut self, group_jid: JID, participants: Vec<GroupParticipant>) {
        // Enforce cache size limit
        if self.participant_cache.len() >= self.config.max_cache_size {
            self.evict_oldest_cache_entry();
        }
        
        let cached = CachedParticipants {
            participants,
            cached_at: SystemTime::now(),
            last_sync: SystemTime::now(),
        };
        
        self.participant_cache.insert(group_jid, cached);
    }
    
    /// Check if cache entry is expired
    fn is_cache_expired(&self, cached: &CachedParticipants) -> bool {
        if let Ok(elapsed) = cached.cached_at.elapsed() {
            elapsed.as_secs() > self.config.cache_ttl
        } else {
            true
        }
    }
    
    /// Evict oldest cache entry
    fn evict_oldest_cache_entry(&mut self) {
        if let Some((oldest_key, _)) = self.participant_cache
            .iter()
            .min_by_key(|(_, cached)| cached.cached_at)
            .map(|(key, cached)| (key.clone(), cached.clone()))
        {
            self.participant_cache.remove(&oldest_key);
        }
    }
    
    /// Clear cache
    pub fn clear_cache(&mut self) {
        self.participant_cache.clear();
    }
    
    /// Validate participant JID
    fn validate_participant(&self, participant: &JID) -> Result<()> {
        if participant.user.is_empty() {
            return Err(Error::Protocol("Invalid participant JID".to_string()));
        }
        
        if !participant.server.ends_with("s.whatsapp.net") {
            return Err(Error::Protocol("Invalid participant server".to_string()));
        }
        
        Ok(())
    }
}

/// Statistics update for participant
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatsUpdate {
    pub total_messages: Option<u64>,
    pub media_messages: Option<u64>,
    pub text_messages: Option<u64>,
    pub messages_today: Option<u64>,
    pub avg_messages_per_day: Option<f64>,
    pub update_last_message_time: bool,
}

impl Default for ParticipantManager {
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
    async fn test_participant_manager_creation() {
        let manager = ParticipantManager::new();
        assert!(manager.participant_cache.is_empty());
        assert_eq!(manager.config.max_cache_size, 500);
    }
    
    #[tokio::test]
    async fn test_get_participants() {
        let mut manager = ParticipantManager::new();
        let group_jid = create_test_group_jid();
        
        let participants = manager.get_participants(&group_jid).await.unwrap();
        
        assert_eq!(participants.len(), 3); // creator + 2 members
        assert!(participants.iter().any(|p| p.role == ParticipantRole::Creator));
        assert_eq!(participants.iter().filter(|p| p.role == ParticipantRole::Member).count(), 2);
        
        // Should be cached now
        assert_eq!(manager.participant_cache.len(), 1);
    }
    
    #[tokio::test]
    async fn test_add_participants() {
        let mut manager = ParticipantManager::new();
        let group_jid = create_test_group_jid();
        let new_participant1 = create_test_jid("new_member1");
        let new_participant2 = create_test_jid("new_member2");
        let added_by = create_test_jid("admin");
        
        let result = manager.add_participants(
            &group_jid,
            vec![new_participant1.clone(), new_participant2.clone()],
            &added_by,
        ).await.unwrap();
        
        assert_eq!(result.successful.len(), 2);
        assert_eq!(result.failed.len(), 0);
        assert_eq!(result.operation, ParticipantOperationType::Add);
        
        // Verify participants were added to cache
        let participants = manager.get_participants(&group_jid).await.unwrap();
        assert!(participants.iter().any(|p| p.jid == new_participant1));
        assert!(participants.iter().any(|p| p.jid == new_participant2));
    }
    
    #[tokio::test]
    async fn test_remove_participants() {
        let mut manager = ParticipantManager::new();
        let group_jid = create_test_group_jid();
        let removed_by = create_test_jid("admin");
        
        // First get participants to have them in cache
        let initial_participants = manager.get_participants(&group_jid).await.unwrap();
        let member_to_remove = initial_participants
            .iter()
            .find(|p| p.role == ParticipantRole::Member)
            .unwrap()
            .jid
            .clone();
        
        let result = manager.remove_participants(
            &group_jid,
            vec![member_to_remove.clone()],
            &removed_by,
        ).await.unwrap();
        
        assert_eq!(result.successful.len(), 1);
        assert_eq!(result.failed.len(), 0);
        assert_eq!(result.operation, ParticipantOperationType::Remove);
        
        // Verify participant was removed from cache
        let updated_participants = manager.get_participants(&group_jid).await.unwrap();
        assert!(!updated_participants.iter().any(|p| p.jid == member_to_remove));
    }
    
    #[tokio::test]
    async fn test_promote_demote_participants() {
        let mut manager = ParticipantManager::new();
        let group_jid = create_test_group_jid();
        let promoter = create_test_jid("creator");
        
        // Get initial participants
        let initial_participants = manager.get_participants(&group_jid).await.unwrap();
        let member_to_promote = initial_participants
            .iter()
            .find(|p| p.role == ParticipantRole::Member)
            .unwrap()
            .jid
            .clone();
        
        // Promote to admin
        let promote_result = manager.promote_participants(
            &group_jid,
            vec![member_to_promote.clone()],
            &promoter,
        ).await.unwrap();
        
        assert_eq!(promote_result.successful.len(), 1);
        assert_eq!(promote_result.operation, ParticipantOperationType::Promote);
        
        // Verify promotion
        let participant = manager.get_participant(&group_jid, &member_to_promote).await.unwrap();
        assert_eq!(participant.role, ParticipantRole::Admin);
        assert!(participant.permissions.can_add_participants);
        
        // Demote back to member
        let demote_result = manager.demote_participants(
            &group_jid,
            vec![member_to_promote.clone()],
            &promoter,
        ).await.unwrap();
        
        assert_eq!(demote_result.successful.len(), 1);
        assert_eq!(demote_result.operation, ParticipantOperationType::Demote);
        
        // Verify demotion
        let participant = manager.get_participant(&group_jid, &member_to_promote).await.unwrap();
        assert_eq!(participant.role, ParticipantRole::Member);
        assert!(!participant.permissions.can_add_participants);
    }
    
    #[tokio::test]
    async fn test_update_permissions() {
        let mut manager = ParticipantManager::new();
        let group_jid = create_test_group_jid();
        
        let participants = manager.get_participants(&group_jid).await.unwrap();
        let member_jid = participants
            .iter()
            .find(|p| p.role == ParticipantRole::Member)
            .unwrap()
            .jid
            .clone();
        
        let new_permissions = ParticipantPermissions {
            can_send_messages: true,
            can_send_media: false,
            can_add_participants: false,
            can_edit_group_info: false,
            can_change_permissions: false,
            can_delete_messages: false,
            can_pin_messages: true,
        };
        
        manager.update_permissions(&group_jid, &member_jid, new_permissions.clone()).await.unwrap();
        
        let updated_participant = manager.get_participant(&group_jid, &member_jid).await.unwrap();
        assert_eq!(updated_participant.permissions, new_permissions);
    }
    
    #[tokio::test]
    async fn test_get_participants_by_role() {
        let mut manager = ParticipantManager::new();
        let group_jid = create_test_group_jid();
        
        let admins = manager.get_participants_by_role(&group_jid, ParticipantRole::Creator).await.unwrap();
        assert_eq!(admins.len(), 1);
        
        let members = manager.get_participants_by_role(&group_jid, ParticipantRole::Member).await.unwrap();
        assert_eq!(members.len(), 2);
    }
    
    #[tokio::test]
    async fn test_update_participant_stats() {
        let mut manager = ParticipantManager::new();
        let group_jid = create_test_group_jid();
        
        let participants = manager.get_participants(&group_jid).await.unwrap();
        let member_jid = participants[1].jid.clone(); // Get a member
        
        let stats_update = StatsUpdate {
            total_messages: Some(100),
            media_messages: Some(25),
            text_messages: Some(75),
            messages_today: Some(10),
            avg_messages_per_day: Some(5.5),
            update_last_message_time: true,
        };
        
        manager.update_participant_stats(&group_jid, &member_jid, stats_update).await.unwrap();
        
        let updated_participant = manager.get_participant(&group_jid, &member_jid).await.unwrap();
        assert_eq!(updated_participant.message_stats.total_messages, 100);
        assert_eq!(updated_participant.message_stats.media_messages, 25);
        assert_eq!(updated_participant.message_stats.text_messages, 75);
        assert_eq!(updated_participant.message_stats.messages_today, 10);
        assert_eq!(updated_participant.message_stats.avg_messages_per_day, 5.5);
        assert!(updated_participant.message_stats.last_message_at.is_some());
        assert!(updated_participant.last_seen.is_some());
    }
    
    #[test]
    fn test_participant_permissions() {
        let default_permissions = ParticipantPermissions::default();
        
        assert!(default_permissions.can_send_messages);
        assert!(default_permissions.can_send_media);
        assert!(!default_permissions.can_add_participants);
        assert!(!default_permissions.can_edit_group_info);
        assert!(!default_permissions.can_change_permissions);
        assert!(!default_permissions.can_delete_messages);
        assert!(!default_permissions.can_pin_messages);
    }
    
    #[test]
    fn test_message_stats() {
        let default_stats = MessageStats::default();
        
        assert_eq!(default_stats.total_messages, 0);
        assert_eq!(default_stats.media_messages, 0);
        assert_eq!(default_stats.text_messages, 0);
        assert_eq!(default_stats.messages_today, 0);
        assert_eq!(default_stats.avg_messages_per_day, 0.0);
        assert!(default_stats.last_message_at.is_none());
    }
    
    #[tokio::test]
    async fn test_cache_operations() {
        let mut manager = ParticipantManager::with_config(ParticipantManagerConfig {
            max_cache_size: 2,
            cache_ttl: 1,
            auto_sync: false,
            batch_size: 10,
        });
        
        let group1 = create_test_group_jid();
        let group2 = JID::new("2234567890".to_string(), "g.us".to_string());
        let group3 = JID::new("3234567890".to_string(), "g.us".to_string());
        
        // Fill cache
        manager.get_participants(&group1).await.unwrap();
        manager.get_participants(&group2).await.unwrap();
        assert_eq!(manager.participant_cache.len(), 2);
        
        // Adding third should evict oldest
        manager.get_participants(&group3).await.unwrap();
        assert_eq!(manager.participant_cache.len(), 2);
        
        // Clear cache
        manager.clear_cache();
        assert!(manager.participant_cache.is_empty());
    }
}