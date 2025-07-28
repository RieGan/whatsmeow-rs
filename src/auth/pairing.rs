/// Advanced pairing flow for WhatsApp multi-device authentication

use crate::{
    error::{Error, Result},
    types::JID,
    util::{
        keys::{ECKeyPair, SigningKeyPair},
        crypto::sha256,
    },
    signal::prekey::{PreKey, SignedPreKey, PreKeyBundle},
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

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
    
    /// Generate signed pre-key for this device
    pub fn generate_signed_prekey(&self, id: u32) -> Result<SignedPreKey> {
        SignedPreKey::generate(id, &self.identity_keypair)
    }
    
    /// Create pre-key bundle for registration
    pub fn create_prekey_bundle(&self, device_info: &DeviceInfo) -> Result<PreKeyBundle> {
        let signed_prekey = self.generate_signed_prekey(1)?;
        let prekey = PreKey::generate(1);
        
        PreKeyBundle::new(
            &self.identity_keypair,
            signed_prekey.id,
            Some(prekey.id),
            self.registration_id,
            device_info.device_id,
        )
    }
}

/// Pairing challenge for secure authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingChallenge {
    pub challenge_data: Vec<u8>,
    pub timestamp: u64,
    pub method: PairingMethod,
}

impl PairingChallenge {
    /// Create a new pairing challenge
    pub fn new(method: PairingMethod) -> Self {
        let mut challenge_data = vec![0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut challenge_data);
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            challenge_data,
            timestamp,
            method,
        }
    }
    
    /// Verify challenge response
    pub fn verify_response(&self, response: &[u8], expected_response: &[u8]) -> bool {
        response == expected_response
    }
    
    /// Generate expected response for challenge
    pub fn generate_response(&self, private_key: &[u8; 32]) -> Result<Vec<u8>> {
        let mut input = self.challenge_data.clone();
        input.extend_from_slice(private_key);
        input.extend_from_slice(&self.timestamp.to_be_bytes());
        
        Ok(sha256(&input).to_vec())
    }
}

/// Advanced pairing flow manager
pub struct PairingFlow {
    method: PairingMethod,
    keys: PairingKeys,
    device_info: DeviceInfo,
    challenge: Option<PairingChallenge>,
    state: PairingState,
}

/// Pairing flow states
#[derive(Debug, Clone, PartialEq)]
pub enum PairingState {
    /// Initial state - ready to start pairing
    Ready,
    /// Challenge generated, waiting for response
    ChallengeGenerated,
    /// Challenge verified, waiting for device registration
    ChallengeVerified,
    /// Device registered successfully
    DeviceRegistered(DeviceRegistration),
    /// Pairing failed
    Failed(String),
}

/// Complete device registration information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceRegistration {
    pub jid: JID,
    pub device_info: DeviceInfo,
    pub keys: PairingKeysData,
    pub pre_key_bundle: PreKeyBundleData,
    pub server_token: String,
    pub push_token: Option<String>,
}

/// Serializable pairing keys data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairingKeysData {
    pub noise_public: Vec<u8>,
    pub noise_private: Vec<u8>,
    pub identity_public: Vec<u8>,
    pub identity_private: Vec<u8>,
    pub static_public: Vec<u8>,
    pub static_private: Vec<u8>,
    pub registration_id: u32,
}

/// Serializable pre-key bundle data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreKeyBundleData {
    pub identity_key: Vec<u8>,
    pub signed_prekey_id: u32,
    pub signed_prekey_public: Vec<u8>,
    pub signed_prekey_signature: Vec<u8>,
    pub prekey_id: Option<u32>,
    pub prekey_public: Option<Vec<u8>>,
    pub registration_id: u32,
    pub device_id: u32,
}

impl PairingFlow {
    /// Create new pairing flow
    pub fn new(method: PairingMethod) -> Self {
        Self {
            method,
            keys: PairingKeys::generate(),
            device_info: DeviceInfo::default(),
            challenge: None,
            state: PairingState::Ready,
        }
    }
    
    /// Create pairing flow with custom device info
    pub fn with_device_info(method: PairingMethod, device_info: DeviceInfo) -> Self {
        Self {
            method,
            keys: PairingKeys::generate(),
            device_info,
            challenge: None,
            state: PairingState::Ready,
        }
    }
    
    /// Get current pairing state
    pub fn state(&self) -> &PairingState {
        &self.state
    }
    
    /// Get device information
    pub fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }
    
    /// Get pairing method
    pub fn method(&self) -> &PairingMethod {
        &self.method
    }
    
    /// Generate pairing challenge
    pub fn generate_challenge(&mut self) -> Result<PairingChallenge> {
        if self.state != PairingState::Ready {
            return Err(Error::Auth("Invalid state for challenge generation".to_string()));
        }
        
        let challenge = PairingChallenge::new(self.method.clone());
        self.challenge = Some(challenge.clone());
        self.state = PairingState::ChallengeGenerated;
        
        Ok(challenge)
    }
    
    /// Verify challenge response
    pub fn verify_challenge(&mut self, response: &[u8]) -> Result<()> {
        let challenge = self.challenge.as_ref()
            .ok_or_else(|| Error::Auth("No challenge generated".to_string()))?;
        
        if self.state != PairingState::ChallengeGenerated {
            return Err(Error::Auth("Invalid state for challenge verification".to_string()));
        }
        
        let expected_response = challenge.generate_response(&self.keys.static_keypair.private_bytes())?;
        
        if !challenge.verify_response(response, &expected_response) {
            self.state = PairingState::Failed("Challenge verification failed".to_string());
            return Err(Error::Auth("Invalid challenge response".to_string()));
        }
        
        self.state = PairingState::ChallengeVerified;
        Ok(())
    }
    
    /// Complete device registration
    pub fn complete_registration(&mut self, jid: JID, server_token: String) -> Result<DeviceRegistration> {
        if self.state != PairingState::ChallengeVerified {
            return Err(Error::Auth("Invalid state for registration completion".to_string()));
        }
        
        // Create pre-key bundle
        let pre_key_bundle = self.keys.create_prekey_bundle(&self.device_info)?;
        
        // Convert keys to serializable format
        let keys_data = PairingKeysData {
            noise_public: self.keys.noise_keypair.public_bytes().to_vec(),
            noise_private: self.keys.noise_keypair.private_bytes().to_vec(),
            identity_public: self.keys.identity_keypair.public_bytes().to_vec(),
            identity_private: self.keys.identity_keypair.private_bytes().to_vec(),
            static_public: self.keys.static_keypair.public_bytes().to_vec(),
            static_private: self.keys.static_keypair.private_bytes().to_vec(),
            registration_id: self.keys.registration_id,
        };
        
        // Convert pre-key bundle to serializable format
        let pre_key_bundle_data = PreKeyBundleData {
            identity_key: pre_key_bundle.identity_key.clone(),
            signed_prekey_id: pre_key_bundle.signed_prekey.id,
            signed_prekey_public: pre_key_bundle.signed_prekey.public_key().to_vec(),
            signed_prekey_signature: pre_key_bundle.signed_prekey.signature.clone(),
            prekey_id: pre_key_bundle.prekey.as_ref().map(|pk| pk.id),
            prekey_public: pre_key_bundle.prekey.as_ref().map(|pk| pk.public_key().to_vec()),
            registration_id: pre_key_bundle.registration_id,
            device_id: pre_key_bundle.device_id,
        };
        
        let registration = DeviceRegistration {
            jid: jid.clone(),
            device_info: self.device_info.clone(),
            keys: keys_data,
            pre_key_bundle: pre_key_bundle_data,
            server_token,
            push_token: None,
        };
        
        self.state = PairingState::DeviceRegistered(registration.clone());
        Ok(registration)
    }
    
    /// Generate QR code data for pairing
    pub fn generate_qr_data(&self) -> Result<String> {
        if self.method != PairingMethod::QRCode {
            return Err(Error::Auth("QR code generation only available for QR pairing method".to_string()));
        }
        
        let qr_data = crate::auth::QRData {
            ref_id: uuid::Uuid::new_v4().to_string(),
            public_key: self.keys.noise_keypair.public_bytes().to_vec(),
            adv_secret: self.keys.static_keypair.public_bytes().to_vec(),
        };
        
        Ok(qr_data.encode())
    }
    
    /// Handle phone number verification
    pub fn handle_phone_verification(&mut self, phone: &str, verification_code: &str) -> Result<()> {
        if let PairingMethod::PhoneNumber(expected_phone) = &self.method {
            if phone != expected_phone {
                return Err(Error::Auth("Phone number mismatch".to_string()));
            }
            
            // In a real implementation, verify the code with WhatsApp servers
            if verification_code.len() != 6 || !verification_code.chars().all(|c| c.is_ascii_digit()) {
                return Err(Error::Auth("Invalid verification code format".to_string()));
            }
            
            self.state = PairingState::ChallengeVerified;
            Ok(())
        } else {
            Err(Error::Auth("Phone verification not available for this pairing method".to_string()))
        }
    }
    
    /// Export pairing data for backup/restore
    pub fn export_pairing_data(&self) -> Result<String> {
        match &self.state {
            PairingState::DeviceRegistered(registration) => {
                let json = serde_json::to_string_pretty(registration)
                    .map_err(|e| Error::Protocol(format!("Failed to serialize pairing data: {}", e)))?;
                Ok(json)
            }
            _ => Err(Error::Auth("Device not registered yet".to_string()))
        }
    }
    
    /// Import pairing data from backup
    pub fn import_pairing_data(data: &str) -> Result<DeviceRegistration> {
        serde_json::from_str::<DeviceRegistration>(data)
            .map_err(|e| Error::Protocol(format!("Failed to deserialize pairing data: {}", e)))
    }
    
    /// Restore keys from pairing data
    pub fn restore_keys(pairing_data: &PairingKeysData) -> Result<PairingKeys> {
        let noise_keypair = ECKeyPair::from_private_bytes(&pairing_data.noise_private)?;
        let identity_keypair = SigningKeyPair::from_private_bytes(&pairing_data.identity_private)?;
        let static_keypair = ECKeyPair::from_private_bytes(&pairing_data.static_private)?;
        
        Ok(PairingKeys {
            noise_keypair,
            identity_keypair,
            static_keypair,
            registration_id: pairing_data.registration_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pairing_keys_generation() {
        let keys = PairingKeys::generate();
        assert_ne!(keys.registration_id, 0);
        assert_eq!(keys.noise_keypair.public_bytes().len(), 32);
        assert_eq!(keys.identity_keypair.public_bytes().len(), 32);
        assert_eq!(keys.static_keypair.public_bytes().len(), 32);
    }
    
    #[test]
    fn test_pairing_challenge() {
        let challenge = PairingChallenge::new(PairingMethod::QRCode);
        assert_eq!(challenge.challenge_data.len(), 32);
        assert!(challenge.timestamp > 0);
        
        let private_key = [1u8; 32];
        let response = challenge.generate_response(&private_key).unwrap();
        assert_eq!(response.len(), 32);
    }
    
    #[test]
    fn test_pairing_flow_qr() {
        let mut flow = PairingFlow::new(PairingMethod::QRCode);
        assert_eq!(flow.state(), &PairingState::Ready);
        
        let challenge = flow.generate_challenge().unwrap();
        assert_eq!(flow.state(), &PairingState::ChallengeGenerated);
        
        let response = challenge.generate_response(&flow.keys.static_keypair.private_bytes()).unwrap();
        flow.verify_challenge(&response).unwrap();
        assert_eq!(flow.state(), &PairingState::ChallengeVerified);
    }
    
    #[test]
    fn test_pairing_flow_phone() {
        let phone = "+1234567890".to_string();
        let mut flow = PairingFlow::new(PairingMethod::PhoneNumber(phone));
        
        flow.handle_phone_verification("+1234567890", "123456").unwrap();
        assert_eq!(flow.state(), &PairingState::ChallengeVerified);
    }
    
    #[test]
    fn test_device_capabilities() {
        let caps = DeviceCapabilities::default();
        assert!(caps.supports_e2e_image);
        assert!(caps.supports_groups_v2);
        assert_eq!(caps.max_participants, 1024);
    }
    
    #[test]
    fn test_device_info() {
        let info = DeviceInfo::default();
        assert_eq!(info.platform, "rust");
        assert!(!info.version.is_empty());
    }
    
    #[test]
    fn test_pairing_data_export_import() {
        let mut flow = PairingFlow::new(PairingMethod::QRCode);
        let challenge = flow.generate_challenge().unwrap();
        let response = challenge.generate_response(&flow.keys.static_keypair.private_bytes()).unwrap();
        flow.verify_challenge(&response).unwrap();
        
        let jid = JID::new("test".to_string(), "s.whatsapp.net".to_string());
        let registration = flow.complete_registration(jid, "test_token".to_string()).unwrap();
        
        let exported = flow.export_pairing_data().unwrap();
        assert!(!exported.is_empty());
        
        let imported = PairingFlow::import_pairing_data(&exported).unwrap();
        assert_eq!(imported.jid.user, "test");
        assert_eq!(imported.server_token, "test_token");
    }
}