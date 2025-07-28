/// Identity key management for Signal protocol

use crate::{
    error::{Error, Result},
    util::keys::SigningKeyPair,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Signal protocol identity key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityKey {
    pub public_key: [u8; 32],
}

impl IdentityKey {
    /// Create identity key from public key bytes
    pub fn new(public_key: [u8; 32]) -> Self {
        Self { public_key }
    }
    
    /// Get the public key bytes
    pub fn public_bytes(&self) -> [u8; 32] {
        self.public_key
    }
    
    /// Verify if this matches a signing key pair
    pub fn matches_keypair(&self, keypair: &SigningKeyPair) -> bool {
        self.public_key == keypair.public_bytes()
    }
}

impl From<&SigningKeyPair> for IdentityKey {
    fn from(keypair: &SigningKeyPair) -> Self {
        Self::new(keypair.public_bytes())
    }
}

/// Trust level for identity keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    /// Key is trusted (verified by user)
    Trusted,
    /// Key is untrusted but can be used
    Untrusted,
    /// Key should be blocked
    Blocked,
}

impl Default for TrustLevel {
    fn default() -> Self {
        TrustLevel::Untrusted
    }
}

/// Identity key record with trust information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityKeyRecord {
    pub identity_key: IdentityKey,
    pub trust_level: TrustLevel,
    pub timestamp: u64,
}

impl IdentityKeyRecord {
    /// Create a new identity key record
    pub fn new(identity_key: IdentityKey, trust_level: TrustLevel) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            identity_key,
            trust_level,
            timestamp,
        }
    }
    
    /// Check if this identity key is trusted
    pub fn is_trusted(&self) -> bool {
        self.trust_level == TrustLevel::Trusted
    }
    
    /// Check if this identity key should be blocked
    pub fn is_blocked(&self) -> bool {
        self.trust_level == TrustLevel::Blocked
    }
}

/// Identity key store trait for managing identity keys and trust
pub trait IdentityKeyStore {
    /// Get identity key pair for our own device
    fn get_identity_keypair(&self) -> Option<SigningKeyPair>;
    
    /// Get our local registration ID  
    fn get_local_registration_id(&self) -> u32;
    
    /// Save identity key for a remote address
    fn save_identity(&mut self, address: &str, identity_key: &IdentityKey) -> Result<bool>;
    
    /// Check if identity key is trusted for address
    fn is_trusted_identity(&self, address: &str, identity_key: &IdentityKey) -> bool;
    
    /// Get identity key for address
    fn get_identity(&self, address: &str) -> Option<IdentityKey>;
    
    /// Set trust level for an identity key
    fn set_trust_level(&mut self, address: &str, trust_level: TrustLevel) -> Result<()>;
}

/// In-memory identity key store implementation
#[derive(Debug)]
pub struct MemoryIdentityKeyStore {
    identity_keypair: SigningKeyPair,
    local_registration_id: u32,
    identity_keys: HashMap<String, IdentityKeyRecord>,
}

impl MemoryIdentityKeyStore {
    /// Create a new memory identity key store
    pub fn new(registration_id: u32) -> Self {
        Self {
            identity_keypair: SigningKeyPair::generate(),
            local_registration_id: registration_id,
            identity_keys: HashMap::new(),
        }
    }
    
    /// Create with existing identity keypair
    pub fn with_keypair(keypair: SigningKeyPair, registration_id: u32) -> Self {
        Self {
            identity_keypair: keypair,
            local_registration_id: registration_id,
            identity_keys: HashMap::new(),
        }
    }
}

impl IdentityKeyStore for MemoryIdentityKeyStore {
    fn get_identity_keypair(&self) -> Option<SigningKeyPair> {
        Some(self.identity_keypair.clone())
    }
    
    fn get_local_registration_id(&self) -> u32 {
        self.local_registration_id
    }
    
    fn save_identity(&mut self, address: &str, identity_key: &IdentityKey) -> Result<bool> {
        let existing = self.identity_keys.get(address);
        
        // Check if this is a new identity or changed identity
        let is_new_or_changed = match existing {
            Some(record) => record.identity_key.public_key != identity_key.public_key,
            None => true,
        };
        
        // Save the identity key
        let record = IdentityKeyRecord::new(identity_key.clone(), TrustLevel::Untrusted);
        self.identity_keys.insert(address.to_string(), record);
        
        Ok(is_new_or_changed)
    }
    
    fn is_trusted_identity(&self, address: &str, identity_key: &IdentityKey) -> bool {
        match self.identity_keys.get(address) {
            Some(record) => {
                // Key must match and not be blocked
                record.identity_key.public_key == identity_key.public_key 
                    && !record.is_blocked()
            },
            None => true, // Allow new identities
        }
    }
    
    fn get_identity(&self, address: &str) -> Option<IdentityKey> {
        self.identity_keys.get(address).map(|record| record.identity_key.clone())
    }
    
    fn set_trust_level(&mut self, address: &str, trust_level: TrustLevel) -> Result<()> {
        match self.identity_keys.get_mut(address) {
            Some(record) => {
                record.trust_level = trust_level;
                Ok(())
            },
            None => Err(Error::Protocol(format!("No identity key found for {}", address))),
        }
    }
}

/// Direction of the protocol exchange
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Sending,
    Receiving,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_identity_key_creation() {
        let keypair = SigningKeyPair::generate();
        let identity_key = IdentityKey::from(&keypair);
        
        assert_eq!(identity_key.public_bytes(), keypair.public_bytes());
        assert!(identity_key.matches_keypair(&keypair));
    }
    
    #[test]
    fn test_identity_key_record() {
        let keypair = SigningKeyPair::generate();
        let identity_key = IdentityKey::from(&keypair);
        let record = IdentityKeyRecord::new(identity_key, TrustLevel::Trusted);
        
        assert!(record.is_trusted());
        assert!(!record.is_blocked());
        assert!(record.timestamp > 0);
    }
    
    #[test]
    fn test_trust_levels() {
        assert_eq!(TrustLevel::default(), TrustLevel::Untrusted);
        
        let keypair = SigningKeyPair::generate();
        let identity_key = IdentityKey::from(&keypair);
        
        let trusted = IdentityKeyRecord::new(identity_key.clone(), TrustLevel::Trusted);
        let blocked = IdentityKeyRecord::new(identity_key.clone(), TrustLevel::Blocked);
        let untrusted = IdentityKeyRecord::new(identity_key, TrustLevel::Untrusted);
        
        assert!(trusted.is_trusted());
        assert!(blocked.is_blocked());
        assert!(!untrusted.is_trusted());
        assert!(!untrusted.is_blocked());
    }
    
    #[test]
    fn test_memory_identity_store() {
        let mut store = MemoryIdentityKeyStore::new(12345);
        let keypair = SigningKeyPair::generate();
        let identity_key = IdentityKey::from(&keypair);
        let address = "test@example.com";
        
        assert_eq!(store.get_local_registration_id(), 12345);
        assert!(store.get_identity_keypair().is_some());
        
        // Save new identity
        let is_new = store.save_identity(address, &identity_key).unwrap();
        assert!(is_new);
        
        // Should be trusted (new identity)
        assert!(store.is_trusted_identity(address, &identity_key));
        
        // Get identity back
        let retrieved = store.get_identity(address).unwrap();
        assert_eq!(retrieved.public_bytes(), identity_key.public_bytes());
        
        // Change trust level
        store.set_trust_level(address, TrustLevel::Blocked).unwrap();
        assert!(!store.is_trusted_identity(address, &identity_key));
        
        store.set_trust_level(address, TrustLevel::Trusted).unwrap();
        assert!(store.is_trusted_identity(address, &identity_key));
    }
    
    #[test]
    fn test_identity_key_change_detection() {
        let mut store = MemoryIdentityKeyStore::new(12345);
        let address = "test@example.com";
        
        let keypair1 = SigningKeyPair::generate();
        let identity1 = IdentityKey::from(&keypair1);
        
        let keypair2 = SigningKeyPair::generate();
        let identity2 = IdentityKey::from(&keypair2);
        
        // Save first identity
        assert!(store.save_identity(address, &identity1).unwrap());
        
        // Save same identity again - should not be new
        assert!(!store.save_identity(address, &identity1).unwrap());
        
        // Save different identity - should be new/changed
        assert!(store.save_identity(address, &identity2).unwrap());
    }
}