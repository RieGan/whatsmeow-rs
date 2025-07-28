/// Group manager for WhatsApp group operations

use crate::{
    error::{Error, Result},
    types::JID,
    group::{
        GroupInfo, GroupSettings, CreateGroupRequest, GroupMetadataUpdate,
        GroupEvent,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;
use uuid::Uuid;

// Import from participants module
use crate::group::participants::ParticipantOperationResult;

/// Configuration for group manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupManagerConfig {
    /// Maximum number of participants per group
    pub max_participants: usize,
    /// Whether to enable automatic group backups
    pub enable_backups: bool,
    /// Group operation timeout in seconds
    pub operation_timeout: u64,
    /// Whether to validate participant phone numbers
    pub validate_participants: bool,
}

impl Default for GroupManagerConfig {
    fn default() -> Self {
        Self {
            max_participants: 1024, // WhatsApp limit
            enable_backups: true,
            operation_timeout: 30,
            validate_participants: true,
        }
    }
}

/// Group manager handles core group operations
pub struct GroupManager {
    /// Configuration
    config: GroupManagerConfig,
    /// Event handlers for group events
    event_handlers: Vec<Box<dyn Fn(&GroupEvent) + Send + Sync>>,
    /// Group operation history
    operation_history: Vec<GroupOperation>,
}

/// Group operation record for history/audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupOperation {
    /// Operation ID
    pub id: String,
    /// Operation type
    pub operation_type: GroupOperationType,
    /// Group JID
    pub group_jid: JID,
    /// User who performed the operation
    pub performed_by: JID,
    /// Operation timestamp
    pub timestamp: SystemTime,
    /// Operation result
    pub result: OperationResult,
    /// Additional context
    pub context: HashMap<String, String>,
}

/// Types of group operations
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GroupOperationType {
    CreateGroup,
    AddParticipants,
    RemoveParticipants,
    PromoteParticipants,
    DemoteParticipants,
    UpdateMetadata,
    UpdateSettings,
    LeaveGroup,
    GetInviteLink,
    RevokeInviteLink,
    JoinViaInvite,
}

/// Operation result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OperationResult {
    Success,
    PartialSuccess,
    Failed(String),
}

impl GroupManager {
    /// Create new group manager
    pub fn new() -> Self {
        Self::with_config(GroupManagerConfig::default())
    }
    
    /// Create group manager with custom config
    pub fn with_config(config: GroupManagerConfig) -> Self {
        Self {
            config,
            event_handlers: Vec::new(),
            operation_history: Vec::new(),
        }
    }
    
    /// Add event handler
    pub fn add_event_handler<F>(&mut self, handler: F)
    where
        F: Fn(&GroupEvent) + Send + Sync + 'static,
    {
        self.event_handlers.push(Box::new(handler));
    }
    
    /// Emit group event
    fn emit_event(&self, event: &GroupEvent) {
        for handler in &self.event_handlers {
            handler(event);
        }
    }
    
    /// Record operation in history
    fn record_operation(
        &mut self,
        operation_type: GroupOperationType,
        group_jid: &JID,
        performed_by: &JID,
        result: OperationResult,
        context: HashMap<String, String>,
    ) {
        let operation = GroupOperation {
            id: Uuid::new_v4().to_string(),
            operation_type,
            group_jid: group_jid.clone(),
            performed_by: performed_by.clone(),
            timestamp: SystemTime::now(),
            result,
            context,
        };
        
        self.operation_history.push(operation);
        
        // Keep history size manageable
        if self.operation_history.len() > 1000 {
            self.operation_history.remove(0);
        }
    }
    
    /// Create a new WhatsApp group
    pub async fn create_group(&mut self, request: CreateGroupRequest) -> Result<GroupInfo> {
        // Validate request
        request.validate()?;
        
        // Check participant limit
        if request.participants.len() > self.config.max_participants - 1 {
            return Err(Error::Protocol(format!(
                "Too many participants. Maximum: {}",
                self.config.max_participants - 1
            )));
        }
        
        // Generate group JID
        let group_id = self.generate_group_id();
        let group_jid = JID::new(group_id, "g.us".to_string());
        
        // For now, we'll simulate the creator JID
        // In a real implementation, this would come from the authenticated session
        let creator_jid = JID::new("creator".to_string(), "s.whatsapp.net".to_string());
        
        // Create participants list including creator
        let mut participants = vec![creator_jid.clone()];
        participants.extend(request.participants.clone());
        
        // Remove duplicates
        participants.sort();
        participants.dedup();
        
        // Create group info
        let group_info = GroupInfo {
            jid: group_jid.clone(),
            name: request.name.clone(),
            description: request.description.clone(),
            participants,
            admins: vec![creator_jid.clone()],
            creator: creator_jid.clone(),
            created_at: SystemTime::now(),
            settings: request.settings.unwrap_or_default(),
            invite_link: None,
        };
        
        // Validate final group info
        group_info.validate()?;
        
        // Record operation
        let mut context = HashMap::new();
        context.insert("group_name".to_string(), request.name.clone());
        context.insert("participant_count".to_string(), group_info.participants.len().to_string());
        
        self.record_operation(
            GroupOperationType::CreateGroup,
            &group_jid,
            &creator_jid,
            OperationResult::Success,
            context,
        );
        
        // Emit event
        let event = GroupEvent::GroupCreated {
            group_info: group_info.clone(),
            by: creator_jid,
        };
        self.emit_event(&event);
        
        tracing::info!("Created group {} with {} participants", group_info.name, group_info.participants.len());
        
        Ok(group_info)
    }
    
    /// Add participants to a group
    pub async fn add_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
    ) -> Result<ParticipantOperationResult> {
        let mut result = ParticipantOperationResult::new();
        
        // Simulate current user
        let current_user = JID::new("current_user".to_string(), "s.whatsapp.net".to_string());
        
        // Validate each participant
        for participant in participants {
            if self.config.validate_participants {
                if let Err(e) = self.validate_participant(&participant) {
                    result.add_failure(participant, e.to_string());
                    continue;
                }
            }
            
            // In a real implementation, this would send the actual WhatsApp protocol messages
            // For now, we'll simulate success
            result.add_success(participant);
        }
        
        // Record operation
        let operation_result = if result.all_successful() {
            OperationResult::Success
        } else if result.any_successful() {
            OperationResult::PartialSuccess
        } else {
            OperationResult::Failed("All participants failed to be added".to_string())
        };
        
        let mut context = HashMap::new();
        context.insert("successful_count".to_string(), result.success_count().to_string());
        context.insert("failed_count".to_string(), result.failure_count().to_string());
        
        self.record_operation(
            GroupOperationType::AddParticipants,
            group_jid,
            &current_user,
            operation_result,
            context,
        );
        
        // Emit event if any were successful
        if result.any_successful() {
            let event = GroupEvent::ParticipantsAdded {
                group_jid: group_jid.clone(),
                participants: result.successful.clone(),
                by: current_user,
            };
            self.emit_event(&event);
        }
        
        tracing::info!(
            "Added participants to group {}: {} successful, {} failed",
            group_jid,
            result.success_count(),
            result.failure_count()
        );
        
        Ok(result)
    }
    
    /// Remove participants from a group
    pub async fn remove_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
    ) -> Result<ParticipantOperationResult> {
        let mut result = ParticipantOperationResult::new();
        let current_user = JID::new("current_user".to_string(), "s.whatsapp.net".to_string());
        
        // Simulate removal (in real implementation, would send protocol messages)
        for participant in participants {
            // For simulation, assume all removals succeed
            result.add_success(participant);
        }
        
        // Record operation
        let operation_result = if result.all_successful() {
            OperationResult::Success
        } else if result.any_successful() {
            OperationResult::PartialSuccess
        } else {
            OperationResult::Failed("All participants failed to be removed".to_string())
        };
        
        let mut context = HashMap::new();
        context.insert("successful_count".to_string(), result.success_count().to_string());
        context.insert("failed_count".to_string(), result.failure_count().to_string());
        
        self.record_operation(
            GroupOperationType::RemoveParticipants,
            group_jid,
            &current_user,
            operation_result,
            context,
        );
        
        // Emit event
        if result.any_successful() {
            let event = GroupEvent::ParticipantsRemoved {
                group_jid: group_jid.clone(),
                participants: result.successful.clone(),
                by: current_user,
            };
            self.emit_event(&event);
        }
        
        tracing::info!(
            "Removed participants from group {}: {} successful, {} failed",
            group_jid,
            result.success_count(),
            result.failure_count()
        );
        
        Ok(result)
    }
    
    /// Promote participants to admin
    pub async fn promote_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
    ) -> Result<ParticipantOperationResult> {
        let mut result = ParticipantOperationResult::new();
        let current_user = JID::new("current_user".to_string(), "s.whatsapp.net".to_string());
        
        // Simulate promotion
        for participant in participants {
            result.add_success(participant);
        }
        
        // Record operation
        self.record_operation(
            GroupOperationType::PromoteParticipants,
            group_jid,
            &current_user,
            OperationResult::Success,
            HashMap::new(),
        );
        
        // Emit event
        if result.any_successful() {
            let event = GroupEvent::ParticipantsPromoted {
                group_jid: group_jid.clone(),
                participants: result.successful.clone(),
                by: current_user,
            };
            self.emit_event(&event);
        }
        
        Ok(result)
    }
    
    /// Demote participants from admin
    pub async fn demote_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
    ) -> Result<ParticipantOperationResult> {
        let mut result = ParticipantOperationResult::new();
        let current_user = JID::new("current_user".to_string(), "s.whatsapp.net".to_string());
        
        // Simulate demotion
        for participant in participants {
            result.add_success(participant);
        }
        
        // Record operation
        self.record_operation(
            GroupOperationType::DemoteParticipants,
            group_jid,
            &current_user,
            OperationResult::Success,
            HashMap::new(),
        );
        
        // Emit event
        if result.any_successful() {
            let event = GroupEvent::ParticipantsDemoted {
                group_jid: group_jid.clone(),
                participants: result.successful.clone(),
                by: current_user,
            };
            self.emit_event(&event);
        }
        
        Ok(result)
    }
    
    /// Update group metadata
    pub async fn update_metadata(
        &mut self,
        group_jid: &JID,
        metadata: GroupMetadataUpdate,
    ) -> Result<GroupInfo> {
        // Validate metadata
        metadata.validate()?;
        
        let current_user = JID::new("current_user".to_string(), "s.whatsapp.net".to_string());
        
        // Create a placeholder group info for the response
        // In a real implementation, this would fetch the current group info and update it
        let updated_group = GroupInfo {
            jid: group_jid.clone(),
            name: metadata.name.clone().unwrap_or_else(|| "Updated Group".to_string()),
            description: metadata.description.clone(),
            participants: vec![current_user.clone()],
            admins: vec![current_user.clone()],
            creator: current_user.clone(),
            created_at: SystemTime::now(),
            settings: GroupSettings::default(),
            invite_link: None,
        };
        
        // Record operation
        let mut context = HashMap::new();
        if let Some(name) = &metadata.name {
            context.insert("new_name".to_string(), name.clone());
        }
        if let Some(desc) = &metadata.description {
            context.insert("new_description".to_string(), desc.clone());
        }
        
        self.record_operation(
            GroupOperationType::UpdateMetadata,
            group_jid,
            &current_user,
            OperationResult::Success,
            context,
        );
        
        // Emit event
        let event = GroupEvent::MetadataUpdated {
            group_jid: group_jid.clone(),
            old_name: None, // Would come from current group info
            new_name: metadata.name.clone(),
            old_description: None,
            new_description: metadata.description.clone(),
            by: current_user,
        };
        self.emit_event(&event);
        
        tracing::info!("Updated metadata for group {}", group_jid);
        
        Ok(updated_group)
    }
    
    /// Update group settings
    pub async fn update_settings(
        &mut self,
        group_jid: &JID,
        settings: GroupSettings,
    ) -> Result<GroupInfo> {
        let current_user = JID::new("current_user".to_string(), "s.whatsapp.net".to_string());
        
        // Create updated group info
        let updated_group = GroupInfo {
            jid: group_jid.clone(),
            name: "Group".to_string(),
            description: None,
            participants: vec![current_user.clone()],
            admins: vec![current_user.clone()],
            creator: current_user.clone(),
            created_at: SystemTime::now(),
            settings: settings.clone(),
            invite_link: None,
        };
        
        // Record operation
        self.record_operation(
            GroupOperationType::UpdateSettings,
            group_jid,
            &current_user,
            OperationResult::Success,
            HashMap::new(),
        );
        
        // Emit event
        let event = GroupEvent::SettingsUpdated {
            group_jid: group_jid.clone(),
            settings,
            by: current_user,
        };
        self.emit_event(&event);
        
        tracing::info!("Updated settings for group {}", group_jid);
        
        Ok(updated_group)
    }
    
    /// Get group information
    pub async fn get_group_info(&self, group_jid: &JID) -> Result<GroupInfo> {
        // In a real implementation, this would fetch from WhatsApp servers
        // For now, return a placeholder
        let current_user = JID::new("current_user".to_string(), "s.whatsapp.net".to_string());
        
        let group_info = GroupInfo {
            jid: group_jid.clone(),
            name: "Sample Group".to_string(),
            description: Some("A sample group for testing".to_string()),
            participants: vec![current_user.clone()],
            admins: vec![current_user.clone()],
            creator: current_user,
            created_at: SystemTime::now(),
            settings: GroupSettings::default(),
            invite_link: None,
        };
        
        Ok(group_info)
    }
    
    /// Leave a group
    pub async fn leave_group(&mut self, group_jid: &JID, user_jid: &JID) -> Result<()> {
        // Record operation
        self.record_operation(
            GroupOperationType::LeaveGroup,
            group_jid,
            user_jid,
            OperationResult::Success,
            HashMap::new(),
        );
        
        // Emit event
        let event = GroupEvent::ParticipantLeft {
            group_jid: group_jid.clone(),
            participant: user_jid.clone(),
        };
        self.emit_event(&event);
        
        tracing::info!("User {} left group {}", user_jid, group_jid);
        
        Ok(())
    }
    
    /// Get group invite link
    pub async fn get_invite_link(&mut self, group_jid: &JID) -> Result<String> {
        let current_user = JID::new("current_user".to_string(), "s.whatsapp.net".to_string());
        
        // Generate a sample invite link
        let invite_code = Uuid::new_v4().simple().to_string()[0..16].to_string();
        let invite_link = format!("https://chat.whatsapp.com/{}", invite_code);
        
        // Record operation
        let mut context = HashMap::new();
        context.insert("invite_link".to_string(), invite_link.clone());
        
        self.record_operation(
            GroupOperationType::GetInviteLink,
            group_jid,
            &current_user,
            OperationResult::Success,
            context,
        );
        
        // Emit event
        let event = GroupEvent::InviteLinkUpdated {
            group_jid: group_jid.clone(),
            invite_link: invite_link.clone(),
            by: current_user,
        };
        self.emit_event(&event);
        
        tracing::info!("Generated invite link for group {}", group_jid);
        
        Ok(invite_link)
    }
    
    /// Revoke group invite link
    pub async fn revoke_invite_link(&mut self, group_jid: &JID) -> Result<String> {
        let current_user = JID::new("current_user".to_string(), "s.whatsapp.net".to_string());
        
        // Generate new invite link
        let new_invite_code = Uuid::new_v4().simple().to_string()[0..16].to_string();
        let new_invite_link = format!("https://chat.whatsapp.com/{}", new_invite_code);
        
        // Record operation
        self.record_operation(
            GroupOperationType::RevokeInviteLink,
            group_jid,
            &current_user,
            OperationResult::Success,
            HashMap::new(),
        );
        
        // Emit revoke event
        let revoke_event = GroupEvent::InviteLinkRevoked {
            group_jid: group_jid.clone(),
            by: current_user.clone(),
        };
        self.emit_event(&revoke_event);
        
        // Emit new link event
        let new_link_event = GroupEvent::InviteLinkUpdated {
            group_jid: group_jid.clone(),
            invite_link: new_invite_link.clone(),
            by: current_user,
        };
        self.emit_event(&new_link_event);
        
        tracing::info!("Revoked and regenerated invite link for group {}", group_jid);
        
        Ok(new_invite_link)
    }
    
    /// Parse invite link to extract group JID
    pub fn parse_invite_link(&self, invite_link: &str) -> Result<JID> {
        if !invite_link.starts_with("https://chat.whatsapp.com/") {
            return Err(Error::Protocol("Invalid invite link format".to_string()));
        }
        
        let invite_code = invite_link.strip_prefix("https://chat.whatsapp.com/")
            .ok_or_else(|| Error::Protocol("Invalid invite link".to_string()))?;
        
        if invite_code.len() < 10 {
            return Err(Error::Protocol("Invalid invite code".to_string()));
        }
        
        // Generate a group JID based on the invite code
        // In reality, this would involve resolving the invite code with WhatsApp servers
        let group_id = format!("group_{}", invite_code);
        let group_jid = JID::new(group_id, "g.us".to_string());
        
        Ok(group_jid)
    }
    
    /// Join group via invite link
    pub async fn join_via_invite(&mut self, invite_link: &str) -> Result<GroupInfo> {
        let group_jid = self.parse_invite_link(invite_link)?;
        let current_user = JID::new("current_user".to_string(), "s.whatsapp.net".to_string());
        
        // Create group info for joined group
        let group_info = GroupInfo {
            jid: group_jid.clone(),
            name: "Joined Group".to_string(),
            description: Some("Joined via invite link".to_string()),
            participants: vec![current_user.clone()],
            admins: vec![], // We're not admin when joining
            creator: JID::new("creator".to_string(), "s.whatsapp.net".to_string()),
            created_at: SystemTime::now(),
            settings: GroupSettings::default(),
            invite_link: Some(invite_link.to_string()),
        };
        
        // Record operation
        let mut context = HashMap::new();
        context.insert("invite_link".to_string(), invite_link.to_string());
        
        self.record_operation(
            GroupOperationType::JoinViaInvite,
            &group_jid,
            &current_user,
            OperationResult::Success,
            context,
        );
        
        // Emit event
        let event = GroupEvent::ParticipantJoinedViaInvite {
            group_jid: group_jid.clone(),
            participant: current_user,
        };
        self.emit_event(&event);
        
        tracing::info!("Joined group {} via invite link", group_jid);
        
        Ok(group_info)
    }
    
    /// Get operation history
    pub fn get_operation_history(&self) -> &[GroupOperation] {
        &self.operation_history
    }
    
    /// Clear operation history
    pub fn clear_operation_history(&mut self) {
        self.operation_history.clear();
    }
    
    /// Get operation statistics
    pub fn get_operation_stats(&self) -> HashMap<GroupOperationType, usize> {
        let mut stats = HashMap::new();
        
        for operation in &self.operation_history {
            *stats.entry(operation.operation_type.clone()).or_insert(0) += 1;
        }
        
        stats
    }
    
    // Helper methods
    
    fn generate_group_id(&self) -> String {
        // Generate a unique group ID
        // In reality, this would be assigned by WhatsApp servers
        format!("{}{}", 
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            rand::random::<u32>() % 1000000
        )
    }
    
    fn validate_participant(&self, participant: &JID) -> Result<()> {
        // Basic validation
        if participant.user.is_empty() {
            return Err(Error::Protocol("Invalid participant JID".to_string()));
        }
        
        if !participant.server.ends_with("s.whatsapp.net") {
            return Err(Error::Protocol("Invalid participant server".to_string()));
        }
        
        Ok(())
    }
}

impl Default for GroupManager {
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
    async fn test_group_manager_creation() {
        let manager = GroupManager::new();
        assert_eq!(manager.config.max_participants, 1024);
        assert!(manager.event_handlers.is_empty());
        assert!(manager.operation_history.is_empty());
    }
    
    #[tokio::test]
    async fn test_create_group() {
        let mut manager = GroupManager::new();
        let participant1 = create_test_jid("participant1");
        let participant2 = create_test_jid("participant2");
        
        let request = CreateGroupRequest::new(
            "Test Group".to_string(),
            vec![participant1.clone(), participant2.clone()],
        );
        
        let group_info = manager.create_group(request).await.unwrap();
        
        assert_eq!(group_info.name, "Test Group");
        assert!(group_info.participants.len() >= 2); // Creator + participants
        assert_eq!(group_info.admin_count(), 1);
        assert!(group_info.jid.server.ends_with("g.us"));
        
        // Check operation was recorded
        assert_eq!(manager.operation_history.len(), 1);
        assert_eq!(manager.operation_history[0].operation_type, GroupOperationType::CreateGroup);
    }
    
    #[tokio::test]
    async fn test_add_participants() {
        let mut manager = GroupManager::new();
        let group_jid = create_test_group_jid();
        let participant1 = create_test_jid("participant1");
        let participant2 = create_test_jid("participant2");
        
        let result = manager.add_participants(
            &group_jid,
            vec![participant1.clone(), participant2.clone()],
        ).await.unwrap();
        
        assert_eq!(result.success_count(), 2);
        assert_eq!(result.failure_count(), 0);
        assert!(result.all_successful());
        
        // Check operation was recorded
        assert_eq!(manager.operation_history.len(), 1);
        assert_eq!(manager.operation_history[0].operation_type, GroupOperationType::AddParticipants);
    }
    
    #[tokio::test]
    async fn test_remove_participants() {
        let mut manager = GroupManager::new();
        let group_jid = create_test_group_jid();
        let participant = create_test_jid("participant");
        
        let result = manager.remove_participants(
            &group_jid,
            vec![participant.clone()],
        ).await.unwrap();
        
        assert_eq!(result.success_count(), 1);
        assert_eq!(result.failure_count(), 0);
        
        // Check operation was recorded
        assert_eq!(manager.operation_history.len(), 1);
        assert_eq!(manager.operation_history[0].operation_type, GroupOperationType::RemoveParticipants);
    }
    
    #[tokio::test]
    async fn test_update_metadata() {
        let mut manager = GroupManager::new();
        let group_jid = create_test_group_jid();
        
        let metadata = GroupMetadataUpdate::new()
            .with_name("New Group Name".to_string())
            .with_description("New description".to_string());
        
        let updated_group = manager.update_metadata(&group_jid, metadata).await.unwrap();
        
        assert_eq!(updated_group.name, "New Group Name");
        assert_eq!(updated_group.description, Some("New description".to_string()));
        
        // Check operation was recorded
        assert_eq!(manager.operation_history.len(), 1);
        assert_eq!(manager.operation_history[0].operation_type, GroupOperationType::UpdateMetadata);
    }
    
    #[tokio::test]
    async fn test_invite_link_operations() {
        let mut manager = GroupManager::new();
        let group_jid = create_test_group_jid();
        
        // Get invite link
        let invite_link = manager.get_invite_link(&group_jid).await.unwrap();
        assert!(invite_link.starts_with("https://chat.whatsapp.com/"));
        
        // Parse invite link
        let parsed_jid = manager.parse_invite_link(&invite_link).unwrap();
        assert!(parsed_jid.server.ends_with("g.us"));
        
        // Revoke invite link
        let new_link = manager.revoke_invite_link(&group_jid).await.unwrap();
        assert!(new_link.starts_with("https://chat.whatsapp.com/"));
        assert_ne!(invite_link, new_link);
        
        // Join via invite
        let group_info = manager.join_via_invite(&new_link).await.unwrap();
        assert_eq!(group_info.invite_link, Some(new_link));
        
        // Check operations were recorded
        assert_eq!(manager.operation_history.len(), 3); // get, revoke, join (parse doesn't record)
    }
    
    #[tokio::test]
    async fn test_event_handling() {
        let mut manager = GroupManager::new();
        let mut events_received: Vec<GroupEvent> = Vec::new();
        
        // Add event handler
        manager.add_event_handler(move |event| {
            // In a real test, we'd use Arc<Mutex<Vec<GroupEvent>>> or similar
            // For this test, we'll just verify the handler is called
            match event {
                GroupEvent::GroupCreated { .. } => {
                    // Event received
                }
                _ => {}
            }
        });
        
        // Create group to trigger event
        let request = CreateGroupRequest::new(
            "Test Group".to_string(),
            vec![create_test_jid("participant")],
        );
        
        let _group_info = manager.create_group(request).await.unwrap();
        
        // Event handler should have been called
        // (We can't easily verify this in the test due to closure capture limitations)
    }
    
    #[test]
    fn test_operation_statistics() {
        let mut manager = GroupManager::new();
        let group_jid = create_test_group_jid();
        let user_jid = create_test_jid("user");
        
        // Record some operations
        manager.record_operation(
            GroupOperationType::CreateGroup,
            &group_jid,
            &user_jid,
            OperationResult::Success,
            HashMap::new(),
        );
        
        manager.record_operation(
            GroupOperationType::AddParticipants,
            &group_jid,
            &user_jid,
            OperationResult::Success,
            HashMap::new(),
        );
        
        manager.record_operation(
            GroupOperationType::AddParticipants,
            &group_jid,
            &user_jid,
            OperationResult::PartialSuccess,
            HashMap::new(),
        );
        
        let stats = manager.get_operation_stats();
        assert_eq!(stats.get(&GroupOperationType::CreateGroup), Some(&1));
        assert_eq!(stats.get(&GroupOperationType::AddParticipants), Some(&2));
        
        // Clear history
        manager.clear_operation_history();
        assert!(manager.get_operation_history().is_empty());
    }
    
    #[test]
    fn test_participant_validation() {
        let manager = GroupManager::new();
        
        let valid_jid = create_test_jid("valid_user");
        assert!(manager.validate_participant(&valid_jid).is_ok());
        
        let invalid_jid = JID::new("".to_string(), "s.whatsapp.net".to_string());
        assert!(manager.validate_participant(&invalid_jid).is_err());
        
        let invalid_server_jid = JID::new("user".to_string(), "invalid.server".to_string());
        assert!(manager.validate_participant(&invalid_server_jid).is_err());
    }
}