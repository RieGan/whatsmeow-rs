/// Pre-key bundle management for Signal protocol

use crate::{
    error::{Error, Result},
    util::keys::{ECKeyPair, SigningKeyPair},
};
use serde::{Deserialize, Serialize};

/// Signal protocol pre-key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreKey {
    pub id: u32,
    pub keypair: ECKeyPair,
}

impl PreKey {
    /// Generate a new pre-key with given ID
    pub fn generate(id: u32) -> Self {
        Self {
            id,
            keypair: ECKeyPair::generate(),
        }
    }
    
    /// Get the public key bytes
    pub fn public_key(&self) -> [u8; 32] {
        self.keypair.public_bytes()
    }
}

/// Signed pre-key for Signal protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPreKey {
    pub id: u32,
    pub keypair: ECKeyPair,
    pub signature: Vec<u8>,
    pub timestamp: u64,
}

impl SignedPreKey {
    /// Generate a new signed pre-key
    pub fn generate(id: u32, identity_keypair: &SigningKeyPair) -> Result<Self> {
        let keypair = ECKeyPair::generate();
        let public_key = keypair.public_bytes();
        
        // Sign the public key with identity key
        let signature = identity_keypair.signing_key()
            .sign(&public_key)
            .to_bytes()
            .to_vec();
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Ok(Self {
            id,
            keypair,
            signature,
            timestamp,
        })
    }
    
    /// Verify the signature on this signed pre-key
    pub fn verify_signature(&self, identity_public_key: &[u8; 32]) -> Result<bool> {
        use ed25519_dalek::{VerifyingKey, Signature};
        
        let verifying_key = VerifyingKey::from_bytes(identity_public_key)
            .map_err(|_| Error::Crypto("Invalid identity public key".to_string()))?;
        
        let signature = Signature::from_slice(&self.signature)
            .map_err(|_| Error::Crypto("Invalid signature format".to_string()))?;
        
        let public_key = self.keypair.public_bytes();
        
        match verifying_key.verify_strict(&public_key, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
    
    /// Get the public key bytes
    pub fn public_key(&self) -> [u8; 32] {
        self.keypair.public_bytes()
    }
}

/// Pre-key bundle containing all necessary keys for Signal session initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreKeyBundle {
    pub identity_key: Vec<u8>,
    pub signed_prekey: SignedPreKey,
    pub prekey: Option<PreKey>,
    pub registration_id: u32,
    pub device_id: u32,
}

impl PreKeyBundle {
    /// Create a new pre-key bundle
    pub fn new(
        identity_keypair: &SigningKeyPair,
        signed_prekey_id: u32,
        prekey_id: Option<u32>,
        registration_id: u32,
        device_id: u32,
    ) -> Result<Self> {
        let identity_key = identity_keypair.public_bytes().to_vec();
        let signed_prekey = SignedPreKey::generate(signed_prekey_id, identity_keypair)?;
        let prekey = prekey_id.map(PreKey::generate);
        
        Ok(Self {
            identity_key,
            signed_prekey,
            prekey,
            registration_id,
            device_id,
        })
    }
    
    /// Validate the pre-key bundle
    pub fn validate(&self) -> Result<()> {
        // Verify signed pre-key signature
        if self.identity_key.len() != 32 {
            return Err(Error::Crypto("Invalid identity key length".to_string()));
        }
        
        let identity_key: [u8; 32] = self.identity_key.as_slice().try_into()
            .map_err(|_| Error::Crypto("Failed to convert identity key".to_string()))?;
        
        if !self.signed_prekey.verify_signature(&identity_key)? {
            return Err(Error::Crypto("Invalid signed pre-key signature".to_string()));
        }
        
        Ok(())
    }
}

/// Pre-key store for managing pre-keys
pub trait PreKeyStore {
    /// Load a pre-key by ID
    fn load_prekey(&self, prekey_id: u32) -> Option<PreKey>;
    
    /// Store a pre-key
    fn store_prekey(&mut self, prekey: PreKey);
    
    /// Remove a pre-key
    fn remove_prekey(&mut self, prekey_id: u32);
    
    /// Load signed pre-key by ID
    fn load_signed_prekey(&self, signed_prekey_id: u32) -> Option<SignedPreKey>;
    
    /// Store signed pre-key
    fn store_signed_prekey(&mut self, signed_prekey: SignedPreKey);
    
    /// Get all signed pre-key IDs
    fn load_signed_prekey_ids(&self) -> Vec<u32>;
}

/// In-memory pre-key store implementation
#[derive(Debug, Default)]
pub struct MemoryPreKeyStore {
    prekeys: std::collections::HashMap<u32, PreKey>,
    signed_prekeys: std::collections::HashMap<u32, SignedPreKey>,
}

impl MemoryPreKeyStore {
    /// Create a new memory pre-key store
    pub fn new() -> Self {
        Self::default()
    }
}

impl PreKeyStore for MemoryPreKeyStore {
    fn load_prekey(&self, prekey_id: u32) -> Option<PreKey> {
        self.prekeys.get(&prekey_id).cloned()
    }
    
    fn store_prekey(&mut self, prekey: PreKey) {
        self.prekeys.insert(prekey.id, prekey);
    }
    
    fn remove_prekey(&mut self, prekey_id: u32) {
        self.prekeys.remove(&prekey_id);
    }
    
    fn load_signed_prekey(&self, signed_prekey_id: u32) -> Option<SignedPreKey> {
        self.signed_prekeys.get(&signed_prekey_id).cloned()
    }
    
    fn store_signed_prekey(&mut self, signed_prekey: SignedPreKey) {
        self.signed_prekeys.insert(signed_prekey.id, signed_prekey);
    }
    
    fn load_signed_prekey_ids(&self) -> Vec<u32> {
        self.signed_prekeys.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_prekey_generation() {
        let prekey = PreKey::generate(1);
        assert_eq!(prekey.id, 1);
        assert_eq!(prekey.public_key().len(), 32);
    }
    
    #[test]
    fn test_signed_prekey_generation() {
        let identity_keypair = SigningKeyPair::generate();
        let signed_prekey = SignedPreKey::generate(1, &identity_keypair).unwrap();
        
        assert_eq!(signed_prekey.id, 1);
        assert!(!signed_prekey.signature.is_empty());
        assert!(signed_prekey.timestamp > 0);
    }
    
    #[test]
    fn test_signed_prekey_verification() {
        let identity_keypair = SigningKeyPair::generate();
        let signed_prekey = SignedPreKey::generate(1, &identity_keypair).unwrap();
        
        // Should verify with correct identity key
        let identity_public = identity_keypair.public_bytes();
        assert!(signed_prekey.verify_signature(&identity_public).unwrap());
        
        // Should not verify with wrong identity key
        let wrong_keypair = SigningKeyPair::generate();
        let wrong_public = wrong_keypair.public_bytes();
        assert!(!signed_prekey.verify_signature(&wrong_public).unwrap());
    }
    
    #[test]
    fn test_prekey_bundle() {
        let identity_keypair = SigningKeyPair::generate();
        let bundle = PreKeyBundle::new(&identity_keypair, 1, Some(2), 12345, 1).unwrap();
        
        assert_eq!(bundle.signed_prekey.id, 1);
        assert_eq!(bundle.prekey.as_ref().unwrap().id, 2);
        assert_eq!(bundle.registration_id, 12345);
        assert_eq!(bundle.device_id, 1);
        
        // Should validate successfully
        assert!(bundle.validate().is_ok());
    }
    
    #[test]
    fn test_memory_prekey_store() {
        let mut store = MemoryPreKeyStore::new();
        let prekey = PreKey::generate(1);
        let identity_keypair = SigningKeyPair::generate();
        let signed_prekey = SignedPreKey::generate(1, &identity_keypair).unwrap();
        
        // Store keys
        store.store_prekey(prekey.clone());
        store.store_signed_prekey(signed_prekey.clone());
        
        // Load keys
        assert_eq!(store.load_prekey(1).unwrap().id, prekey.id);
        assert_eq!(store.load_signed_prekey(1).unwrap().id, signed_prekey.id);
        
        // Remove key
        store.remove_prekey(1);
        assert!(store.load_prekey(1).is_none());
    }
}