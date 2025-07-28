/// Authentication and device registration for WhatsApp multi-device protocol

pub mod pairing;
pub mod multidevice;

use crate::{
    error::{Error, Result},
    types::JID,
    util::keys::{ECKeyPair, SigningKeyPair},
};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use serde::{Deserialize, Serialize};

pub use pairing::{
    PairingFlow, PairingMethod, PairingState, PairingChallenge,
    DeviceInfo, DeviceCapabilities, DeviceRegistration,
    PairingKeys, PairingKeysData, PreKeyBundleData,
};

pub use multidevice::{
    MultiDeviceManager, DeviceSession, DeviceType, DeviceStatus,
    DeviceAnnouncement, MultiDeviceConfig,
};

/// QR code data for WhatsApp authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QRData {
    pub ref_id: String,
    pub public_key: Vec<u8>,
    pub adv_secret: Vec<u8>,
}

impl QRData {
    /// Generate a new QR code for authentication
    pub fn generate() -> Self {
        let ref_id = uuid::Uuid::new_v4().to_string();
        let keypair = ECKeyPair::generate();
        let public_key = keypair.public_bytes().to_vec();
        let adv_secret = crate::util::crypto::random_bytes(32);
        
        Self {
            ref_id,
            public_key,
            adv_secret,
        }
    }
    
    /// Encode QR data as a string that can be displayed as QR code
    pub fn encode(&self) -> String {
        let data = format!("{},{},{}", 
            self.ref_id,
            STANDARD_NO_PAD.encode(&self.public_key),
            STANDARD_NO_PAD.encode(&self.adv_secret)
        );
        data
    }
    
    /// Decode QR data from string
    pub fn decode(data: &str) -> Result<Self> {
        let parts: Vec<&str> = data.split(',').collect();
        if parts.len() != 3 {
            return Err(Error::Protocol("Invalid QR data format".to_string()));
        }
        
        let ref_id = parts[0].to_string();
        let public_key = STANDARD_NO_PAD.decode(parts[1])
            .map_err(|e| Error::Protocol(format!("Invalid public key: {}", e)))?;
        let adv_secret = STANDARD_NO_PAD.decode(parts[2])
            .map_err(|e| Error::Protocol(format!("Invalid ADV secret: {}", e)))?;
            
        Ok(Self {
            ref_id,
            public_key,
            adv_secret,
        })
    }
}

/// Legacy device registration information (for backward compatibility)
#[derive(Debug, Clone)]
pub struct LegacyDeviceRegistration {
    pub jid: JID,
    pub registration_id: u32,
    pub noise_keypair: ECKeyPair,
    pub identity_keypair: SigningKeyPair,
    pub signed_pre_key: SigningKeyPair,
    pub signed_pre_key_id: u32,
    pub signed_pre_key_signature: Vec<u8>,
}

impl LegacyDeviceRegistration {
    /// Generate a new device registration
    pub fn generate() -> Self {
        let noise_keypair = ECKeyPair::generate();
        let identity_keypair = SigningKeyPair::generate();
        let signed_pre_key = SigningKeyPair::generate();
        let signed_pre_key_id = rand::random::<u32>();
        
        // Sign the pre key with the identity key
        use ed25519_dalek::Signer;
        let signed_pre_key_signature = identity_keypair.signing_key()
            .sign(&signed_pre_key.public_bytes())
            .to_bytes()
            .to_vec();
        
        Self {
            jid: JID::new("placeholder".to_string(), "s.whatsapp.net".to_string()),
            registration_id: rand::random::<u32>(),
            noise_keypair,
            identity_keypair,
            signed_pre_key,
            signed_pre_key_id,
            signed_pre_key_signature,
        }
    }
}

/// Authentication state
#[derive(Debug, Clone)]
pub enum AuthState {
    /// Not authenticated, need to scan QR code
    Unauthenticated,
    /// QR code generated and waiting for scan
    QRGenerated(QRData),
    /// QR code scanned, waiting for confirmation
    QRScanned,
    /// Successfully authenticated (legacy)
    Authenticated(LegacyDeviceRegistration),
    /// Successfully authenticated (new multi-device)
    AuthenticatedMultiDevice(DeviceRegistration),
    /// Authentication failed
    Failed(String),
}

/// Enhanced authentication manager with multi-device support
pub struct AuthManager {
    state: AuthState,
    pairing_flow: Option<PairingFlow>,
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new() -> Self {
        Self {
            state: AuthState::Unauthenticated,
            pairing_flow: None,
        }
    }
    
    /// Get current authentication state
    pub fn state(&self) -> &AuthState {
        &self.state
    }
    
    /// Start pairing flow with specified method
    pub fn start_pairing(&mut self, method: PairingMethod) -> Result<()> {
        self.pairing_flow = Some(PairingFlow::new(method));
        Ok(())
    }
    
    /// Start pairing flow with custom device info
    pub fn start_pairing_with_device_info(&mut self, method: PairingMethod, device_info: DeviceInfo) -> Result<()> {
        self.pairing_flow = Some(PairingFlow::with_device_info(method, device_info));
        Ok(())
    }
    
    /// Generate a QR code for authentication
    pub fn generate_qr(&mut self) -> Result<String> {
        // Start QR pairing if not already started
        if self.pairing_flow.is_none() {
            self.start_pairing(PairingMethod::QRCode)?;
        }
        
        let pairing_flow = self.pairing_flow.as_ref()
            .ok_or_else(|| Error::Auth("No pairing flow active".to_string()))?;
        
        let qr_string = pairing_flow.generate_qr_data()?;
        
        // Also maintain legacy QR data for backward compatibility
        let qr_data = QRData::generate();
        let _legacy_qr_string = qr_data.encode();
        self.state = AuthState::QRGenerated(qr_data);
        
        // Return the new pairing QR data
        Ok(qr_string)
    }
    
    /// Handle QR code scan response
    pub fn handle_qr_scan(&mut self, response_data: &[u8]) -> Result<()> {
        if let Some(pairing_flow) = &mut self.pairing_flow {
            pairing_flow.verify_challenge(response_data)?;
            self.state = AuthState::QRScanned;
            Ok(())
        } else {
            // Legacy handling
            self.state = AuthState::QRScanned;
            Ok(())
        }
    }
    
    /// Handle phone number verification
    pub fn handle_phone_verification(&mut self, phone: &str, verification_code: &str) -> Result<()> {
        if let Some(pairing_flow) = &mut self.pairing_flow {
            pairing_flow.handle_phone_verification(phone, verification_code)?;
            Ok(())
        } else {
            Err(Error::Auth("No phone pairing flow active".to_string()))
        }
    }
    
    /// Complete authentication process with multi-device registration
    pub fn complete_auth(&mut self, jid: JID, server_token: String) -> Result<DeviceRegistration> {
        if let Some(pairing_flow) = &mut self.pairing_flow {
            let registration = pairing_flow.complete_registration(jid, server_token)?;
            self.state = AuthState::AuthenticatedMultiDevice(registration.clone());
            Ok(registration)
        } else {
            Err(Error::Auth("No pairing flow to complete".to_string()))
        }
    }
    
    /// Complete authentication process (legacy)
    pub fn complete_auth_legacy(&mut self, registration: LegacyDeviceRegistration) {
        self.state = AuthState::Authenticated(registration);
    }
    
    /// Mark authentication as failed
    pub fn mark_failed(&mut self, reason: String) {
        self.state = AuthState::Failed(reason);
        self.pairing_flow = None;
    }
    
    /// Export authentication data for backup
    pub fn export_auth_data(&self) -> Result<String> {
        match &self.state {
            AuthState::AuthenticatedMultiDevice(_) => {
                if let Some(pairing_flow) = &self.pairing_flow {
                    pairing_flow.export_pairing_data()
                } else {
                    Err(Error::Auth("No pairing data to export".to_string()))
                }
            }
            _ => Err(Error::Auth("Device not authenticated with multi-device support".to_string()))
        }
    }
    
    /// Import authentication data from backup
    pub fn import_auth_data(&mut self, data: &str) -> Result<()> {
        let registration = PairingFlow::import_pairing_data(data)?;
        self.state = AuthState::AuthenticatedMultiDevice(registration);
        Ok(())
    }
    
    /// Check if authenticated with multi-device support
    pub fn is_multi_device_authenticated(&self) -> bool {
        matches!(self.state, AuthState::AuthenticatedMultiDevice(_))
    }
    
    /// Check if authenticated (any method)
    pub fn is_authenticated(&self) -> bool {
        matches!(self.state, AuthState::Authenticated(_) | AuthState::AuthenticatedMultiDevice(_))
    }
    
    /// Get device registration if available
    pub fn get_device_registration(&self) -> Option<&DeviceRegistration> {
        match &self.state {
            AuthState::AuthenticatedMultiDevice(reg) => Some(reg),
            _ => None,
        }
    }
    
    /// Get legacy device registration if available
    pub fn get_legacy_device_registration(&self) -> Option<&LegacyDeviceRegistration> {
        match &self.state {
            AuthState::Authenticated(reg) => Some(reg),
            _ => None,
        }
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_qr_data_encode_decode() {
        let qr_data = QRData::generate();
        let encoded = qr_data.encode();
        let decoded = QRData::decode(&encoded).unwrap();
        
        assert_eq!(qr_data.ref_id, decoded.ref_id);
        assert_eq!(qr_data.public_key, decoded.public_key);
        assert_eq!(qr_data.adv_secret, decoded.adv_secret);
    }
    
    #[test]
    fn test_auth_manager_qr_flow() {
        let mut auth_manager = AuthManager::new();
        
        // Generate QR code
        let qr_code = auth_manager.generate_qr().unwrap();
        assert!(!qr_code.is_empty());
        
        // Simulate QR scan
        let response = vec![1, 2, 3, 4]; // Mock response
        // Note: This will fail challenge verification, but tests the flow
        let _ = auth_manager.handle_qr_scan(&response);
    }
    
    #[test]
    fn test_auth_manager_phone_flow() {
        let mut auth_manager = AuthManager::new();
        let phone = "+1234567890".to_string();
        
        auth_manager.start_pairing(PairingMethod::PhoneNumber(phone.clone())).unwrap();
        auth_manager.handle_phone_verification(&phone, "123456").unwrap();
    }
    
    #[test]
    fn test_legacy_device_registration() {
        let registration = LegacyDeviceRegistration::generate();
        assert_ne!(registration.registration_id, 0);
        assert!(!registration.signed_pre_key_signature.is_empty());
    }
}