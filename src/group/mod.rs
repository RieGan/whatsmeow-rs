/// Group management for WhatsApp - create groups, manage participants, group metadata

pub mod types;
pub mod manager;
pub mod metadata;
pub mod participants;
pub mod permissions;
pub mod community;
pub mod announcement;
pub mod disappearing;

use crate::{
    error::{Error, Result},
    types::JID,
    signal::SignalProtocolManager,
    auth::multidevice::MultiDeviceManager,
};
use std::collections::HashMap;

pub use types::{GroupInfo, GroupSettings, CreateGroupRequest, GroupMetadataUpdate, GroupEvent, ParticipantPermission, DisappearingMessageSettings};
pub use manager::{GroupManager, GroupManagerConfig};
pub use metadata::{GroupMetadataManager, GroupMetadata};
pub use participants::{ParticipantManager, GroupParticipant, ParticipantRole, ParticipantStatus, ParticipantOperationResult};
pub use permissions::{PermissionManager, GroupPermissions};
pub use community::{CommunityInfo, CommunityManager, CommunitySettings, CreateCommunityRequest, CommunityEvent, AddGroupToCommunityRequest};
pub use announcement::{AnnouncementGroupManager, AnnouncementGroupConfig, AnnouncementMessage, AnnouncementPriority, MemberAnnouncementStatus};
pub use disappearing::{GroupDisappearingManager, GroupDisappearingConfig, DisappearingTimer, DisappearingMessage, MessageContentType};

/// Group management service for WhatsApp groups
pub struct GroupService {
    /// Group manager for operations
    group_manager: GroupManager,
    /// Signal protocol manager for encryption
    signal_manager: SignalProtocolManager,
    /// Multi-device manager for device coordination
    device_manager: MultiDeviceManager,
    /// Cache of group information
    group_cache: HashMap<JID, GroupInfo>,
    /// Community manager for community groups
    community_manager: CommunityManager,
    /// Announcement group manager
    announcement_manager: AnnouncementGroupManager,
    /// Disappearing messages manager
    disappearing_manager: GroupDisappearingManager,
    /// Permission manager
    permission_manager: PermissionManager,
}

impl GroupService {
    /// Create new group service
    pub fn new(
        signal_manager: SignalProtocolManager,
        device_manager: MultiDeviceManager,
    ) -> Self {
        Self {
            group_manager: GroupManager::new(),
            signal_manager,
            device_manager,
            group_cache: HashMap::new(),
            community_manager: CommunityManager::new(),
            announcement_manager: AnnouncementGroupManager::new(),
            disappearing_manager: GroupDisappearingManager::new(),
            permission_manager: PermissionManager::new(),
        }
    }
    
    /// Create a new WhatsApp group
    pub async fn create_group(&mut self, request: CreateGroupRequest) -> Result<GroupInfo> {
        // Validate request
        request.validate()?;
        
        // Create group with manager
        let group_info = self.group_manager.create_group(request).await?;
        
        // Set up Signal group session for encryption
        self.setup_group_encryption(&group_info).await?;
        
        // Cache group info
        self.group_cache.insert(group_info.jid.clone(), group_info.clone());
        
        Ok(group_info)
    }
    
    /// Add participants to an existing group
    pub async fn add_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
    ) -> Result<ParticipantOperationResult> {
        // Get group info
        let group_info = self.get_group_info(group_jid).await?;
        
        // Check permissions
        self.check_add_permission(&group_info)?;
        
        // Add participants
        let result = self.group_manager
            .add_participants(group_jid, participants.clone())
            .await?;
        
        // Update group encryption for new participants
        for participant in &participants {
            if result.successful.contains(participant) {
                self.add_participant_to_encryption(group_jid, participant).await?;
            }
        }
        
        // Update cache
        if let Some(cached_group) = self.group_cache.get_mut(group_jid) {
            for participant in &result.successful {
                if !cached_group.participants.contains(participant) {
                    cached_group.participants.push(participant.clone());
                }
            }
        }
        
        Ok(result)
    }
    
    /// Remove participants from a group
    pub async fn remove_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
    ) -> Result<ParticipantOperationResult> {
        // Get group info
        let group_info = self.get_group_info(group_jid).await?;
        
        // Check permissions
        self.check_remove_permission(&group_info, &participants)?;
        
        // Remove participants
        let result = self.group_manager
            .remove_participants(group_jid, participants.clone())
            .await?;
        
        // Update group encryption
        for participant in &participants {
            if result.successful.contains(participant) {
                self.remove_participant_from_encryption(group_jid, participant).await?;
            }
        }
        
        // Update cache
        if let Some(cached_group) = self.group_cache.get_mut(group_jid) {
            cached_group.participants.retain(|p| !result.successful.contains(p));
        }
        
        Ok(result)
    }
    
    /// Update group metadata (name, description, etc.)
    pub async fn update_metadata(
        &mut self,
        group_jid: &JID,
        metadata: GroupMetadataUpdate,
    ) -> Result<GroupInfo> {
        // Get group info
        let group_info = self.get_group_info(group_jid).await?;
        
        // Check permissions
        self.check_metadata_permission(&group_info)?;
        
        // Update metadata
        let updated_group = self.group_manager
            .update_metadata(group_jid, metadata)
            .await?;
        
        // Update cache
        self.group_cache.insert(group_jid.clone(), updated_group.clone());
        
        Ok(updated_group)
    }
    
    /// Get group information
    pub async fn get_group_info(&mut self, group_jid: &JID) -> Result<GroupInfo> {
        // Check cache first
        if let Some(cached) = self.group_cache.get(group_jid) {
            return Ok(cached.clone());
        }
        
        // Fetch from manager
        let group_info = self.group_manager.get_group_info(group_jid).await?;
        
        // Cache result
        self.group_cache.insert(group_jid.clone(), group_info.clone());
        
        Ok(group_info)
    }
    
    /// Leave a group
    pub async fn leave_group(&mut self, group_jid: &JID) -> Result<()> {
        // Remove ourselves from the group
        let own_jid = self.device_manager.get_own_jid();
        self.group_manager.leave_group(group_jid, &own_jid).await?;
        
        // Clean up encryption
        self.cleanup_group_encryption(group_jid).await?;
        
        // Remove from cache
        self.group_cache.remove(group_jid);
        
        Ok(())
    }
    
    /// Promote participants to admin
    pub async fn promote_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
    ) -> Result<ParticipantOperationResult> {
        // Get group info
        let group_info = self.get_group_info(group_jid).await?;
        
        // Check permissions
        self.check_admin_permission(&group_info)?;
        
        // Promote participants
        let result = self.group_manager
            .promote_participants(group_jid, participants)
            .await?;
        
        // Update cache
        if let Some(cached_group) = self.group_cache.get_mut(group_jid) {
            for participant in &result.successful {
                cached_group.admins.push(participant.clone());
            }
        }
        
        Ok(result)
    }
    
    /// Demote participants from admin
    pub async fn demote_participants(
        &mut self,
        group_jid: &JID,
        participants: Vec<JID>,
    ) -> Result<ParticipantOperationResult> {
        // Get group info
        let group_info = self.get_group_info(group_jid).await?;
        
        // Check permissions
        self.check_admin_permission(&group_info)?;
        
        // Demote participants
        let result = self.group_manager
            .demote_participants(group_jid, participants)
            .await?;
        
        // Update cache
        if let Some(cached_group) = self.group_cache.get_mut(group_jid) {
            cached_group.admins.retain(|p| !result.successful.contains(p));
        }
        
        Ok(result)
    }
    
    /// Update group settings (permissions, etc.)
    pub async fn update_settings(
        &mut self,
        group_jid: &JID,
        settings: GroupSettings,
    ) -> Result<GroupInfo> {
        // Get group info
        let group_info = self.get_group_info(group_jid).await?;
        
        // Check permissions
        self.check_admin_permission(&group_info)?;
        
        // Update settings
        let updated_group = self.group_manager
            .update_settings(group_jid, settings)
            .await?;
        
        // Update cache
        self.group_cache.insert(group_jid.clone(), updated_group.clone());
        
        Ok(updated_group)
    }
    
    /// Get group invite link
    pub async fn get_invite_link(&mut self, group_jid: &JID) -> Result<String> {
        // Get group info
        let group_info = self.get_group_info(group_jid).await?;
        
        // Check permissions
        self.check_admin_permission(&group_info)?;
        
        // Get invite link
        let invite_link = self.group_manager.get_invite_link(group_jid).await?;
        
        Ok(invite_link)
    }
    
    /// Revoke group invite link
    pub async fn revoke_invite_link(&mut self, group_jid: &JID) -> Result<String> {
        // Get group info
        let group_info = self.get_group_info(group_jid).await?;
        
        // Check permissions
        self.check_admin_permission(&group_info)?;
        
        // Revoke and get new link
        let new_link = self.group_manager.revoke_invite_link(group_jid).await?;
        
        Ok(new_link)
    }
    
    /// Join group via invite link
    pub async fn join_via_invite(&mut self, invite_link: &str) -> Result<GroupInfo> {
        // Parse invite link
        let group_jid = self.group_manager.parse_invite_link(invite_link)?;
        
        // Join group
        let group_info = self.group_manager.join_via_invite(invite_link).await?;
        
        // Set up encryption for new group
        self.setup_group_encryption(&group_info).await?;
        
        // Cache group info
        self.group_cache.insert(group_jid, group_info.clone());
        
        Ok(group_info)
    }
    
    /// Clear group cache
    pub fn clear_cache(&mut self) {
        self.group_cache.clear();
    }
    
    /// Get cached groups
    pub fn get_cached_groups(&self) -> Vec<&GroupInfo> {
        self.group_cache.values().collect()
    }
    
    // ========== PHASE 4: ADVANCED GROUP FEATURES ==========
    
    // ===== Community Groups =====
    
    /// Create a new community
    pub async fn create_community(
        &mut self,
        request: CreateCommunityRequest,
    ) -> Result<CommunityInfo> {
        let creator = self.device_manager.get_own_jid();
        let community_info = self.community_manager
            .create_community(request, creator)
            .await?;
        
        tracing::info!("Created community: {}", community_info.name);
        
        Ok(community_info)
    }
    
    /// Add a group to a community
    pub async fn add_group_to_community(
        &mut self,
        community_jid: &JID,
        group_jid: &JID,
        merge_members: bool,
    ) -> Result<()> {
        // Get group info for member merging
        let group_info = self.get_group_info(group_jid).await?;
        
        let request = AddGroupToCommunityRequest::new(community_jid.clone(), group_jid.clone())
            .with_merge_members(merge_members);
        
        self.community_manager
            .add_group_to_community(request, &group_info)
            .await?;
        
        tracing::info!("Added group {} to community {}", group_jid, community_jid);
        
        Ok(())
    }
    
    /// Remove a group from a community
    pub async fn remove_group_from_community(
        &mut self,
        community_jid: &JID,
        group_jid: &JID,
    ) -> Result<()> {
        self.community_manager
            .remove_group_from_community(community_jid, group_jid)
            .await?;
        
        tracing::info!("Removed group {} from community {}", group_jid, community_jid);
        
        Ok(())
    }
    
    /// Get community information
    pub fn get_community(&self, community_jid: &JID) -> Option<&CommunityInfo> {
        self.community_manager.get_community(community_jid)
    }
    
    /// Get all communities
    pub fn get_all_communities(&self) -> Vec<&CommunityInfo> {
        self.community_manager.get_all_communities()
    }
    
    /// Find community for a group
    pub fn find_community_for_group(&self, group_jid: &JID) -> Option<&JID> {
        self.community_manager.find_community_for_group(group_jid)
    }
    
    // ===== Announcement Groups =====
    
    /// Configure group as announcement group
    pub fn configure_announcement_group(
        &mut self,
        group_jid: &JID,
        config: AnnouncementGroupConfig,
    ) -> Result<()> {
        self.announcement_manager
            .configure_announcement_group(group_jid.clone(), config)?;
        
        // Update group settings
        if let Some(cached_group) = self.group_cache.get_mut(group_jid) {
            AnnouncementGroupManager::convert_to_announcement_group(&mut cached_group.settings);
        }
        
        tracing::info!("Configured announcement group: {}", group_jid);
        
        Ok(())
    }
    
    /// Post announcement to group
    pub async fn post_announcement(
        &mut self,
        group_jid: &JID,
        announcement: AnnouncementMessage,
    ) -> Result<String> {
        let sender = self.device_manager.get_own_jid();
        let group_info = self.get_group_info(group_jid).await?;
        
        let announcement_id = self.announcement_manager
            .post_announcement(group_jid, announcement, &sender, &group_info)?;
        
        tracing::info!("Posted announcement {} to group {}", announcement_id, group_jid);
        
        Ok(announcement_id)
    }
    
    /// Pin an announcement
    pub async fn pin_announcement(
        &mut self,
        group_jid: &JID,
        announcement_id: &str,
    ) -> Result<()> {
        let sender = self.device_manager.get_own_jid();
        let group_info = self.get_group_info(group_jid).await?;
        
        self.announcement_manager
            .pin_announcement(group_jid, announcement_id, &sender, &group_info)?;
        
        tracing::info!("Pinned announcement {} in group {}", announcement_id, group_jid);
        
        Ok(())
    }
    
    /// Get announcements for group
    pub fn get_announcements(&self, group_jid: &JID) -> Option<&Vec<AnnouncementMessage>> {
        self.announcement_manager.get_announcements(group_jid)
    }
    
    /// Get pinned announcements
    pub fn get_pinned_announcements(&self, group_jid: &JID) -> Vec<&AnnouncementMessage> {
        self.announcement_manager.get_pinned_announcements(group_jid)
    }
    
    /// Mark announcement as read
    pub fn mark_announcement_read(
        &mut self,
        group_jid: &JID,
        announcement_id: &str,
    ) -> Result<()> {
        let member = self.device_manager.get_own_jid();
        self.announcement_manager
            .mark_announcement_read(group_jid, announcement_id, &member)
    }
    
    // ===== Disappearing Messages =====
    
    /// Enable disappearing messages for a group
    pub async fn enable_disappearing_messages(
        &mut self,
        group_jid: &JID,
        timer: DisappearingTimer,
    ) -> Result<()> {
        let enabled_by = self.device_manager.get_own_jid();
        let group_info = self.get_group_info(group_jid).await?;
        
        self.disappearing_manager
            .enable_disappearing_messages(group_jid, timer.clone(), enabled_by, &group_info)?;
        
        // Update cached group settings
        if let Some(cached_group) = self.group_cache.get_mut(group_jid) {
            let config = self.disappearing_manager.get_config(group_jid).unwrap();
            GroupDisappearingManager::apply_to_group_settings(config, &mut cached_group.settings);
        }
        
        tracing::info!("Enabled disappearing messages for group: {}", group_jid);
        
        Ok(())
    }
    
    /// Disable disappearing messages for a group
    pub async fn disable_disappearing_messages(
        &mut self,
        group_jid: &JID,
    ) -> Result<()> {
        let disabled_by = self.device_manager.get_own_jid();
        let group_info = self.get_group_info(group_jid).await?;
        
        self.disappearing_manager
            .disable_disappearing_messages(group_jid, disabled_by, &group_info)?;
        
        // Update cached group settings
        if let Some(cached_group) = self.group_cache.get_mut(group_jid) {
            cached_group.settings.disappearing_messages = None;
        }
        
        tracing::info!("Disabled disappearing messages for group: {}", group_jid);
        
        Ok(())
    }
    
    /// Schedule a message for disappearing
    pub fn schedule_disappearing_message(
        &mut self,
        message_id: String,
        group_jid: &JID,
        content_type: MessageContentType,
    ) -> Result<()> {
        let sender = self.device_manager.get_own_jid();
        self.disappearing_manager
            .schedule_message(message_id, group_jid, sender, content_type)
    }
    
    /// Process all disappearing messages (should be called periodically)
    pub async fn process_disappearing_messages(&mut self) -> Result<Vec<(JID, String)>> {
        self.disappearing_manager.process_disappearing_messages().await
    }
    
    /// Check if disappearing messages are enabled for a group
    pub fn are_disappearing_messages_enabled(&self, group_jid: &JID) -> bool {
        self.disappearing_manager.is_enabled(group_jid)
    }
    
    // ===== Advanced Permissions =====
    
    /// Check if a participant has a specific permission
    pub async fn has_permission(
        &mut self,
        group_jid: &JID,
        participant_jid: &JID,
        permission: &str,
        role: ParticipantRole,
    ) -> Result<bool> {
        self.permission_manager
            .has_permission(group_jid, participant_jid, permission, role)
            .await
    }
    
    /// Apply permission template to group
    pub async fn apply_permission_template(
        &mut self,
        group_jid: &JID,
        template_id: &str,
    ) -> Result<GroupPermissions> {
        let permissions = self.permission_manager
            .apply_template(group_jid, template_id)
            .await?;
        
        tracing::info!("Applied permission template {} to group {}", template_id, group_jid);
        
        Ok(permissions)
    }
    
    /// Get permissions for a group
    pub async fn get_group_permissions(&mut self, group_jid: &JID) -> Result<GroupPermissions> {
        self.permission_manager.get_permissions(group_jid).await
    }
    
    /// Get available permission templates
    pub fn get_permission_templates(&self) -> &HashMap<String, permissions::PermissionTemplate> {
        self.permission_manager.get_templates()
    }
    
    // Permission checking methods
    
    fn check_add_permission(&self, group_info: &GroupInfo) -> Result<()> {
        let own_jid = self.device_manager.get_own_jid();
        
        match group_info.settings.add_participants {
            ParticipantPermission::Everyone => Ok(()),
            ParticipantPermission::AdminsOnly => {
                if group_info.admins.contains(&own_jid) {
                    Ok(())
                } else {
                    Err(Error::Protocol("Only admins can add participants".to_string()))
                }
            }
        }
    }
    
    fn check_remove_permission(&self, group_info: &GroupInfo, participants: &[JID]) -> Result<()> {
        let own_jid = self.device_manager.get_own_jid();
        
        // Check if we're admin
        if !group_info.admins.contains(&own_jid) {
            return Err(Error::Protocol("Only admins can remove participants".to_string()));
        }
        
        // Check if trying to remove other admins
        for participant in participants {
            if group_info.admins.contains(participant) && participant != &own_jid {
                return Err(Error::Protocol("Cannot remove other admins".to_string()));
            }
        }
        
        Ok(())
    }
    
    fn check_metadata_permission(&self, group_info: &GroupInfo) -> Result<()> {
        let own_jid = self.device_manager.get_own_jid();
        
        match group_info.settings.edit_group_info {
            ParticipantPermission::Everyone => Ok(()),
            ParticipantPermission::AdminsOnly => {
                if group_info.admins.contains(&own_jid) {
                    Ok(())
                } else {
                    Err(Error::Protocol("Only admins can edit group info".to_string()))
                }
            }
        }
    }
    
    fn check_admin_permission(&self, group_info: &GroupInfo) -> Result<()> {
        let own_jid = self.device_manager.get_own_jid();
        
        if group_info.admins.contains(&own_jid) {
            Ok(())
        } else {
            Err(Error::Protocol("Admin privileges required".to_string()))
        }
    }
    
    // Encryption management methods
    
    async fn setup_group_encryption(&mut self, group_info: &GroupInfo) -> Result<()> {
        // Set up Signal group session for the group
        // This would involve creating sender keys and distributing them
        tracing::info!("Setting up group encryption for {}", group_info.jid);
        
        // TODO: Implement actual Signal group session setup
        // This would involve:
        // 1. Creating a new sender key for the group
        // 2. Distributing the sender key to all participants
        // 3. Setting up group session state
        
        Ok(())
    }
    
    async fn add_participant_to_encryption(&mut self, group_jid: &JID, participant: &JID) -> Result<()> {
        tracing::info!("Adding {} to group encryption for {}", participant, group_jid);
        
        // TODO: Implement adding participant to group encryption
        // This would involve sending the current sender key to the new participant
        
        Ok(())
    }
    
    async fn remove_participant_from_encryption(&mut self, group_jid: &JID, participant: &JID) -> Result<()> {
        tracing::info!("Removing {} from group encryption for {}", participant, group_jid);
        
        // TODO: Implement removing participant from group encryption
        // This might involve rotating the sender key for security
        
        Ok(())
    }
    
    async fn cleanup_group_encryption(&mut self, group_jid: &JID) -> Result<()> {
        tracing::info!("Cleaning up group encryption for {}", group_jid);
        
        // TODO: Implement cleanup of group encryption state
        // This would remove all sender key state for the group
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_signal_manager() -> SignalProtocolManager {
        use crate::signal::{identity::MemoryIdentityKeyStore, session::MemorySessionStore, 
                           group::MemoryGroupSessionStore, prekey::MemoryPreKeyStore};
        
        SignalProtocolManager::new_with_stores(
            Box::new(MemoryIdentityKeyStore::new(12345)), // test registration ID
            Box::new(MemorySessionStore::new()),
            Box::new(MemoryPreKeyStore::new()),
            Box::new(MemoryGroupSessionStore::new()),
        )
    }
    
    fn create_test_device_manager() -> MultiDeviceManager {
        let account_jid = JID::new("test".to_string(), "s.whatsapp.net".to_string());
        
        // Create a minimal device registration for testing
        let device_registration = crate::auth::pairing::DeviceRegistration {
            jid: account_jid.clone(),
            device_id: 1,
            registration_id: 12345,
            device_info: crate::auth::pairing::DeviceInfo::default(),
            keys: crate::auth::pairing::PairingKeysData {
                noise_public_key: vec![0u8; 32],
                noise_private_key: vec![0u8; 32],
                identity_public_key: vec![0u8; 32],
                identity_private_key: vec![0u8; 32],
                static_public_key: vec![0u8; 32],
                static_private_key: vec![0u8; 32],
                registration_id: 12345,
            },
            pre_key_bundle: crate::auth::pairing::PreKeyBundleData {
                identity_key: vec![0u8; 32],
                signed_pre_key_id: 1,
                signed_pre_key: vec![0u8; 32],
                signed_pre_key_signature: vec![0u8; 64],
                pre_key_id: Some(1),
                pre_key: Some(vec![0u8; 32]),
                registration_id: 12345,
                device_id: 1,
            },
            server_token: "test_token".to_string(),
            business_name: None,
            platform: "test".to_string(),
            registered_at: std::time::SystemTime::now(),
            adv_secret: vec![0u8; 32],
        };
        
        MultiDeviceManager::new(account_jid, device_registration)
    }
    
    #[tokio::test]
    async fn test_group_service_creation() {
        let signal_manager = create_test_signal_manager();
        let device_manager = create_test_device_manager();
        
        let group_service = GroupService::new(signal_manager, device_manager);
        assert!(group_service.group_cache.is_empty());
    }
    
    #[tokio::test]
    async fn test_cache_operations() {
        let signal_manager = create_test_signal_manager();
        let device_manager = create_test_device_manager();
        
        let mut group_service = GroupService::new(signal_manager, device_manager);
        
        // Initially empty
        assert!(group_service.get_cached_groups().is_empty());
        
        // Clear empty cache
        group_service.clear_cache();
        assert!(group_service.get_cached_groups().is_empty());
    }
    
    #[test]
    fn test_permission_checking() {
        let signal_manager = create_test_signal_manager();
        let device_manager = create_test_device_manager();
        let group_service = GroupService::new(signal_manager, device_manager);
        
        let own_jid = group_service.device_manager.get_own_jid();
        
        // Create test group info
        let group_info = GroupInfo {
            jid: JID::new("group".to_string(), "g.us".to_string()),
            name: "Test Group".to_string(),
            description: None,
            participants: vec![own_jid.clone()],
            admins: vec![own_jid.clone()],
            creator: own_jid.clone(),
            created_at: std::time::SystemTime::now(),
            settings: GroupSettings::default(),
            invite_link: None,
        };
        
        // Admin should have all permissions
        assert!(group_service.check_admin_permission(&group_info).is_ok());
        assert!(group_service.check_metadata_permission(&group_info).is_ok());
        assert!(group_service.check_add_permission(&group_info).is_ok());
        assert!(group_service.check_remove_permission(&group_info, &[]).is_ok());
    }
}