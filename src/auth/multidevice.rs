/// Multi-device session management for WhatsApp

use crate::{
    error::{Error, Result},
    types::JID,
    auth::DeviceRegistration,
    signal::{SignalProtocolManager, PreKeyBundle},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Device type in a multi-device environment
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeviceType {
    /// Primary phone device
    Primary,
    /// Companion device (web, desktop, etc.)
    Companion,
    /// Business API device
    Business,
}

/// Device status in the multi-device environment
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeviceStatus {
    /// Device is active and online
    Active,
    /// Device is inactive but registered
    Inactive,
    /// Device is temporarily disconnected
    Disconnected,
    /// Device has been revoked/removed
    Revoked,
}

/// Multi-device session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSession {
    pub device_id: u32,
    pub device_type: DeviceType,
    pub status: DeviceStatus,
    pub last_seen: u64,
    pub platform: String,
    pub app_version: String,
    pub registration_data: DeviceRegistration,
    pub signal_session_id: Option<String>,
}

impl DeviceSession {
    /// Create a new device session
    pub fn new(registration: DeviceRegistration, device_type: DeviceType) -> Self {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            device_id: registration.device_info.device_id,
            device_type,
            status: DeviceStatus::Active,
            last_seen: current_time,
            platform: registration.device_info.platform.clone(),
            app_version: registration.device_info.version.clone(),
            registration_data: registration,
            signal_session_id: None,
        }
    }
    
    /// Update last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
    
    /// Check if device is currently active
    pub fn is_active(&self) -> bool {
        self.status == DeviceStatus::Active
    }
    
    /// Mark device as disconnected
    pub fn mark_disconnected(&mut self) {
        self.status = DeviceStatus::Disconnected;
    }
    
    /// Mark device as revoked
    pub fn revoke(&mut self) {
        self.status = DeviceStatus::Revoked;
    }
    
    /// Restore device to active status
    pub fn restore(&mut self) {
        self.status = DeviceStatus::Active;
        self.update_last_seen();
    }
}

/// Multi-device account manager
pub struct MultiDeviceManager {
    /// Primary account JID
    account_jid: JID,
    /// Our device sessions (devices we own)
    own_devices: HashMap<u32, DeviceSession>,
    /// Other account's device sessions (for contacts)
    contact_devices: HashMap<String, HashMap<u32, DeviceSession>>,
    /// Signal protocol manager for E2E encryption
    signal_manager: SignalProtocolManager,
    /// Maximum number of devices allowed
    max_devices: u32,
}

impl MultiDeviceManager {
    /// Create a new multi-device manager
    pub fn new(account_jid: JID, primary_registration: DeviceRegistration) -> Self {
        let mut own_devices = HashMap::new();
        let primary_session = DeviceSession::new(primary_registration.clone(), DeviceType::Primary);
        own_devices.insert(primary_session.device_id, primary_session);
        
        let signal_manager = SignalProtocolManager::new_with_memory_stores(
            primary_registration.keys.registration_id
        );
        
        Self {
            account_jid,
            own_devices,
            contact_devices: HashMap::new(),
            signal_manager,
            max_devices: 5, // WhatsApp limit
        }
    }
    
    /// Add a companion device to our account
    pub fn add_companion_device(&mut self, registration: DeviceRegistration) -> Result<()> {
        if self.own_devices.len() >= self.max_devices as usize {
            return Err(Error::Auth("Maximum number of devices reached".to_string()));
        }
        
        if self.own_devices.contains_key(&registration.device_info.device_id) {
            return Err(Error::Auth("Device already registered".to_string()));
        }
        
        let session = DeviceSession::new(registration, DeviceType::Companion);
        self.own_devices.insert(session.device_id, session);
        
        Ok(())
    }
    
    /// Remove a device from our account
    pub fn remove_device(&mut self, device_id: u32) -> Result<()> {
        if let Some(session) = self.own_devices.get_mut(&device_id) {
            if session.device_type == DeviceType::Primary {
                return Err(Error::Auth("Cannot remove primary device".to_string()));
            }
            session.revoke();
            Ok(())
        } else {
            Err(Error::Auth("Device not found".to_string()))
        }
    }
    
    /// Get all active devices for our account
    pub fn get_active_devices(&self) -> Vec<&DeviceSession> {
        self.own_devices.values()
            .filter(|session| session.is_active())
            .collect()
    }
    
    /// Get device session by ID
    pub fn get_device(&self, device_id: u32) -> Option<&DeviceSession> {
        self.own_devices.get(&device_id)
    }
    
    /// Update device status
    pub fn update_device_status(&mut self, device_id: u32, status: DeviceStatus) -> Result<()> {
        if let Some(session) = self.own_devices.get_mut(&device_id) {
            session.status = status;
            session.update_last_seen();
            Ok(())
        } else {
            Err(Error::Auth("Device not found".to_string()))
        }
    }
    
    /// Add device information for a contact
    pub fn add_contact_device(&mut self, contact_jid: &str, registration: DeviceRegistration) {
        let session = DeviceSession::new(registration, DeviceType::Primary);
        
        self.contact_devices
            .entry(contact_jid.to_string())
            .or_insert_with(HashMap::new)
            .insert(session.device_id, session);
    }
    
    /// Get devices for a contact
    pub fn get_contact_devices(&self, contact_jid: &str) -> Option<&HashMap<u32, DeviceSession>> {
        self.contact_devices.get(contact_jid)
    }
    
    /// Initialize Signal session with a contact device
    pub fn initialize_signal_session(&mut self, contact_jid: &str, device_id: u32, prekey_bundle: PreKeyBundle) -> Result<()> {
        let session_address = format!("{}:{}", contact_jid, device_id);
        self.signal_manager.initialize_outgoing_session(&session_address, &prekey_bundle)?;
        
        // Update the device session with Signal session info
        if let Some(contact_devices) = self.contact_devices.get_mut(contact_jid) {
            if let Some(device_session) = contact_devices.get_mut(&device_id) {
                device_session.signal_session_id = Some(session_address);
            }
        }
        
        Ok(())
    }
    
    /// Encrypt message for all active devices of a contact
    pub fn encrypt_for_contact(&mut self, contact_jid: &str, plaintext: &[u8]) -> Result<HashMap<u32, Vec<u8>>> {
        let mut encrypted_messages = HashMap::new();
        
        if let Some(contact_devices) = self.contact_devices.get(contact_jid) {
            for (device_id, device_session) in contact_devices {
                if device_session.is_active() {
                    let session_address = format!("{}:{}", contact_jid, device_id);
                    
                    if self.signal_manager.has_session(&session_address) {
                        let encrypted = self.signal_manager.encrypt_message(&session_address, plaintext)?;
                        encrypted_messages.insert(*device_id, encrypted.serialized);
                    }
                }
            }
        }
        
        if encrypted_messages.is_empty() {
            return Err(Error::Protocol("No active sessions found for contact".to_string()));
        }
        
        Ok(encrypted_messages)
    }
    
    /// Decrypt message from a contact device
    pub fn decrypt_from_contact(&mut self, contact_jid: &str, device_id: u32, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let session_address = format!("{}:{}", contact_jid, device_id);
        
        if !self.signal_manager.has_session(&session_address) {
            return Err(Error::Protocol("No session found for contact device".to_string()));
        }
        
        use crate::signal::{SignalMessage, SignalMessageType};
        let signal_message = SignalMessage {
            message_type: SignalMessageType::WhisperMessage,
            serialized: ciphertext.to_vec(),
        };
        
        let plaintext = self.signal_manager.decrypt_message(&session_address, &signal_message)?;
        
        // Update last seen for the contact device
        if let Some(contact_devices) = self.contact_devices.get_mut(contact_jid) {
            if let Some(device_session) = contact_devices.get_mut(&device_id) {
                device_session.update_last_seen();
            }
        }
        
        Ok(plaintext)
    }
    
    /// Sync device list with server
    pub fn sync_device_list(&mut self) -> Result<Vec<u32>> {
        // In a real implementation, this would query the server for the current device list
        // and update our local state accordingly
        
        let active_device_ids: Vec<u32> = self.get_active_devices()
            .iter()
            .map(|session| session.device_id)
            .collect();
        
        Ok(active_device_ids)
    }
    
    /// Generate device announcement for other devices
    pub fn generate_device_announcement(&self, device_id: u32) -> Result<DeviceAnnouncement> {
        if let Some(session) = self.get_device(device_id) {
            Ok(DeviceAnnouncement {
                device_id: session.device_id,
                platform: session.platform.clone(),
                app_version: session.app_version.clone(),
                capabilities: session.registration_data.device_info.capabilities.clone(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            })
        } else {
            Err(Error::Auth("Device not found".to_string()))
        }
    }
    
    /// Handle device announcement from another device
    pub fn handle_device_announcement(&mut self, contact_jid: &str, announcement: DeviceAnnouncement) {
        // Update or add device information based on announcement
        if let Some(contact_devices) = self.contact_devices.get_mut(contact_jid) {
            if let Some(device_session) = contact_devices.get_mut(&announcement.device_id) {
                device_session.platform = announcement.platform;
                device_session.app_version = announcement.app_version;
                device_session.last_seen = announcement.timestamp;
                device_session.registration_data.device_info.capabilities = announcement.capabilities;
            }
        }
    }
    
    /// Export multi-device configuration
    pub fn export_config(&self) -> Result<String> {
        let config = MultiDeviceConfig {
            account_jid: self.account_jid.clone(),
            own_devices: self.own_devices.clone(),
            max_devices: self.max_devices,
        };
        
        serde_json::to_string_pretty(&config)
            .map_err(|e| Error::Protocol(format!("Failed to serialize config: {}", e)))
    }
    
    /// Import multi-device configuration
    pub fn import_config(&mut self, config_data: &str) -> Result<()> {
        let config: MultiDeviceConfig = serde_json::from_str(config_data)
            .map_err(|e| Error::Protocol(format!("Failed to deserialize config: {}", e)))?;
        
        self.account_jid = config.account_jid;
        self.own_devices = config.own_devices;
        self.max_devices = config.max_devices;
        
        Ok(())
    }
    
    /// Get account JID
    pub fn account_jid(&self) -> &JID {
        &self.account_jid
    }
    
    /// Get own JID (alias for account_jid for backward compatibility)
    pub fn get_own_jid(&self) -> JID {
        self.account_jid.clone()
    }
    
    /// Get Signal protocol manager
    pub fn signal_manager(&mut self) -> &mut SignalProtocolManager {
        &mut self.signal_manager
    }
}

/// Device announcement message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAnnouncement {
    pub device_id: u32,
    pub platform: String,
    pub app_version: String,
    pub capabilities: crate::auth::DeviceCapabilities,
    pub timestamp: u64,
}

/// Multi-device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiDeviceConfig {
    pub account_jid: JID,
    pub own_devices: HashMap<u32, DeviceSession>,
    pub max_devices: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{DeviceInfo, PairingKeysData, PreKeyBundleData};
    
    fn create_test_registration(device_id: u32) -> DeviceRegistration {
        let mut device_info = DeviceInfo::default();
        device_info.device_id = device_id;
        
        DeviceRegistration {
            jid: JID::new("test".to_string(), "s.whatsapp.net".to_string()),
            device_id,
            registration_id: 12345,
            device_info,
            keys: PairingKeysData {
                noise_public_key: vec![1; 32],
                noise_private_key: vec![2; 32],
                identity_public_key: vec![3; 32],
                identity_private_key: vec![4; 32],
                static_public_key: vec![5; 32],
                static_private_key: vec![6; 32],
                registration_id: 12345,
            },
            pre_key_bundle: PreKeyBundleData {
                identity_key: vec![7; 32],
                signed_pre_key_id: 1,
                signed_pre_key: vec![8; 32],
                signed_pre_key_signature: vec![9; 64],
                pre_key_id: Some(2),
                pre_key: Some(vec![10; 32]),
                registration_id: 12345,
                device_id,
            },
            server_token: "test_token".to_string(),
            business_name: None,
            platform: "test".to_string(),
            registered_at: std::time::SystemTime::now(),
            adv_secret: vec![0u8; 32],
        }
    }
    
    #[test]
    fn test_device_session_creation() {
        let registration = create_test_registration(1);
        let session = DeviceSession::new(registration, DeviceType::Primary);
        
        assert_eq!(session.device_id, 1);
        assert_eq!(session.device_type, DeviceType::Primary);
        assert_eq!(session.status, DeviceStatus::Active);
        assert!(session.is_active());
    }
    
    #[test]
    fn test_device_session_status_changes() {
        let registration = create_test_registration(1);
        let mut session = DeviceSession::new(registration, DeviceType::Companion);
        
        session.mark_disconnected();
        assert_eq!(session.status, DeviceStatus::Disconnected);
        assert!(!session.is_active());
        
        session.revoke();
        assert_eq!(session.status, DeviceStatus::Revoked);
        
        session.restore();
        assert_eq!(session.status, DeviceStatus::Active);
        assert!(session.is_active());
    }
    
    #[test]
    fn test_multi_device_manager_creation() {
        let account_jid = JID::new("user".to_string(), "s.whatsapp.net".to_string());
        let primary_registration = create_test_registration(0);
        
        let manager = MultiDeviceManager::new(account_jid.clone(), primary_registration);
        
        assert_eq!(manager.account_jid, account_jid);
        assert_eq!(manager.own_devices.len(), 1);
        assert!(manager.get_device(0).is_some());
    }
    
    #[test]
    fn test_add_companion_device() {
        let account_jid = JID::new("user".to_string(), "s.whatsapp.net".to_string());
        let primary_registration = create_test_registration(0);
        let mut manager = MultiDeviceManager::new(account_jid, primary_registration);
        
        let companion_registration = create_test_registration(1);
        manager.add_companion_device(companion_registration).unwrap();
        
        assert_eq!(manager.own_devices.len(), 2);
        assert!(manager.get_device(1).is_some());
        
        let active_devices = manager.get_active_devices();
        assert_eq!(active_devices.len(), 2);
    }
    
    #[test]
    fn test_remove_device() {
        let account_jid = JID::new("user".to_string(), "s.whatsapp.net".to_string());
        let primary_registration = create_test_registration(0);
        let mut manager = MultiDeviceManager::new(account_jid, primary_registration);
        
        let companion_registration = create_test_registration(1);
        manager.add_companion_device(companion_registration).unwrap();
        
        // Cannot remove primary device
        assert!(manager.remove_device(0).is_err());
        
        // Can remove companion device
        manager.remove_device(1).unwrap();
        let device = manager.get_device(1).unwrap();
        assert_eq!(device.status, DeviceStatus::Revoked);
        
        let active_devices = manager.get_active_devices();
        assert_eq!(active_devices.len(), 1);
    }
    
    #[test]
    fn test_contact_devices() {
        let account_jid = JID::new("user".to_string(), "s.whatsapp.net".to_string());
        let primary_registration = create_test_registration(0);
        let mut manager = MultiDeviceManager::new(account_jid, primary_registration);
        
        let contact_jid = "contact@s.whatsapp.net";
        let contact_registration = create_test_registration(5);
        
        manager.add_contact_device(contact_jid, contact_registration);
        
        let contact_devices = manager.get_contact_devices(contact_jid).unwrap();
        assert_eq!(contact_devices.len(), 1);
        assert!(contact_devices.contains_key(&5));
    }
    
    #[test]
    fn test_device_announcement() {
        let account_jid = JID::new("user".to_string(), "s.whatsapp.net".to_string());
        let primary_registration = create_test_registration(0);
        let manager = MultiDeviceManager::new(account_jid, primary_registration);
        
        let announcement = manager.generate_device_announcement(0).unwrap();
        assert_eq!(announcement.device_id, 0);
        assert_eq!(announcement.platform, "rust");
    }
    
    #[test]
    fn test_config_export_import() {
        let account_jid = JID::new("user".to_string(), "s.whatsapp.net".to_string());
        let primary_registration = create_test_registration(0);
        let mut manager = MultiDeviceManager::new(account_jid.clone(), primary_registration);
        
        let companion_registration = create_test_registration(1);
        manager.add_companion_device(companion_registration).unwrap();
        
        let exported = manager.export_config().unwrap();
        assert!(!exported.is_empty());
        
        let mut new_manager = MultiDeviceManager::new(
            JID::new("temp".to_string(), "s.whatsapp.net".to_string()),
            create_test_registration(99)
        );
        
        new_manager.import_config(&exported).unwrap();
        assert_eq!(new_manager.account_jid, account_jid);
        assert_eq!(new_manager.own_devices.len(), 2);
    }
}