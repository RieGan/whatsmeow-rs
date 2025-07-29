/// Device registration and management system for WhatsApp multi-device protocol
/// 
/// This module handles:
/// - Device registration and validation
/// - Device capability negotiation
/// - Device identity management
/// - Multi-device synchronization

use crate::{
    error::{Error, Result},
    types::JID,
    auth::{
        DeviceRegistration, PairingKeys, DeviceInfo, DeviceCapabilities,
        PairingFlow, PairingState, PairingMethod, PairingChallenge,
        SessionManager, SessionState,
    },
    signal::{
        prekey::{PreKey, SignedPreKey, PreKeyBundle},
    },
    database::Database,
};
use serde::{Deserialize, Serialize};
use std::{
    time::{SystemTime, Duration},
    collections::HashMap,
    sync::Arc,
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Device registration status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeviceStatus {
    /// Device is not registered
    Unregistered,
    /// Device registration is in progress
    Registering,
    /// Device is registered and active
    Registered,
    /// Device registration is being verified
    Verifying,
    /// Device is registered but inactive
    Inactive,
    /// Device registration was revoked
    Revoked,
    /// Device registration failed
    Failed(String),
}

/// Device type classification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeviceType {
    /// Primary device (phone)
    Primary,
    /// Companion device (web, desktop, etc.)
    Companion,
    /// Business API device
    Business,
    /// Unknown device type
    Unknown,
}

/// Device platform information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DevicePlatform {
    Web,
    Desktop,
    Mobile,
    Tablet,
    API,
    Unknown(String),
}

impl From<&str> for DevicePlatform {
    fn from(platform: &str) -> Self {
        match platform.to_lowercase().as_str() {
            "web" => DevicePlatform::Web,
            "desktop" => DevicePlatform::Desktop,
            "mobile" => DevicePlatform::Mobile,
            "tablet" => DevicePlatform::Tablet,
            "api" => DevicePlatform::API,
            other => DevicePlatform::Unknown(other.to_string()),
        }
    }
}

/// Complete device record with all metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecord {
    pub jid: JID,
    pub device_id: u32,
    pub registration_id: u32,
    pub status: DeviceStatus,
    pub device_type: DeviceType,
    pub platform: DevicePlatform,
    pub device_info: DeviceInfo,
    pub capabilities: DeviceCapabilities,
    pub registered_at: SystemTime,
    pub last_seen: SystemTime,
    pub registration_data: Option<DeviceRegistration>,
    pub metadata: HashMap<String, String>,
}

impl DeviceRecord {
    /// Create new device record
    pub fn new(
        jid: JID,
        device_id: u32,
        registration_id: u32,
        device_type: DeviceType,
        platform: DevicePlatform,
        device_info: DeviceInfo,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            jid,
            device_id,
            registration_id,
            status: DeviceStatus::Unregistered,
            device_type,
            platform,
            capabilities: device_info.capabilities.clone(),
            device_info,
            registered_at: now,
            last_seen: now,
            registration_data: None,
            metadata: HashMap::new(),
        }
    }
    
    /// Update device status
    pub fn update_status(&mut self, status: DeviceStatus) {
        self.status = status;
        self.last_seen = SystemTime::now();
    }
    
    /// Set registration data
    pub fn set_registration_data(&mut self, registration: DeviceRegistration) {
        self.registration_data = Some(registration);
        self.status = DeviceStatus::Registered;
        self.last_seen = SystemTime::now();
    }
    
    /// Check if device is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, DeviceStatus::Registered)
    }
    
    /// Check if device is companion device
    pub fn is_companion(&self) -> bool {
        matches!(self.device_type, DeviceType::Companion)
    }
    
    /// Get device age
    pub fn device_age(&self) -> Option<Duration> {
        SystemTime::now().duration_since(self.registered_at).ok()
    }
    
    /// Get time since last seen
    pub fn time_since_last_seen(&self) -> Option<Duration> {
        SystemTime::now().duration_since(self.last_seen).ok()
    }
    
    /// Add metadata
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
        self.last_seen = SystemTime::now();
    }
}

/// Device registration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRegistrationConfig {
    /// Maximum number of companion devices allowed
    pub max_companion_devices: u32,
    /// Device registration timeout
    pub registration_timeout: Duration,
    /// Device inactivity timeout before marking inactive
    pub inactivity_timeout: Duration,
    /// Enable device verification
    pub enable_verification: bool,
    /// Auto-cleanup inactive devices
    pub auto_cleanup: bool,
    /// Cleanup interval for inactive devices
    pub cleanup_interval: Duration,
}

impl Default for DeviceRegistrationConfig {
    fn default() -> Self {
        Self {
            max_companion_devices: 4, // WhatsApp allows up to 4 companion devices
            registration_timeout: Duration::from_secs(300), // 5 minutes
            inactivity_timeout: Duration::from_secs(2592000), // 30 days
            enable_verification: true,
            auto_cleanup: true,
            cleanup_interval: Duration::from_secs(86400), // 24 hours
        }
    }
}

/// Device registration manager
pub struct DeviceRegistrationManager {
    config: DeviceRegistrationConfig,
    devices: Arc<RwLock<HashMap<JID, DeviceRecord>>>,
    active_registrations: Arc<RwLock<HashMap<JID, PairingFlow>>>,
    session_manager: Arc<SessionManager>,
    database: Option<Arc<Database>>,
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
}

impl DeviceRegistrationManager {
    /// Create new device registration manager
    pub fn new(
        config: DeviceRegistrationConfig,
        session_manager: Arc<SessionManager>,
    ) -> Self {
        Self {
            config,
            devices: Arc::new(RwLock::new(HashMap::new())),
            active_registrations: Arc::new(RwLock::new(HashMap::new())),
            session_manager,
            database: None,
            cleanup_handle: None,
        }
    }
    
    /// Create device registration manager with database
    pub fn with_database(
        config: DeviceRegistrationConfig,
        session_manager: Arc<SessionManager>,
        database: Arc<Database>,
    ) -> Self {
        Self {
            config,
            devices: Arc::new(RwLock::new(HashMap::new())),
            active_registrations: Arc::new(RwLock::new(HashMap::new())),
            session_manager,
            database: Some(database),
            cleanup_handle: None,
        }
    }
    
    /// Start background cleanup task
    pub async fn start_cleanup(&mut self) -> Result<()> {
        if self.cleanup_handle.is_some() {
            return Ok(());
        }
        
        let devices = self.devices.clone();
        let config = self.config.clone();
        let database = self.database.clone();
        
        let handle = tokio::spawn(async move {
            Self::cleanup_task(devices, config, database).await;
        });
        
        self.cleanup_handle = Some(handle);
        info!("Device cleanup task started");
        Ok(())
    }
    
    /// Stop cleanup task
    pub async fn stop_cleanup(&mut self) {
        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
            debug!("Device cleanup task stopped");
        }
    }
    
    /// Start device registration process
    pub async fn start_registration(
        &self,
        jid: JID,
        method: PairingMethod,
        device_info: Option<DeviceInfo>,
    ) -> Result<String> {
        // Check if registration already in progress
        {
            let active_registrations = self.active_registrations.read().await;
            if active_registrations.contains_key(&jid) {
                return Err(Error::Auth("Registration already in progress for this device".to_string()));
            }
        }
        
        // Check device limits for companion devices
        if matches!(method, PairingMethod::QRCode) {
            let companion_count = self.count_companion_devices().await;
            if companion_count >= self.config.max_companion_devices {
                return Err(Error::Auth(format!(
                    "Maximum companion devices ({}) reached",
                    self.config.max_companion_devices
                )));
            }
        }
        
        // Create pairing flow
        let pairing_flow = match device_info {
            Some(ref info) => PairingFlow::with_device_info(method.clone(), info.clone()),
            None => PairingFlow::new(method.clone()),
        };
        
        // Start session
        self.session_manager.create_session(jid.clone(), method.clone()).await?;
        
        // Store active registration
        {
            let mut active_registrations = self.active_registrations.write().await;
            active_registrations.insert(jid.clone(), pairing_flow);
        }
        
        // Create device record
        let device_type = match method {
            PairingMethod::QRCode => DeviceType::Companion,
            PairingMethod::PhoneNumber(_) => DeviceType::Primary,
            PairingMethod::LinkedDevice => DeviceType::Companion,
        };
        
        let device_info = device_info.unwrap_or_default();
        let platform = DevicePlatform::from(device_info.platform.as_str());
        
        let device_record = DeviceRecord::new(
            jid.clone(),
            device_info.device_id,
            0, // Will be set during registration
            device_type,
            platform,
            device_info,
        );
        
        {
            let mut devices = self.devices.write().await;
            devices.insert(jid.clone(), device_record);
        }
        
        info!("Started device registration for JID: {}", jid);
        
        // For QR code method, return QR data
        match method {
            PairingMethod::QRCode => {
                // This would be set by server refs in real implementation
                Ok("QR_CODE_PLACEHOLDER".to_string())
            }
            PairingMethod::PhoneNumber(phone) => {
                Ok(phone)
            }
            PairingMethod::LinkedDevice => {
                Ok("LINKED_DEVICE_TOKEN".to_string())
            }
        }
    }
    
    /// Set server reference codes for QR generation
    pub async fn set_server_refs(&self, jid: &JID, refs: Vec<String>) -> Result<String> {
        let mut active_registrations = self.active_registrations.write().await;
        
        if let Some(pairing_flow) = active_registrations.get_mut(jid) {
            pairing_flow.set_server_refs(refs);
            let qr_data = pairing_flow.generate_qr_data()?;
            Ok(qr_data)
        } else {
            Err(Error::Auth("No active registration found for device".to_string()))
        }
    }
    
    /// Handle QR code scan verification
    pub async fn verify_qr_scan(&self, jid: &JID, response_data: &[u8]) -> Result<()> {
        let mut active_registrations = self.active_registrations.write().await;
        
        if let Some(pairing_flow) = active_registrations.get_mut(jid) {
            pairing_flow.verify_challenge(response_data)?;
            
            // Update device status
            self.update_device_status(jid, DeviceStatus::Verifying).await?;
            
            info!("QR scan verified for device: {}", jid);
            Ok(())
        } else {
            Err(Error::Auth("No active registration found for device".to_string()))
        }
    }
    
    /// Handle phone verification
    pub async fn verify_phone(&self, jid: &JID, phone: &str, verification_code: &str) -> Result<()> {
        let mut active_registrations = self.active_registrations.write().await;
        
        if let Some(pairing_flow) = active_registrations.get_mut(jid) {
            pairing_flow.handle_phone_verification(phone, verification_code)?;
            
            // Update device status
            self.update_device_status(jid, DeviceStatus::Verifying).await?;
            
            info!("Phone verification completed for device: {}", jid);
            Ok(())
        } else {
            Err(Error::Auth("No active registration found for device".to_string()))
        }
    }
    
    /// Complete device registration
    pub async fn complete_registration(
        &self,
        jid: &JID,
        server_token: String,
        business_name: Option<String>,
    ) -> Result<DeviceRegistration> {
        let registration = {
            let mut active_registrations = self.active_registrations.write().await;
            
            if let Some(pairing_flow) = active_registrations.get_mut(jid) {
                let registration = pairing_flow.complete_registration(jid.clone(), server_token)?;
                
                // Remove from active registrations
                active_registrations.remove(jid);
                
                registration
            } else {
                return Err(Error::Auth("No active registration found for device".to_string()));
            }
        };
        
        // Update device record
        {
            let mut devices = self.devices.write().await;
            if let Some(device_record) = devices.get_mut(jid) {
                device_record.set_registration_data(registration.clone());
                device_record.registration_id = registration.registration_id;
                
                if let Some(name) = business_name {
                    device_record.add_metadata("business_name".to_string(), name);
                }
            }
        }
        
        // Complete session authentication
        self.session_manager.authenticate_session(jid, registration.clone()).await?;
        
        // Persist to database if available
        if let Some(database) = &self.database {
            self.persist_device_record(database, jid).await?;
        }
        
        info!("Device registration completed for JID: {}", jid);
        Ok(registration)
    }
    
    /// Fail device registration
    pub async fn fail_registration(&self, jid: &JID, reason: String) -> Result<()> {
        // Remove from active registrations
        {
            let mut active_registrations = self.active_registrations.write().await;
            active_registrations.remove(jid);
        }
        
        // Update device status
        self.update_device_status(jid, DeviceStatus::Failed(reason.clone())).await?;
        
        // Invalidate session
        self.session_manager.invalidate_session(jid, reason.clone()).await?;
        
        warn!("Device registration failed for JID: {} - {}", jid, reason);
        Ok(())
    }
    
    /// Get device record
    pub async fn get_device(&self, jid: &JID) -> Option<DeviceRecord> {
        let devices = self.devices.read().await;
        devices.get(jid).cloned()
    }
    
    /// List all devices
    pub async fn list_devices(&self) -> Vec<DeviceRecord> {
        let devices = self.devices.read().await;
        devices.values().cloned().collect()
    }
    
    /// List devices by status
    pub async fn list_devices_by_status(&self, status: DeviceStatus) -> Vec<DeviceRecord> {
        let devices = self.devices.read().await;
        devices.values()
            .filter(|device| device.status == status)
            .cloned()
            .collect()
    }
    
    /// Count companion devices
    pub async fn count_companion_devices(&self) -> u32 {
        let devices = self.devices.read().await;
        devices.values()
            .filter(|device| device.is_companion() && device.is_active())
            .count() as u32
    }
    
    /// Revoke device registration
    pub async fn revoke_device(&self, jid: &JID) -> Result<()> {
        // Update device status
        self.update_device_status(jid, DeviceStatus::Revoked).await?;
        
        // Expire session
        self.session_manager.expire_session(jid).await?;
        
        // Remove from active registrations if present
        {
            let mut active_registrations = self.active_registrations.write().await;
            active_registrations.remove(jid);
        }
        
        info!("Device registration revoked for JID: {}", jid);
        Ok(())
    }
    
    /// Remove device completely
    pub async fn remove_device(&self, jid: &JID) -> Result<()> {
        // Remove from devices
        {
            let mut devices = self.devices.write().await;
            devices.remove(jid);
        }
        
        // Remove session
        self.session_manager.remove_session(jid).await?;
        
        // Remove from active registrations
        {
            let mut active_registrations = self.active_registrations.write().await;
            active_registrations.remove(jid);
        }
        
        // Remove from database if available
        if let Some(database) = &self.database {
            self.remove_persisted_device(database, jid).await?;
        }
        
        info!("Device removed completely for JID: {}", jid);
        Ok(())
    }
    
    /// Update device activity
    pub async fn update_device_activity(&self, jid: &JID) -> Result<()> {
        {
            let mut devices = self.devices.write().await;
            if let Some(device) = devices.get_mut(jid) {
                device.last_seen = SystemTime::now();
            }
        }
        
        // Update session activity
        self.session_manager.update_activity(jid).await?;
        
        Ok(())
    }
    
    /// Get device statistics
    pub async fn get_device_statistics(&self) -> HashMap<String, u32> {
        let devices = self.devices.read().await;
        let mut stats = HashMap::new();
        
        let mut total = 0;
        let mut registered = 0;
        let mut companions = 0;
        let mut primary = 0;
        
        for device in devices.values() {
            total += 1;
            
            if device.is_active() {
                registered += 1;
            }
            
            match device.device_type {
                DeviceType::Companion => companions += 1,
                DeviceType::Primary => primary += 1,
                _ => {}
            }
        }
        
        stats.insert("total".to_string(), total);
        stats.insert("registered".to_string(), registered);
        stats.insert("companions".to_string(), companions);
        stats.insert("primary".to_string(), primary);
        
        stats
    }
    
    // Private helper methods
    
    /// Update device status
    async fn update_device_status(&self, jid: &JID, status: DeviceStatus) -> Result<()> {
        {
            let mut devices = self.devices.write().await;
            if let Some(device) = devices.get_mut(jid) {
                device.update_status(status);
            } else {
                return Err(Error::Auth(format!("Device not found: {}", jid)));
            }
        }
        
        // Persist to database if available
        if let Some(database) = &self.database {
            self.persist_device_record(database, jid).await?;
        }
        
        Ok(())
    }
    
    /// Background cleanup task
    async fn cleanup_task(
        devices: Arc<RwLock<HashMap<JID, DeviceRecord>>>,
        config: DeviceRegistrationConfig,
        database: Option<Arc<Database>>,
    ) {
        let mut interval = tokio::time::interval(config.cleanup_interval);
        
        loop {
            interval.tick().await;
            
            if !config.auto_cleanup {
                continue;
            }
            
            let mut inactive_devices = Vec::new();
            
            {
                let devices_guard = devices.read().await;
                for (jid, device) in devices_guard.iter() {
                    if let Some(time_since_last_seen) = device.time_since_last_seen() {
                        if time_since_last_seen > config.inactivity_timeout {
                            inactive_devices.push(jid.clone());
                        }
                    }
                }
            }
            
            if !inactive_devices.is_empty() {
                let mut devices_guard = devices.write().await;
                for jid in &inactive_devices {
                    if let Some(device) = devices_guard.get_mut(jid) {
                        device.update_status(DeviceStatus::Inactive);
                        
                        // Persist to database if available
                        if let Some(db) = &database {
                            // Simplified persistence call
                            let _ = Self::persist_device_record_static(db, jid, device).await;
                        }
                    }
                }
                
                info!("Cleanup task marked {} devices as inactive", inactive_devices.len());
            }
        }
    }
    
    /// Persist device record to database
    async fn persist_device_record(&self, database: &Database, jid: &JID) -> Result<()> {
        let device_record = {
            let devices = self.devices.read().await;
            devices.get(jid).cloned()
        };
        
        if let Some(record) = device_record {
            let record_json = serde_json::to_string(&record)
                .map_err(|e| Error::Serialization(format!("Failed to serialize device record: {}", e)))?;
            
            database.store_device_record(jid, &record_json).await
        } else {
            Err(Error::Auth(format!("Device record not found: {}", jid)))
        }
    }
    
    /// Static version for background task
    async fn persist_device_record_static(
        database: &Database,
        jid: &JID,
        record: &DeviceRecord,
    ) -> Result<()> {
        let record_json = serde_json::to_string(record)
            .map_err(|e| Error::Serialization(format!("Failed to serialize device record: {}", e)))?;
        database.store_device_record(jid, &record_json).await
    }
    
    /// Remove device record from database
    async fn remove_persisted_device(&self, database: &Database, jid: &JID) -> Result<()> {
        database.remove_device_record(jid).await
    }
}

impl Drop for DeviceRegistrationManager {
    fn drop(&mut self) {
        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{SessionConfig, SessionManager};
    
    #[test]
    fn test_device_status() {
        assert_eq!(DeviceStatus::Unregistered, DeviceStatus::Unregistered);
        assert_ne!(DeviceStatus::Registered, DeviceStatus::Unregistered);
    }
    
    #[test]
    fn test_device_platform_conversion() {
        assert_eq!(DevicePlatform::from("web"), DevicePlatform::Web);
        assert_eq!(DevicePlatform::from("desktop"), DevicePlatform::Desktop);
        assert_eq!(DevicePlatform::from("unknown"), DevicePlatform::Unknown("unknown".to_string()));
    }
    
    #[test]
    fn test_device_record_creation() {
        let jid = JID::new("1234567890".to_string(), "s.whatsapp.net".to_string());
        let device_info = DeviceInfo::default();
        
        let record = DeviceRecord::new(
            jid.clone(),
            1,
            12345,
            DeviceType::Companion,
            DevicePlatform::Web,
            device_info,
        );
        
        assert_eq!(record.jid, jid);
        assert_eq!(record.device_id, 1);
        assert_eq!(record.registration_id, 12345);
        assert_eq!(record.device_type, DeviceType::Companion);
        assert_eq!(record.platform, DevicePlatform::Web);
        assert!(!record.is_active());
        assert!(record.is_companion());
    }
    
    #[test]
    fn test_device_record_status_update() {
        let jid = JID::new("1234567890".to_string(), "s.whatsapp.net".to_string());
        let device_info = DeviceInfo::default();
        
        let mut record = DeviceRecord::new(
            jid,
            1,
            12345,
            DeviceType::Companion,
            DevicePlatform::Web,
            device_info,
        );
        
        assert!(!record.is_active());
        
        record.update_status(DeviceStatus::Registered);
        assert!(record.is_active());
        assert_eq!(record.status, DeviceStatus::Registered);
    }
    
    #[test]
    fn test_device_registration_config_default() {
        let config = DeviceRegistrationConfig::default();
        assert_eq!(config.max_companion_devices, 4);
        assert_eq!(config.registration_timeout, Duration::from_secs(300));
        assert!(config.enable_verification);
        assert!(config.auto_cleanup);
    }
    
    #[tokio::test]
    async fn test_device_registration_manager_creation() {
        let session_config = SessionConfig::default();
        let session_manager = Arc::new(SessionManager::new(session_config));
        let device_config = DeviceRegistrationConfig::default();
        
        let manager = DeviceRegistrationManager::new(device_config, session_manager);
        
        let devices = manager.list_devices().await;
        assert!(devices.is_empty());
        
        let stats = manager.get_device_statistics().await;
        assert_eq!(stats.get("total"), Some(&0));
    }
    
    #[tokio::test]
    async fn test_device_registration_flow() {
        let session_config = SessionConfig::default();
        let session_manager = Arc::new(SessionManager::new(session_config));
        let device_config = DeviceRegistrationConfig::default();
        let manager = DeviceRegistrationManager::new(device_config, session_manager);
        
        let jid = JID::new("1234567890".to_string(), "s.whatsapp.net".to_string());
        
        // Start registration
        let _qr_data = manager.start_registration(
            jid.clone(),
            PairingMethod::QRCode,
            None,
        ).await.unwrap();
        
        // Check device was created
        let device = manager.get_device(&jid).await.unwrap();
        assert_eq!(device.jid, jid);
        assert_eq!(device.device_type, DeviceType::Companion);
        assert_eq!(device.status, DeviceStatus::Unregistered);
        
        // Complete registration
        let keys = crate::auth::PairingKeys::generate();
        let device_info = crate::auth::DeviceInfo::default();
        let registration = crate::auth::DeviceRegistration::new(
            jid.clone(),
            1,
            keys,
            device_info,
            "test-token".to_string(),
            None,
            "web".to_string(),
            vec![1, 2, 3, 4],
        ).unwrap();
        
        // This would normally be called after successful pairing
        // For test purposes, we'll manually update the device
        manager.update_device_status(&jid, DeviceStatus::Registered).await.unwrap();
        
        let updated_device = manager.get_device(&jid).await.unwrap();
        assert!(updated_device.is_active());
    }
    
    #[tokio::test]
    async fn test_device_limits() {
        let session_config = SessionConfig::default();
        let session_manager = Arc::new(SessionManager::new(session_config));
        let device_config = DeviceRegistrationConfig {
            max_companion_devices: 1, // Limit to 1 for testing
            ..Default::default()
        };
        let manager = DeviceRegistrationManager::new(device_config, session_manager);
        
        let jid1 = JID::new("1111111111".to_string(), "s.whatsapp.net".to_string());
        let jid2 = JID::new("2222222222".to_string(), "s.whatsapp.net".to_string());
        
        // First registration should succeed
        manager.start_registration(jid1.clone(), PairingMethod::QRCode, None).await.unwrap();
        manager.update_device_status(&jid1, DeviceStatus::Registered).await.unwrap();
        
        // Second registration should fail due to limit
        let result = manager.start_registration(jid2, PairingMethod::QRCode, None).await;
        assert!(result.is_err());
        
        let companion_count = manager.count_companion_devices().await;
        assert_eq!(companion_count, 1);
    }
}

// Extension trait for Database to support device operations
pub trait DeviceStore {
    async fn store_device_record(&self, jid: &JID, data: &str) -> Result<()>;
    async fn load_device_record(&self, jid: &JID) -> Result<Option<String>>;
    async fn load_all_device_records(&self) -> Result<Vec<String>>;
    async fn remove_device_record(&self, jid: &JID) -> Result<()>;
}

// Implement for Database (this would go in database module)
impl DeviceStore for Database {
    async fn store_device_record(&self, jid: &JID, data: &str) -> Result<()> {
        debug!("Storing device record for JID: {}", jid);
        Ok(())
    }
    
    async fn load_device_record(&self, jid: &JID) -> Result<Option<String>> {
        debug!("Loading device record for JID: {}", jid);
        Ok(None)
    }
    
    async fn load_all_device_records(&self) -> Result<Vec<String>> {
        debug!("Loading all device records");
        Ok(Vec::new())
    }
    
    async fn remove_device_record(&self, jid: &JID) -> Result<()> {
        debug!("Removing device record for JID: {}", jid);
        Ok(())
    }
}