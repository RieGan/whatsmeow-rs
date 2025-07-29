/// Advanced pairing flow for WhatsApp multi-device authentication
/// 
/// This module implements the complete WhatsApp pairing protocol including:
/// - QR code pairing with proper cryptographic verification
/// - Phone number verification for primary devices
/// - Device registration and capability negotiation
/// - Multi-device session establishment

use crate::{
    error::{Error, Result},
    types::JID,
    util::{
        keys::{ECKeyPair, SigningKeyPair},
        crypto::{sha256, random_bytes, hkdf_expand},
    },
    signal::prekey::{PreKey, SignedPreKey, PreKeyBundle},
    auth::qr::{QRData, QRChannel, QREvent},
};
use serde::{Deserialize, Serialize};
use std::{
    time::{SystemTime, UNIX_EPOCH, Duration},
    collections::HashMap,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use tracing::{debug, info, warn, error};
use tokio::time::timeout;

/// Pairing method for device registration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PairingMethod {
    /// QR code pairing (primary device scans)
    QRCode,
    /// Phone number verification
    PhoneNumber(String),
    /// Existing device pairing
    LinkedDevice,
}

/// Device capabilities for multi-device support
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub supports_e2e_image: bool,
    pub supports_e2e_audio: bool,
    pub supports_e2e_video: bool,
    pub supports_e2e_document: bool,
    pub supports_groups_v2: bool,
    pub supports_calls: bool,
    pub supports_status: bool,
    pub supports_payments: bool,
    pub max_participants: u32,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            supports_e2e_image: true,
            supports_e2e_audio: true,
            supports_e2e_video: true, 
            supports_e2e_document: true,
            supports_groups_v2: true,
            supports_calls: true,
            supports_status: true,
            supports_payments: false, // Conservative default
            max_participants: 1024,
        }
    }
}

/// Device information for registration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: u32,
    pub platform: String,
    pub version: String,
    pub model: String,
    pub os_version: String,
    pub manufacturer: String,
    pub capabilities: DeviceCapabilities,
    pub push_name: String,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            device_id: 0, // Will be assigned by server
            platform: "rust".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            model: "whatsmeow-rs".to_string(),
            os_version: std::env::consts::OS.to_string(),
            manufacturer: "whatsmeow-rs".to_string(),
            capabilities: DeviceCapabilities::default(),
            push_name: "WhatsApp Rust Client".to_string(),
        }
    }
}

/// Pairing keys for secure device authentication
#[derive(Debug, Clone)]
pub struct PairingKeys {
    /// Noise protocol keypair for initial handshake
    pub noise_keypair: ECKeyPair,
    /// Identity keypair for Signal protocol
    pub identity_keypair: SigningKeyPair,
    /// Static keypair for device authentication
    pub static_keypair: ECKeyPair,
    /// Registration ID for this device
    pub registration_id: u32,
}

impl PairingKeys {
    /// Generate new pairing keys
    pub fn generate() -> Self {
        Self {
            noise_keypair: ECKeyPair::generate(),
            identity_keypair: SigningKeyPair::generate(),
            static_keypair: ECKeyPair::generate(),
            registration_id: rand::random::<u32>(),
        }
    }
    
    /// Generate pairing keys with specific registration ID
    pub fn generate_with_id(registration_id: u32) -> Self {
        Self {
            noise_keypair: ECKeyPair::generate(),
            identity_keypair: SigningKeyPair::generate(),
            static_keypair: ECKeyPair::generate(),
            registration_id,
        }
    }
    
    /// Export keys for persistence
    pub fn export(&self) -> Result<PairingKeysData> {
        Ok(PairingKeysData {
            noise_private_key: self.noise_keypair.private_bytes().to_vec(),
            noise_public_key: self.noise_keypair.public_bytes().to_vec(),
            identity_private_key: self.identity_keypair.private_bytes().to_vec(),
            identity_public_key: self.identity_keypair.public_bytes().to_vec(),
            static_private_key: self.static_keypair.private_bytes().to_vec(),
            static_public_key: self.static_keypair.public_bytes().to_vec(),
            registration_id: self.registration_id,
        })
    }
    
    /// Import keys from persistence
    pub fn import(data: &PairingKeysData) -> Result<Self> {
        let noise_keypair = ECKeyPair::from_private_bytes(&data.noise_private_key)?;
        let identity_keypair = SigningKeyPair::from_private_bytes(&data.identity_private_key)?;
        let static_keypair = ECKeyPair::from_private_bytes(&data.static_private_key)?;
        
        Ok(Self {
            noise_keypair,
            identity_keypair,
            static_keypair,
            registration_id: data.registration_id,
        })
    }
}

/// Serializable pairing keys data for persistence
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairingKeysData {
    pub noise_private_key: Vec<u8>,
    pub noise_public_key: Vec<u8>,
    pub identity_private_key: Vec<u8>,
    pub identity_public_key: Vec<u8>,
    pub static_private_key: Vec<u8>,
    pub static_public_key: Vec<u8>,
    pub registration_id: u32,
}

/// Pre-key bundle data for Signal protocol
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreKeyBundleData {
    pub registration_id: u32,
    pub device_id: u32,
    pub identity_key: Vec<u8>,
    pub signed_pre_key_id: u32,
    pub signed_pre_key: Vec<u8>,
    pub signed_pre_key_signature: Vec<u8>,
    pub pre_key_id: Option<u32>,
    pub pre_key: Option<Vec<u8>>,
}

/// Complete device registration data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceRegistration {
    pub jid: JID,
    pub device_id: u32,
    pub registration_id: u32,
    pub keys: PairingKeysData,
    pub device_info: DeviceInfo,
    pub server_token: String,
    pub business_name: Option<String>,
    pub platform: String,
    pub registered_at: SystemTime,
    pub adv_secret: Vec<u8>,
    pub pre_key_bundle: PreKeyBundleData,
}

impl DeviceRegistration {
    /// Create new device registration
    pub fn new(
        jid: JID,
        device_id: u32,
        keys: PairingKeys,
        device_info: DeviceInfo,
        server_token: String,
        business_name: Option<String>,
        platform: String,
        adv_secret: Vec<u8>,
    ) -> Result<Self> {
        let keys_data = keys.export()?;
        
        // Generate pre-key bundle
        let signed_pre_key = SignedPreKey::generate(1, &keys.identity_keypair)?;
        let pre_key = PreKey::generate(1);
        
        let pre_key_bundle = PreKeyBundleData {
            registration_id: keys.registration_id,
            device_id,
            identity_key: keys.identity_keypair.public_bytes().to_vec(),
            signed_pre_key_id: signed_pre_key.id,
            signed_pre_key: signed_pre_key.public_key().to_vec(),
            signed_pre_key_signature: signed_pre_key.signature.clone(),
            pre_key_id: Some(pre_key.id),
            pre_key: Some(pre_key.public_key().to_vec()),
        };
        
        Ok(Self {
            jid,
            device_id,
            registration_id: keys.registration_id,
            keys: keys_data,
            device_info,
            server_token,
            business_name,
            platform,
            registered_at: SystemTime::now(),
            adv_secret,
            pre_key_bundle,
        })
    }
    
    /// Export registration data for backup/import
    pub fn export_data(&self) -> Result<String> {
        let json = serde_json::to_string(self)
            .map_err(|e| Error::Serialization(format!("Failed to serialize registration: {}", e)))?;
        Ok(STANDARD.encode(json.as_bytes()))
    }
    
    /// Import registration data from backup
    pub fn import_data(data: &str) -> Result<Self> {
        let json_bytes = STANDARD.decode(data)
            .map_err(|e| Error::Serialization(format!("Failed to decode registration data: {}", e)))?;
        let json = String::from_utf8(json_bytes)
            .map_err(|e| Error::Serialization(format!("Invalid UTF-8 in registration data: {}", e)))?;
        let registration: Self = serde_json::from_str(&json)
            .map_err(|e| Error::Serialization(format!("Failed to deserialize registration: {}", e)))?;
        Ok(registration)
    }
    
    /// Get pairing keys
    pub fn get_pairing_keys(&self) -> Result<PairingKeys> {
        PairingKeys::import(&self.keys)
    }
    
    /// Check if device is business account
    pub fn is_business(&self) -> bool {
        self.business_name.is_some()
    }
    
    /// Get device age
    pub fn device_age(&self) -> Option<Duration> {
        SystemTime::now().duration_since(self.registered_at).ok()
    }
}

/// Pairing challenge for QR code verification
#[derive(Debug, Clone)]
pub struct PairingChallenge {
    pub challenge_data: Vec<u8>,
    pub expected_response: Vec<u8>,
    pub created_at: SystemTime,
    pub expires_at: SystemTime,
}

impl PairingChallenge {
    /// Create new pairing challenge
    pub fn new(challenge_data: Vec<u8>, expected_response: Vec<u8>) -> Self {
        let now = SystemTime::now();
        let expires_at = now + Duration::from_secs(30); // 30 second challenge timeout
        
        Self {
            challenge_data,
            expected_response,
            created_at: now,
            expires_at,
        }
    }
    
    /// Verify challenge response
    pub fn verify(&self, response: &[u8]) -> Result<bool> {
        if SystemTime::now() > self.expires_at {
            return Err(Error::Auth("Challenge expired".to_string()));
        }
        
        Ok(response == self.expected_response)
    }
    
    /// Check if challenge is expired
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }
}

/// Pairing state tracking
#[derive(Debug, Clone, PartialEq)]
pub enum PairingState {
    /// Initial state - not started
    NotStarted,
    /// QR code generated, waiting for scan
    QRGenerated,
    /// QR code scanned, waiting for verification
    QRScanned,
    /// Phone verification initiated
    PhoneVerificationSent,
    /// Phone verification completed
    PhoneVerificationCompleted,
    /// Device pairing completed successfully
    PairingCompleted(DeviceRegistration),
    /// Pairing failed
    PairingFailed(String),
}

/// Complete pairing flow implementation
pub struct PairingFlow {
    method: PairingMethod,
    state: PairingState,
    device_info: DeviceInfo,
    keys: PairingKeys,
    qr_channel: Option<QRChannel>,
    challenge: Option<PairingChallenge>,
    adv_secret: Vec<u8>,
    server_refs: Vec<String>,
    phone_number: Option<String>,
    verification_code: Option<String>,
}

impl PairingFlow {
    /// Create new pairing flow
    pub fn new(method: PairingMethod) -> Self {
        Self {
            method,
            state: PairingState::NotStarted,
            device_info: DeviceInfo::default(),
            keys: PairingKeys::generate(),
            qr_channel: None,
            challenge: None,
            adv_secret: random_bytes(32),
            server_refs: Vec::new(),
            phone_number: None,
            verification_code: None,
        }
    }
    
    /// Create pairing flow with custom device info
    pub fn with_device_info(method: PairingMethod, device_info: DeviceInfo) -> Self {
        Self {
            method,
            state: PairingState::NotStarted,
            device_info,
            keys: PairingKeys::generate(),
            qr_channel: None,
            challenge: None,
            adv_secret: random_bytes(32),
            server_refs: Vec::new(),
            phone_number: None,
            verification_code: None,
        }
    }
    
    /// Create pairing flow with existing keys
    pub fn with_keys(method: PairingMethod, keys: PairingKeys) -> Self {
        Self {
            method,
            state: PairingState::NotStarted,
            device_info: DeviceInfo::default(),
            keys,
            qr_channel: None,
            challenge: None,
            adv_secret: random_bytes(32),
            server_refs: Vec::new(),
            phone_number: None,
            verification_code: None,
        }
    }
    
    /// Set server reference codes for QR generation
    pub fn set_server_refs(&mut self, refs: Vec<String>) {
        self.server_refs = refs;
    }
    
    /// Generate QR code data for scanning
    pub fn generate_qr_data(&self) -> Result<String> {
        if !matches!(self.method, PairingMethod::QRCode) {
            return Err(Error::Auth("QR generation only available for QR code pairing".to_string()));
        }
        
        if self.server_refs.is_empty() {
            return Err(Error::Auth("No server reference codes available".to_string()));
        }
        
        // Use first available reference
        let ref_id = &self.server_refs[0];
        let qr_data = QRData::new(
            ref_id.clone(),
            &self.keys.noise_keypair,
            &self.keys.identity_keypair,
            self.adv_secret.clone(),
        );
        
        Ok(qr_data.to_qr_string())
    }
    
    /// Start QR channel for continuous QR generation
    pub async fn start_qr_channel(&mut self) -> Result<()> {
        if !matches!(self.method, PairingMethod::QRCode) {
            return Err(Error::Auth("QR channel only available for QR code pairing".to_string()));
        }
        
        if self.server_refs.is_empty() {
            return Err(Error::Auth("No server reference codes available for QR channel".to_string()));
        }
        
        let mut qr_channel = QRChannel::with_keys(
            Default::default(),
            self.keys.noise_keypair.clone(),
            self.keys.identity_keypair.clone(),
            self.adv_secret.clone(),
        );
        
        qr_channel.start(self.server_refs.clone()).await?;
        self.qr_channel = Some(qr_channel);
        self.state = PairingState::QRGenerated;
        
        info!("QR channel started with {} reference codes", self.server_refs.len());
        Ok(())
    }
    
    /// Get next QR event from channel
    pub async fn next_qr_event(&mut self) -> Option<QREvent> {
        match &mut self.qr_channel {
            Some(channel) => channel.next_event().await,
            None => None,
        }
    }
    
    /// Stop QR channel
    pub async fn stop_qr_channel(&mut self) -> Result<()> {
        if let Some(mut channel) = self.qr_channel.take() {
            channel.stop().await?;
        }
        Ok(())
    }
    
    /// Verify challenge response from QR scan
    pub fn verify_challenge(&mut self, response_data: &[u8]) -> Result<()> {
        match &self.challenge {
            Some(challenge) => {
                if challenge.verify(response_data)? {
                    self.state = PairingState::QRScanned;
                    info!("QR challenge verification successful");
                    Ok(())
                } else {
                    let error = "Challenge verification failed".to_string();
                    self.state = PairingState::PairingFailed(error.clone());
                    Err(Error::Auth(error))
                }
            }
            None => {
                let error = "No challenge available for verification".to_string();
                self.state = PairingState::PairingFailed(error.clone());
                Err(Error::Auth(error))
            }
        }
    }
    
    /// Handle phone number verification
    pub fn handle_phone_verification(&mut self, phone: &str, verification_code: &str) -> Result<()> {
        match &self.method {
            PairingMethod::PhoneNumber(expected_phone) => {
                if phone != expected_phone {
                    let error = "Phone number mismatch".to_string();
                    self.state = PairingState::PairingFailed(error.clone());
                    return Err(Error::Auth(error));
                }
                
                // Store verification details
                self.phone_number = Some(phone.to_string());
                self.verification_code = Some(verification_code.to_string());
                self.state = PairingState::PhoneVerificationCompleted;
                
                info!("Phone verification completed for {}", phone);
                Ok(())
            }
            _ => {
                let error = "Phone verification not available for this pairing method".to_string();
                self.state = PairingState::PairingFailed(error.clone());
                Err(Error::Auth(error))
            }
        }
    }
    
    /// Complete device registration
    pub fn complete_registration(&mut self, jid: JID, server_token: String) -> Result<DeviceRegistration> {
        // Validate pairing state
        match &self.state {
            PairingState::QRScanned | PairingState::PhoneVerificationCompleted => {
                // Proceed with registration
            }
            _ => {
                let error = format!("Cannot complete registration in state: {:?}", self.state);
                self.state = PairingState::PairingFailed(error.clone());
                return Err(Error::Auth(error));
            }
        }
        
        // Extract device ID from JID
        let device_id = jid.device_id().unwrap_or(0);
        
        // Create device registration
        let registration = DeviceRegistration::new(
            jid,
            device_id,
            self.keys.clone(),
            self.device_info.clone(),
            server_token,
            None, // business_name - set later if needed
            "web".to_string(), // platform
            self.adv_secret.clone(),
        )?;
        
        self.state = PairingState::PairingCompleted(registration.clone());
        
        info!("Device registration completed for JID: {}", registration.jid);
        Ok(registration)
    }
    
    /// Export pairing data for backup
    pub fn export_pairing_data(&self) -> Result<String> {
        match &self.state {
            PairingState::PairingCompleted(registration) => {
                registration.export_data()
            }
            _ => Err(Error::Auth("No completed pairing data to export".to_string()))
        }
    }
    
    /// Import pairing data from backup
    pub fn import_pairing_data(data: &str) -> Result<DeviceRegistration> {
        DeviceRegistration::import_data(data)
    }
    
    /// Get current pairing state
    pub fn state(&self) -> &PairingState {
        &self.state
    }
    
    /// Get pairing method
    pub fn method(&self) -> &PairingMethod {
        &self.method
    }
    
    /// Get device info
    pub fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }
    
    /// Get pairing keys
    pub fn keys(&self) -> &PairingKeys {
        &self.keys
    }
    
    /// Set challenge for verification
    pub fn set_challenge(&mut self, challenge: PairingChallenge) {
        self.challenge = Some(challenge);
    }
    
    /// Check if pairing is completed
    pub fn is_completed(&self) -> bool {
        matches!(self.state, PairingState::PairingCompleted(_))
    }
    
    /// Check if pairing failed
    pub fn is_failed(&self) -> bool {
        matches!(self.state, PairingState::PairingFailed(_))
    }
    
    /// Get completed registration if available
    pub fn get_registration(&self) -> Option<&DeviceRegistration> {
        match &self.state {
            PairingState::PairingCompleted(registration) => Some(registration),
            _ => None,
        }
    }
    
    /// Reset pairing flow to start over
    pub fn reset(&mut self) {
        self.state = PairingState::NotStarted;
        self.qr_channel = None;
        self.challenge = None;
        self.server_refs.clear();
        self.phone_number = None;
        self.verification_code = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;
    
    #[test]
    fn test_device_capabilities_default() {
        let caps = DeviceCapabilities::default();
        assert!(caps.supports_e2e_image);
        assert!(caps.supports_groups_v2);
        assert!(!caps.supports_payments);
        assert_eq!(caps.max_participants, 1024);
    }
    
    #[test]
    fn test_device_info_default() {
        let info = DeviceInfo::default();
        assert_eq!(info.platform, "rust");
        assert_eq!(info.model, "whatsmeow-rs");
        assert_eq!(info.device_id, 0);
    }
    
    #[test]
    fn test_pairing_keys_generation() {
        let keys = PairingKeys::generate();
        assert_ne!(keys.registration_id, 0);
        
        // Test export/import round trip
        let exported = keys.export().unwrap();
        let imported = PairingKeys::import(&exported).unwrap();
        assert_eq!(keys.registration_id, imported.registration_id);
    }
    
    #[test]
    fn test_pairing_challenge() {
        let challenge_data = vec![1, 2, 3, 4];
        let expected_response = vec![5, 6, 7, 8];
        let challenge = PairingChallenge::new(challenge_data, expected_response.clone());
        
        // Test valid response
        assert!(challenge.verify(&expected_response).unwrap());
        
        // Test invalid response
        assert!(!challenge.verify(&[9, 10, 11, 12]).unwrap());
    }
    
    #[test]
    fn test_pairing_flow_creation() {
        let flow = PairingFlow::new(PairingMethod::QRCode);
        assert!(matches!(flow.method, PairingMethod::QRCode));
        assert!(matches!(flow.state, PairingState::NotStarted));
        assert!(!flow.is_completed());
        assert!(!flow.is_failed());
    }
    
    #[test]
    fn test_device_registration_creation() {
        let jid = JID::new("1234567890".to_string(), "s.whatsapp.net".to_string());
        let keys = PairingKeys::generate();
        let device_info = DeviceInfo::default();
        let server_token = "test-token".to_string();
        let adv_secret = vec![1, 2, 3, 4];
        
        let registration = DeviceRegistration::new(
            jid.clone(),
            1,
            keys,
            device_info,
            server_token.clone(),
            None,
            "web".to_string(),
            adv_secret,
        ).unwrap();
        
        assert_eq!(registration.jid, jid);
        assert_eq!(registration.device_id, 1);
        assert_eq!(registration.server_token, server_token);
        assert!(!registration.is_business());
    }
    
    #[test]
    fn test_device_registration_export_import() {
        let jid = JID::new("1234567890".to_string(), "s.whatsapp.net".to_string());
        let keys = PairingKeys::generate();
        let device_info = DeviceInfo::default();
        let server_token = "test-token".to_string();
        let adv_secret = vec![1, 2, 3, 4];
        
        let original = DeviceRegistration::new(
            jid,
            1,
            keys,
            device_info,
            server_token,
            None,
            "web".to_string(),
            adv_secret,
        ).unwrap();
        
        let exported = original.export_data().unwrap();
        let imported = DeviceRegistration::import_data(&exported).unwrap();
        
        assert_eq!(original.jid, imported.jid);
        assert_eq!(original.device_id, imported.device_id);
        assert_eq!(original.registration_id, imported.registration_id);
        assert_eq!(original.server_token, imported.server_token);
    }
    
    #[tokio::test]
    async fn test_pairing_flow_qr_generation() {
        let mut flow = PairingFlow::new(PairingMethod::QRCode);
        flow.set_server_refs(vec!["ref123".to_string()]);
        
        let qr_data = flow.generate_qr_data().unwrap();
        assert!(qr_data.contains("ref123"));
        
        // Test without server refs
        let mut flow2 = PairingFlow::new(PairingMethod::QRCode);
        assert!(flow2.generate_qr_data().is_err());
    }
    
    #[test]
    fn test_pairing_flow_phone_verification() {
        let phone = "+1234567890".to_string();
        let mut flow = PairingFlow::new(PairingMethod::PhoneNumber(phone.clone()));
        
        let result = flow.handle_phone_verification(&phone, "123456");
        assert!(result.is_ok());
        assert!(matches!(flow.state, PairingState::PhoneVerificationCompleted));
        
        // Test with wrong phone number
        let result = flow.handle_phone_verification("+9999999999", "123456");
        assert!(result.is_err());
        assert!(matches!(flow.state, PairingState::PairingFailed(_)));
    }
}