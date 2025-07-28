/// Group management for WhatsApp - create groups, manage participants, group metadata

pub mod types;
pub mod manager;
pub mod metadata;
pub mod participants;
pub mod permissions;

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
            device_info: crate::auth::pairing::DeviceInfo::default(),
            keys: crate::auth::pairing::PairingKeysData {
                noise_public: vec![0u8; 32],
                noise_private: vec![0u8; 32],
                identity_public: vec![0u8; 32],
                identity_private: vec![0u8; 32],
                static_public: vec![0u8; 32],
                static_private: vec![0u8; 32],
                registration_id: 12345,
            },
            pre_key_bundle: crate::auth::pairing::PreKeyBundleData {
                identity_key: vec![0u8; 32],
                signed_prekey_id: 1,
                signed_prekey_public: vec![0u8; 32],
                signed_prekey_signature: vec![0u8; 64],
                prekey_id: Some(1),
                prekey_public: Some(vec![0u8; 32]),
                registration_id: 12345,
                device_id: 1,
            },
            server_token: "test_token".to_string(),
            push_token: Some("push_token".to_string()),
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