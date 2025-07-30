/// WhatsApp Community Groups implementation
/// 
/// Communities are a collection of linked groups with shared administration
/// and member management. This module implements the community-specific
/// functionality for WhatsApp Business and advanced group features.

use crate::{
    error::{Error, Result},
    types::JID,
    group::{GroupInfo, GroupSettings, ParticipantPermission},
};
use serde::{Deserialize, Serialize};
use std::{time::SystemTime, collections::HashMap};

/// WhatsApp Community information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommunityInfo {
    /// Community JID (ends with @g.us but has special community marker)
    pub jid: JID,
    /// Community name
    pub name: String,
    /// Community description
    pub description: Option<String>,
    /// Community creator
    pub creator: JID,
    /// Community administrators
    pub admins: Vec<JID>,
    /// Community creation timestamp
    pub created_at: SystemTime,
    /// Community settings
    pub settings: CommunitySettings,
    /// Community invite link
    pub invite_link: Option<String>,
    /// Community icon/avatar
    pub avatar: Option<Vec<u8>>,
    /// Groups that are part of this community
    pub linked_groups: Vec<JID>,
    /// Community membership (includes all members from linked groups)
    pub members: Vec<JID>,
    /// Community announcement group (optional)
    pub announcement_group: Option<JID>,
}

impl CommunityInfo {
    /// Create new community info
    pub fn new(
        jid: JID,
        name: String,
        creator: JID,
        description: Option<String>,
    ) -> Self {
        Self {
            jid,
            name,
            description,
            creator: creator.clone(),
            admins: vec![creator],
            created_at: SystemTime::now(),
            settings: CommunitySettings::default(),
            invite_link: None,
            avatar: None,
            linked_groups: Vec::new(),
            members: Vec::new(),
            announcement_group: None,
        }
    }
    
    /// Check if a JID is a community admin
    pub fn is_admin(&self, jid: &JID) -> bool {
        self.admins.contains(jid)
    }
    
    /// Check if a JID is the community creator
    pub fn is_creator(&self, jid: &JID) -> bool {
        self.creator == *jid
    }
    
    /// Check if a JID is a community member
    pub fn is_member(&self, jid: &JID) -> bool {
        self.members.contains(jid)
    }
    
    /// Get total member count across all linked groups
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
    
    /// Get linked group count
    pub fn group_count(&self) -> usize {
        self.linked_groups.len()
    }
    
    /// Add a group to the community
    pub fn add_group(&mut self, group_jid: JID) {
        if !self.linked_groups.contains(&group_jid) {
            self.linked_groups.push(group_jid);
        }
    }
    
    /// Remove a group from the community
    pub fn remove_group(&mut self, group_jid: &JID) {
        self.linked_groups.retain(|g| g != group_jid);
    }
    
    /// Check if a group is linked to this community
    pub fn has_group(&self, group_jid: &JID) -> bool {
        self.linked_groups.contains(group_jid)
    }
    
    /// Validate community info
    pub fn validate(&self) -> Result<()> {
        // JID must be a group JID (communities use group infrastructure)
        if !self.jid.server.ends_with("g.us") {
            return Err(Error::Protocol("Invalid community JID".to_string()));
        }
        
        // Name cannot be empty
        if self.name.trim().is_empty() {
            return Err(Error::Protocol("Community name cannot be empty".to_string()));
        }
        
        // Name length limit (similar to groups)
        if self.name.len() > 50 {
            return Err(Error::Protocol("Community name too long".to_string()));
        }
        
        // Description length limit
        if let Some(desc) = &self.description {
            if desc.len() > 1024 {
                return Err(Error::Protocol("Community description too long".to_string()));
            }
        }
        
        // Creator must be an admin
        if !self.admins.contains(&self.creator) {
            return Err(Error::Protocol("Creator must be an admin".to_string()));
        }
        
        // Group limit (WhatsApp communities can have up to 50 groups)
        if self.linked_groups.len() > 50 {
            return Err(Error::Protocol("Too many linked groups".to_string()));
        }
        
        // Member limit (communities can have thousands of members)
        if self.members.len() > 5000 {
            return Err(Error::Protocol("Too many community members".to_string()));
        }
        
        Ok(())
    }
}

/// Community-specific settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommunitySettings {
    /// Who can add groups to the community
    pub add_groups: ParticipantPermission,
    /// Who can remove groups from community
    pub remove_groups: ParticipantPermission,
    /// Who can edit community info
    pub edit_community_info: ParticipantPermission,
    /// Who can add members to community
    pub add_members: ParticipantPermission,
    /// Whether community has an announcement group
    pub has_announcement_group: bool,
    /// Whether community history is visible to new members
    pub history_visible: bool,
    /// Whether members can discover the community via search
    pub discoverable: bool,
    /// Whether approval is required for new members
    pub approval_required: bool,
}

impl Default for CommunitySettings {
    fn default() -> Self {
        Self {
            add_groups: ParticipantPermission::AdminsOnly,
            remove_groups: ParticipantPermission::AdminsOnly,
            edit_community_info: ParticipantPermission::AdminsOnly,
            add_members: ParticipantPermission::AdminsOnly,
            has_announcement_group: true,
            history_visible: false,
            discoverable: false,
            approval_required: true,
        }
    }
}

/// Request to create a new community
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateCommunityRequest {
    /// Community name
    pub name: String,
    /// Optional community description
    pub description: Option<String>,
    /// Initial community settings
    pub settings: Option<CommunitySettings>,
    /// Community avatar/icon
    pub avatar: Option<Vec<u8>>,
    /// Whether to create an announcement group
    pub create_announcement_group: bool,
}

impl CreateCommunityRequest {
    /// Create new community creation request
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            settings: None,
            avatar: None,
            create_announcement_group: true,
        }
    }
    
    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
    
    /// Set settings
    pub fn with_settings(mut self, settings: CommunitySettings) -> Self {
        self.settings = Some(settings);
        self
    }
    
    /// Set avatar
    pub fn with_avatar(mut self, avatar: Vec<u8>) -> Self {
        self.avatar = Some(avatar);
        self
    }
    
    /// Set whether to create announcement group
    pub fn with_announcement_group(mut self, create: bool) -> Self {
        self.create_announcement_group = create;
        self
    }
    
    /// Validate the request
    pub fn validate(&self) -> Result<()> {
        // Name validation
        if self.name.trim().is_empty() {
            return Err(Error::Protocol("Community name cannot be empty".to_string()));
        }
        
        if self.name.len() > 50 {
            return Err(Error::Protocol("Community name too long".to_string()));
        }
        
        // Description validation
        if let Some(desc) = &self.description {
            if desc.len() > 1024 {
                return Err(Error::Protocol("Community description too long".to_string()));
            }
        }
        
        // Avatar validation
        if let Some(avatar) = &self.avatar {
            if avatar.len() > 5 * 1024 * 1024 {  // 5MB limit for communities
                return Err(Error::Protocol("Community avatar too large".to_string()));
            }
            
            if avatar.len() < 10 {
                return Err(Error::Protocol("Invalid avatar data".to_string()));
            }
        }
        
        Ok(())
    }
}

/// Community metadata update request
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommunityMetadataUpdate {
    /// New community name (optional)
    pub name: Option<String>,
    /// New community description (optional)
    pub description: Option<String>,
    /// Community avatar/icon data (optional)
    pub avatar: Option<Vec<u8>>,
}

impl CommunityMetadataUpdate {
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
                return Err(Error::Protocol("Community name cannot be empty".to_string()));
            }
            if name.len() > 50 {
                return Err(Error::Protocol("Community name too long".to_string()));
            }
        }
        
        if let Some(desc) = &self.description {
            if desc.len() > 1024 {
                return Err(Error::Protocol("Community description too long".to_string()));
            }
        }
        
        if let Some(avatar) = &self.avatar {
            if avatar.len() > 5 * 1024 * 1024 {
                return Err(Error::Protocol("Community avatar too large".to_string()));
            }
            
            if avatar.len() < 10 {
                return Err(Error::Protocol("Invalid avatar data".to_string()));
            }
        }
        
        Ok(())
    }
}

impl Default for CommunityMetadataUpdate {
    fn default() -> Self {
        Self::new()
    }
}

/// Request to add a group to a community
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddGroupToCommunityRequest {
    /// Community JID
    pub community_jid: JID,
    /// Group JID to add
    pub group_jid: JID,
    /// Whether to merge group members into community
    pub merge_members: bool,
}

impl AddGroupToCommunityRequest {
    /// Create new add group request
    pub fn new(community_jid: JID, group_jid: JID) -> Self {
        Self {
            community_jid,
            group_jid,
            merge_members: true,
        }
    }
    
    /// Set whether to merge members
    pub fn with_merge_members(mut self, merge: bool) -> Self {
        self.merge_members = merge;
        self
    }
    
    /// Validate the request
    pub fn validate(&self) -> Result<()> {
        // Both JIDs must be group JIDs
        if !self.community_jid.server.ends_with("g.us") {
            return Err(Error::Protocol("Invalid community JID".to_string()));
        }
        
        if !self.group_jid.server.ends_with("g.us") {
            return Err(Error::Protocol("Invalid group JID".to_string()));
        }
        
        // Cannot add community to itself
        if self.community_jid == self.group_jid {
            return Err(Error::Protocol("Cannot add community to itself".to_string()));
        }
        
        Ok(())
    }
}

/// Community event types for notifications
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommunityEvent {
    /// Community was created
    CommunityCreated {
        community_info: CommunityInfo,
        by: JID,
    },
    /// Group was added to community
    GroupAddedToCommunity {
        community_jid: JID,
        group_jid: JID,
        by: JID,
    },
    /// Group was removed from community
    GroupRemovedFromCommunity {
        community_jid: JID,
        group_jid: JID,
        by: JID,
    },
    /// Community metadata was updated
    CommunityMetadataUpdated {
        community_jid: JID,
        old_name: Option<String>,
        new_name: Option<String>,
        old_description: Option<String>,
        new_description: Option<String>,
        by: JID,
    },
    /// Community settings were updated
    CommunitySettingsUpdated {
        community_jid: JID,
        settings: CommunitySettings,
        by: JID,
    },
    /// Member was added to community
    MemberAddedToCommunity {
        community_jid: JID,
        member: JID,
        by: JID,
    },
    /// Member was removed from community
    MemberRemovedFromCommunity {
        community_jid: JID,
        member: JID,
        by: JID,
    },
    /// Admin was promoted in community
    AdminPromotedInCommunity {
        community_jid: JID,
        admin: JID,
        by: JID,
    },
    /// Admin was demoted in community
    AdminDemotedInCommunity {
        community_jid: JID,
        admin: JID,
        by: JID,
    },
    /// Community invite link was updated
    CommunityInviteLinkUpdated {
        community_jid: JID,
        invite_link: String,
        by: JID,
    },
    /// Community invite link was revoked
    CommunityInviteLinkRevoked {
        community_jid: JID,
        by: JID,
    },
    /// Someone joined community via invite
    MemberJoinedCommunityViaInvite {
        community_jid: JID,
        member: JID,
    },
    /// Announcement was posted to community
    CommunityAnnouncement {
        community_jid: JID,
        announcement_group: JID,
        message_id: String,
        by: JID,
    },
}

/// Community manager for handling community operations
#[derive(Debug)]
pub struct CommunityManager {
    /// Cache of community information
    communities: HashMap<JID, CommunityInfo>,
    /// Mapping of groups to their parent communities
    group_to_community: HashMap<JID, JID>,
}

impl CommunityManager {
    /// Create new community manager
    pub fn new() -> Self {
        Self {
            communities: HashMap::new(),
            group_to_community: HashMap::new(),
        }
    }
    
    /// Create a new community
    pub async fn create_community(
        &mut self,
        request: CreateCommunityRequest,
        creator: JID,
    ) -> Result<CommunityInfo> {
        // Validate request
        request.validate()?;
        
        // Generate community JID (would normally be from server)
        let community_jid = self.generate_community_jid();
        
        // Create community info
        let mut community_info = CommunityInfo::new(
            community_jid.clone(),
            request.name,
            creator.clone(),
            request.description,
        );
        
        // Apply custom settings if provided
        if let Some(settings) = request.settings {
            community_info.settings = settings;
        }
        
        // Set avatar if provided
        if let Some(avatar) = request.avatar {
            community_info.avatar = Some(avatar);
        }
        
        // Add creator as member
        community_info.members.push(creator);
        
        // Validate final community info
        community_info.validate()?;
        
        // Store community
        self.communities.insert(community_jid.clone(), community_info.clone());
        
        tracing::info!("Created community: {}", community_info.name);
        
        Ok(community_info)
    }
    
    /// Add a group to a community
    pub async fn add_group_to_community(
        &mut self,
        request: AddGroupToCommunityRequest,
        group_info: &GroupInfo,
    ) -> Result<()> {
        // Validate request
        request.validate()?;
        
        // Get community
        let community = self.communities.get_mut(&request.community_jid)
            .ok_or_else(|| Error::Protocol("Community not found".to_string()))?;
        
        // Check if group is already in a community
        if self.group_to_community.contains_key(&request.group_jid) {
            return Err(Error::Protocol("Group already belongs to a community".to_string()));
        }
        
        // Check group limit
        if community.linked_groups.len() >= 50 {
            return Err(Error::Protocol("Community group limit reached".to_string()));
        }
        
        // Add group to community
        community.add_group(request.group_jid.clone());
        
        // Update group-to-community mapping
        self.group_to_community.insert(request.group_jid.clone(), request.community_jid.clone());
        
        // Merge members if requested
        if request.merge_members {
            for participant in &group_info.participants {
                if !community.members.contains(participant) {
                    community.members.push(participant.clone());
                }
            }
        }
        
        tracing::info!("Added group {} to community {}", request.group_jid, request.community_jid);
        
        Ok(())
    }
    
    /// Remove a group from a community
    pub async fn remove_group_from_community(
        &mut self,
        community_jid: &JID,
        group_jid: &JID,
    ) -> Result<()> {
        // Get community
        let community = self.communities.get_mut(community_jid)
            .ok_or_else(|| Error::Protocol("Community not found".to_string()))?;
        
        // Remove group from community
        community.remove_group(group_jid);
        
        // Update mapping
        self.group_to_community.remove(group_jid);
        
        tracing::info!("Removed group {} from community {}", group_jid, community_jid);
        
        Ok(())
    }
    
    /// Update community metadata
    pub async fn update_community_metadata(
        &mut self,
        community_jid: &JID,
        update: CommunityMetadataUpdate,
    ) -> Result<CommunityInfo> {
        // Validate update
        update.validate()?;
        
        // Get community
        let community = self.communities.get_mut(community_jid)
            .ok_or_else(|| Error::Protocol("Community not found".to_string()))?;
        
        // Apply updates
        if let Some(name) = update.name {
            community.name = name;
        }
        
        if let Some(description) = update.description {
            community.description = Some(description);
        }
        
        if let Some(avatar) = update.avatar {
            community.avatar = Some(avatar);
        }
        
        // Validate updated community
        community.validate()?;
        
        tracing::info!("Updated metadata for community {}", community_jid);
        
        Ok(community.clone())
    }
    
    /// Get community information
    pub fn get_community(&self, community_jid: &JID) -> Option<&CommunityInfo> {
        self.communities.get(community_jid)
    }
    
    /// Get all communities
    pub fn get_all_communities(&self) -> Vec<&CommunityInfo> {
        self.communities.values().collect()
    }
    
    /// Find community for a group
    pub fn find_community_for_group(&self, group_jid: &JID) -> Option<&JID> {
        self.group_to_community.get(group_jid)
    }
    
    /// Check if a group belongs to a community
    pub fn is_group_in_community(&self, group_jid: &JID) -> bool {
        self.group_to_community.contains_key(group_jid)
    }
    
    /// Generate a community JID (simplified - normally from server)
    fn generate_community_jid(&self) -> JID {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let community_id = format!("community_{}", timestamp);
        JID::new(community_id, "g.us".to_string())
    }
}

impl Default for CommunityManager {
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
    
    fn create_test_community_jid() -> JID {
        JID::new("community_123".to_string(), "g.us".to_string())
    }
    
    fn create_test_group_jid() -> JID {
        JID::new("group_456".to_string(), "g.us".to_string())
    }
    
    #[test]
    fn test_community_info_creation() {
        let community_jid = create_test_community_jid();
        let creator = create_test_jid("creator");
        
        let community_info = CommunityInfo::new(
            community_jid.clone(),
            "Test Community".to_string(),
            creator.clone(),
            Some("A test community".to_string()),
        );
        
        assert_eq!(community_info.jid, community_jid);
        assert_eq!(community_info.name, "Test Community");
        assert_eq!(community_info.creator, creator);
        assert_eq!(community_info.description, Some("A test community".to_string()));
        assert!(community_info.is_creator(&creator));
        assert!(community_info.is_admin(&creator));
        assert_eq!(community_info.member_count(), 0);
        assert_eq!(community_info.group_count(), 0);
    }
    
    #[test]
    fn test_community_info_validation() {
        let community_jid = create_test_community_jid();
        let creator = create_test_jid("creator");
        
        let community_info = CommunityInfo::new(
            community_jid,
            "Test Community".to_string(),
            creator,
            None,
        );
        
        assert!(community_info.validate().is_ok());
        
        // Test invalid community name
        let mut invalid_community = community_info.clone();
        invalid_community.name = "".to_string();
        assert!(invalid_community.validate().is_err());
        
        // Test too long name
        let mut invalid_community = community_info.clone();
        invalid_community.name = "a".repeat(51);
        assert!(invalid_community.validate().is_err());
    }
    
    #[test]
    fn test_create_community_request() {
        let request = CreateCommunityRequest::new("Test Community".to_string())
            .with_description("Test Description".to_string())
            .with_announcement_group(true);
        
        assert_eq!(request.name, "Test Community");
        assert_eq!(request.description, Some("Test Description".to_string()));
        assert!(request.create_announcement_group);
        assert!(request.validate().is_ok());
        
        // Test invalid request
        let invalid_request = CreateCommunityRequest::new("".to_string());
        assert!(invalid_request.validate().is_err());
    }
    
    #[test]
    fn test_add_group_to_community_request() {
        let community_jid = create_test_community_jid();
        let group_jid = create_test_group_jid();
        
        let request = AddGroupToCommunityRequest::new(community_jid.clone(), group_jid.clone())
            .with_merge_members(true);
        
        assert_eq!(request.community_jid, community_jid);
        assert_eq!(request.group_jid, group_jid);
        assert!(request.merge_members);
        assert!(request.validate().is_ok());
        
        // Test invalid request (same JID)
        let invalid_request = AddGroupToCommunityRequest::new(community_jid.clone(), community_jid);
        assert!(invalid_request.validate().is_err());
    }
    
    #[tokio::test]
    async fn test_community_manager() {
        let mut manager = CommunityManager::new();
        let creator = create_test_jid("creator");
        
        // Create community
        let request = CreateCommunityRequest::new("Test Community".to_string());
        let community_info = manager.create_community(request, creator.clone()).await.unwrap();
        
        assert_eq!(community_info.name, "Test Community");
        assert_eq!(community_info.creator, creator);
        
        // Check if community exists
        assert!(manager.get_community(&community_info.jid).is_some());
        assert_eq!(manager.get_all_communities().len(), 1);
    }
    
    #[test]
    fn test_community_metadata_update() {
        let update = CommunityMetadataUpdate::new()
            .with_name("New Name".to_string())
            .with_description("New Description".to_string());
        
        assert_eq!(update.name, Some("New Name".to_string()));
        assert_eq!(update.description, Some("New Description".to_string()));
        assert!(update.has_changes());
        assert!(update.validate().is_ok());
        
        // Test empty update
        let empty_update = CommunityMetadataUpdate::new();
        assert!(!empty_update.has_changes());
    }
    
    #[test]
    fn test_community_group_operations() {
        let community_jid = create_test_community_jid();
        let group_jid = create_test_group_jid();
        let creator = create_test_jid("creator");
        
        let mut community_info = CommunityInfo::new(
            community_jid,
            "Test Community".to_string(),
            creator,
            None,
        );
        
        // Add group
        community_info.add_group(group_jid.clone());
        assert!(community_info.has_group(&group_jid));
        assert_eq!(community_info.group_count(), 1);
        
        // Remove group
        community_info.remove_group(&group_jid);
        assert!(!community_info.has_group(&group_jid));
        assert_eq!(community_info.group_count(), 0);
    }
}