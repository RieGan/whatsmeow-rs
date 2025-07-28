/// Group types and structures for WhatsApp group management

use crate::{
    error::{Error, Result},
    types::JID,
};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// WhatsApp group information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupInfo {
    /// Group JID (ends with @g.us)
    pub jid: JID,
    /// Group name/subject
    pub name: String,
    /// Group description
    pub description: Option<String>,
    /// List of group participants
    pub participants: Vec<JID>,
    /// List of group administrators
    pub admins: Vec<JID>,
    /// Group creator
    pub creator: JID,
    /// Group creation timestamp
    pub created_at: SystemTime,
    /// Group settings and permissions
    pub settings: GroupSettings,
    /// Group invite link (if available)
    pub invite_link: Option<String>,
}

impl GroupInfo {
    /// Create new group info
    pub fn new(
        jid: JID,
        name: String,
        creator: JID,
        participants: Vec<JID>,
    ) -> Self {
        let admins = vec![creator.clone()];
        
        Self {
            jid,
            name,
            description: None,
            participants,
            admins,
            creator,
            created_at: SystemTime::now(),
            settings: GroupSettings::default(),
            invite_link: None,
        }
    }
    
    /// Check if a JID is a participant
    pub fn is_participant(&self, jid: &JID) -> bool {
        self.participants.contains(jid)
    }
    
    /// Check if a JID is an admin
    pub fn is_admin(&self, jid: &JID) -> bool {
        self.admins.contains(jid)
    }
    
    /// Check if a JID is the creator
    pub fn is_creator(&self, jid: &JID) -> bool {
        self.creator == *jid
    }
    
    /// Get participant count
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }
    
    /// Get admin count
    pub fn admin_count(&self) -> usize {
        self.admins.len()
    }
    
    /// Validate group info
    pub fn validate(&self) -> Result<()> {
        // JID must be a group JID
        if !self.jid.server.ends_with("g.us") {
            return Err(Error::Protocol("Invalid group JID".to_string()));
        }
        
        // Name cannot be empty
        if self.name.trim().is_empty() {
            return Err(Error::Protocol("Group name cannot be empty".to_string()));
        }
        
        // Name length limit (WhatsApp limit is 25 characters)
        if self.name.len() > 25 {
            return Err(Error::Protocol("Group name too long".to_string()));
        }
        
        // Description length limit (WhatsApp limit is 512 characters)
        if let Some(desc) = &self.description {
            if desc.len() > 512 {
                return Err(Error::Protocol("Group description too long".to_string()));
            }
        }
        
        // Creator must be in participants
        if !self.participants.contains(&self.creator) {
            return Err(Error::Protocol("Creator must be a participant".to_string()));
        }
        
        // Creator must be an admin
        if !self.admins.contains(&self.creator) {
            return Err(Error::Protocol("Creator must be an admin".to_string()));
        }
        
        // All admins must be participants
        for admin in &self.admins {
            if !self.participants.contains(admin) {
                return Err(Error::Protocol("All admins must be participants".to_string()));
            }
        }
        
        // Participant limit (WhatsApp supports up to 1024 participants)
        if self.participants.len() > 1024 {
            return Err(Error::Protocol("Too many participants".to_string()));
        }
        
        Ok(())
    }
}

/// Group settings and permissions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupSettings {
    /// Who can add participants
    pub add_participants: ParticipantPermission,
    /// Who can edit group info
    pub edit_group_info: ParticipantPermission,
    /// Who can send messages
    pub send_messages: ParticipantPermission,
    /// Whether group is announcement-only
    pub announcement_only: bool,
    /// Whether group history is visible to new members
    pub history_visible: bool,
    /// Whether disappearing messages are enabled
    pub disappearing_messages: Option<DisappearingMessageSettings>,
}

impl Default for GroupSettings {
    fn default() -> Self {
        Self {
            add_participants: ParticipantPermission::AdminsOnly,
            edit_group_info: ParticipantPermission::AdminsOnly,
            send_messages: ParticipantPermission::Everyone,
            announcement_only: false,
            history_visible: true,
            disappearing_messages: None,
        }
    }
}

/// Permission levels for group operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantPermission {
    /// Only administrators can perform this action
    AdminsOnly,
    /// All participants can perform this action
    Everyone,
}

/// Disappearing message settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisappearingMessageSettings {
    /// Duration in seconds after which messages disappear
    pub duration: u64,
    /// Whether setting was enabled by admin
    pub enabled_by_admin: bool,
}

impl DisappearingMessageSettings {
    /// Create new disappearing message settings
    pub fn new(duration: u64, enabled_by_admin: bool) -> Self {
        Self {
            duration,
            enabled_by_admin,
        }
    }
    
    /// Common durations
    pub fn one_day() -> Self {
        Self::new(24 * 60 * 60, true)
    }
    
    pub fn one_week() -> Self {
        Self::new(7 * 24 * 60 * 60, true)
    }
    
    pub fn ninety_days() -> Self {
        Self::new(90 * 24 * 60 * 60, true)
    }
}

/// Request to create a new group
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateGroupRequest {
    /// Group name/subject
    pub name: String,
    /// Optional group description
    pub description: Option<String>,
    /// Initial participants (creator will be added automatically)
    pub participants: Vec<JID>,
    /// Initial group settings
    pub settings: Option<GroupSettings>,
}

impl CreateGroupRequest {
    /// Create new group creation request
    pub fn new(name: String, participants: Vec<JID>) -> Self {
        Self {
            name,
            description: None,
            participants,
            settings: None,
        }
    }
    
    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
    
    /// Set settings
    pub fn with_settings(mut self, settings: GroupSettings) -> Self {
        self.settings = Some(settings);
        self
    }
    
    /// Validate the request
    pub fn validate(&self) -> Result<()> {
        // Name validation
        if self.name.trim().is_empty() {
            return Err(Error::Protocol("Group name cannot be empty".to_string()));
        }
        
        if self.name.len() > 25 {
            return Err(Error::Protocol("Group name too long".to_string()));
        }
        
        // Description validation
        if let Some(desc) = &self.description {
            if desc.len() > 512 {
                return Err(Error::Protocol("Group description too long".to_string()));
            }
        }
        
        // Participant validation
        if self.participants.is_empty() {
            return Err(Error::Protocol("At least one participant required".to_string()));
        }
        
        if self.participants.len() > 1023 {
            return Err(Error::Protocol("Too many initial participants".to_string()));
        }
        
        // Check for duplicate participants
        let mut unique_participants = std::collections::HashSet::new();
        for participant in &self.participants {
            if !unique_participants.insert(participant) {
                return Err(Error::Protocol("Duplicate participants not allowed".to_string()));
            }
        }
        
        Ok(())
    }
}

/// Group metadata update request
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupMetadataUpdate {
    /// New group name (optional)
    pub name: Option<String>,
    /// New group description (optional)
    pub description: Option<String>,
    /// Group avatar/picture data (optional)
    pub avatar: Option<Vec<u8>>,
}

impl GroupMetadataUpdate {
    /// Create empty metadata update
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            avatar: None,
        }
    }
    
    /// Set name
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }
    
    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
    
    /// Set avatar
    pub fn with_avatar(mut self, avatar: Vec<u8>) -> Self {
        self.avatar = Some(avatar);
        self
    }
    
    /// Check if update has any changes
    pub fn has_changes(&self) -> bool {
        self.name.is_some() || self.description.is_some() || self.avatar.is_some()
    }
    
    /// Validate the update
    pub fn validate(&self) -> Result<()> {
        if let Some(name) = &self.name {
            if name.trim().is_empty() {
                return Err(Error::Protocol("Group name cannot be empty".to_string()));
            }
            if name.len() > 25 {
                return Err(Error::Protocol("Group name too long".to_string()));
            }
        }
        
        if let Some(desc) = &self.description {
            if desc.len() > 512 {
                return Err(Error::Protocol("Group description too long".to_string()));
            }
        }
        
        if let Some(avatar) = &self.avatar {
            // Avatar size limit (typical WhatsApp limit is around 2MB)
            if avatar.len() > 2 * 1024 * 1024 {
                return Err(Error::Protocol("Avatar too large".to_string()));
            }
            
            // Check if it looks like image data (basic validation)
            if avatar.len() < 10 {
                return Err(Error::Protocol("Invalid avatar data".to_string()));
            }
        }
        
        Ok(())
    }
}

impl Default for GroupMetadataUpdate {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of participant operations (add/remove/promote/demote) from types module
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupParticipantOperationResult {
    /// Participants that were successfully processed
    pub successful: Vec<JID>,
    /// Participants that failed with error messages
    pub failed: Vec<(JID, String)>,
}

impl GroupParticipantOperationResult {
    /// Create new result
    pub fn new() -> Self {
        Self {
            successful: Vec::new(),
            failed: Vec::new(),
        }
    }
    
    /// Add successful participant
    pub fn add_success(&mut self, jid: JID) {
        self.successful.push(jid);
    }
    
    /// Add failed participant
    pub fn add_failure(&mut self, jid: JID, error: String) {
        self.failed.push((jid, error));
    }
    
    /// Check if all operations were successful
    pub fn all_successful(&self) -> bool {
        self.failed.is_empty()
    }
    
    /// Check if any operations were successful
    pub fn any_successful(&self) -> bool {
        !self.successful.is_empty()
    }
    
    /// Get success count
    pub fn success_count(&self) -> usize {
        self.successful.len()
    }
    
    /// Get failure count
    pub fn failure_count(&self) -> usize {
        self.failed.len()
    }
}

impl Default for GroupParticipantOperationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Group event types for notifications
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GroupEvent {
    /// Group was created
    GroupCreated {
        group_info: GroupInfo,
        by: JID,
    },
    /// Participants were added
    ParticipantsAdded {
        group_jid: JID,
        participants: Vec<JID>,
        by: JID,
    },
    /// Participants were removed
    ParticipantsRemoved {
        group_jid: JID,
        participants: Vec<JID>,
        by: JID,
    },
    /// Participants were promoted to admin
    ParticipantsPromoted {
        group_jid: JID,
        participants: Vec<JID>,
        by: JID,
    },
    /// Participants were demoted from admin
    ParticipantsDemoted {
        group_jid: JID,
        participants: Vec<JID>,
        by: JID,
    },
    /// Group metadata was updated
    MetadataUpdated {
        group_jid: JID,
        old_name: Option<String>,
        new_name: Option<String>,
        old_description: Option<String>,
        new_description: Option<String>,
        by: JID,
    },
    /// Group settings were updated
    SettingsUpdated {
        group_jid: JID,
        settings: GroupSettings,
        by: JID,
    },
    /// Group invite link was created or changed
    InviteLinkUpdated {
        group_jid: JID,
        invite_link: String,
        by: JID,
    },
    /// Group invite link was revoked
    InviteLinkRevoked {
        group_jid: JID,
        by: JID,
    },
    /// Someone left the group
    ParticipantLeft {
        group_jid: JID,
        participant: JID,
    },
    /// Someone joined via invite link
    ParticipantJoinedViaInvite {
        group_jid: JID,
        participant: JID,
    },
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
    
    #[test]
    fn test_group_info_creation() {
        let group_jid = create_test_group_jid();
        let creator = create_test_jid("creator");
        let participant1 = create_test_jid("participant1");
        let participant2 = create_test_jid("participant2");
        
        let participants = vec![creator.clone(), participant1, participant2];
        
        let group_info = GroupInfo::new(
            group_jid.clone(),
            "Test Group".to_string(),
            creator.clone(),
            participants.clone(),
        );
        
        assert_eq!(group_info.jid, group_jid);
        assert_eq!(group_info.name, "Test Group");
        assert_eq!(group_info.creator, creator);
        assert_eq!(group_info.participants, participants);
        assert_eq!(group_info.admins, vec![creator.clone()]);
        assert!(group_info.is_creator(&creator));
        assert!(group_info.is_admin(&creator));
        assert_eq!(group_info.participant_count(), 3);
        assert_eq!(group_info.admin_count(), 1);
    }
    
    #[test]
    fn test_group_info_validation() {
        let group_jid = create_test_group_jid();
        let creator = create_test_jid("creator");
        let participants = vec![creator.clone()];
        
        let group_info = GroupInfo::new(
            group_jid,
            "Test Group".to_string(),
            creator,
            participants,
        );
        
        assert!(group_info.validate().is_ok());
        
        // Test invalid group JID
        let mut invalid_group = group_info.clone();
        invalid_group.jid = create_test_jid("invalid");
        assert!(invalid_group.validate().is_err());
        
        // Test empty name
        let mut invalid_group = group_info.clone();
        invalid_group.name = "".to_string();
        assert!(invalid_group.validate().is_err());
        
        // Test too long name
        let mut invalid_group = group_info.clone();
        invalid_group.name = "a".repeat(26);
        assert!(invalid_group.validate().is_err());
    }
    
    #[test]
    fn test_create_group_request() {
        let participant1 = create_test_jid("participant1");
        let participant2 = create_test_jid("participant2");
        let participants = vec![participant1, participant2];
        
        let request = CreateGroupRequest::new("Test Group".to_string(), participants.clone())
            .with_description("Test Description".to_string());
        
        assert_eq!(request.name, "Test Group");
        assert_eq!(request.description, Some("Test Description".to_string()));
        assert_eq!(request.participants, participants);
        assert!(request.validate().is_ok());
        
        // Test invalid request
        let invalid_request = CreateGroupRequest::new("".to_string(), participants);
        assert!(invalid_request.validate().is_err());
    }
    
    #[test]
    fn test_group_metadata_update() {
        let update = GroupMetadataUpdate::new()
            .with_name("New Name".to_string())
            .with_description("New Description".to_string());
        
        assert_eq!(update.name, Some("New Name".to_string()));
        assert_eq!(update.description, Some("New Description".to_string()));
        assert!(update.has_changes());
        assert!(update.validate().is_ok());
        
        // Test empty update
        let empty_update = GroupMetadataUpdate::new();
        assert!(!empty_update.has_changes());
        
        // Test invalid update
        let invalid_update = GroupMetadataUpdate::new()
            .with_name("".to_string());
        assert!(invalid_update.validate().is_err());
    }
    
    #[test]
    fn test_participant_operation_result() {
        let mut result = GroupParticipantOperationResult::new();
        let jid1 = create_test_jid("user1");
        let jid2 = create_test_jid("user2");
        
        result.add_success(jid1.clone());
        result.add_failure(jid2.clone(), "Error message".to_string());
        
        assert_eq!(result.success_count(), 1);
        assert_eq!(result.failure_count(), 1);
        assert!(!result.all_successful());
        assert!(result.any_successful());
        assert_eq!(result.successful, vec![jid1]);
        assert_eq!(result.failed, vec![(jid2, "Error message".to_string())]);
    }
    
    #[test]
    fn test_disappearing_message_settings() {
        let settings = DisappearingMessageSettings::one_day();
        assert_eq!(settings.duration, 24 * 60 * 60);
        assert!(settings.enabled_by_admin);
        
        let settings = DisappearingMessageSettings::one_week();
        assert_eq!(settings.duration, 7 * 24 * 60 * 60);
        
        let settings = DisappearingMessageSettings::ninety_days();
        assert_eq!(settings.duration, 90 * 24 * 60 * 60);
    }
    
    #[test]
    fn test_group_settings() {
        let settings = GroupSettings::default();
        assert_eq!(settings.add_participants, ParticipantPermission::AdminsOnly);
        assert_eq!(settings.edit_group_info, ParticipantPermission::AdminsOnly);
        assert_eq!(settings.send_messages, ParticipantPermission::Everyone);
        assert!(!settings.announcement_only);
        assert!(settings.history_visible);
        assert!(settings.disappearing_messages.is_none());
    }
    
    #[test]
    fn test_group_event_types() {
        let group_jid = create_test_group_jid();
        let user_jid = create_test_jid("user");
        
        let event = GroupEvent::ParticipantsAdded {
            group_jid: group_jid.clone(),
            participants: vec![user_jid.clone()],
            by: user_jid.clone(),
        };
        
        match event {
            GroupEvent::ParticipantsAdded { participants, .. } => {
                assert_eq!(participants.len(), 1);
            }
            _ => panic!("Wrong event type"),
        }
    }
}