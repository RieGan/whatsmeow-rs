use crate::error::{Error, Result};
use ed25519_dalek::{SigningKey, VerifyingKey};
use curve25519_dalek::{constants, scalar::Scalar, montgomery::MontgomeryPoint};

#[cfg(test)]
mod tests;

/// Elliptic curve key pair for X25519  
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ECKeyPair {
    pub private_key: [u8; 32],
    pub public_key: [u8; 32],
}

impl ECKeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut private_key = [0u8; 32];
        rng.fill_bytes(&mut private_key);
        
        // Clamp the private key for X25519
        private_key[0] &= 248;
        private_key[31] &= 127;
        private_key[31] |= 64;
        
        // Derive public key using curve25519-dalek
        let scalar = Scalar::from_bytes_mod_order(private_key);
        let public_point = &scalar * &constants::X25519_BASEPOINT;
        let public_key = public_point.to_bytes();
        
        Self {
            private_key,
            public_key,
        }
    }
    
    /// Create from private key bytes
    pub fn from_private_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(Error::Crypto("Private key must be 32 bytes".to_string()));
        }
        
        let mut private_key = [0u8; 32];
        private_key.copy_from_slice(bytes);
        
        // Clamp the private key for X25519
        private_key[0] &= 248;
        private_key[31] &= 127;
        private_key[31] |= 64;
        
        // Derive public key using curve25519-dalek
        let scalar = Scalar::from_bytes_mod_order(private_key);
        let public_point = &scalar * &constants::X25519_BASEPOINT;
        let public_key = public_point.to_bytes();
        
        Ok(Self {
            private_key,
            public_key,
        })
    }
    
    /// Get private key bytes
    pub fn private_bytes(&self) -> [u8; 32] {
        self.private_key
    }
    
    /// Get public key bytes
    pub fn public_bytes(&self) -> [u8; 32] {
        self.public_key
    }
    
    /// Perform ECDH using curve25519-dalek
    pub fn ecdh(&self, other_public: &[u8; 32]) -> [u8; 32] {
        // Create scalar from our private key
        let our_scalar = Scalar::from_bytes_mod_order(self.private_key);
        
        // Create Montgomery point from other party's public key
        let other_point = MontgomeryPoint(*other_public);
        
        // Perform scalar multiplication (ECDH)
        let shared_point = &our_scalar * &other_point;
        
        shared_point.to_bytes()
    }
    
    /// Perform ECDH with raw bytes
    pub fn ecdh_bytes(&self, other_public_bytes: &[u8; 32]) -> Result<[u8; 32]> {
        Ok(self.ecdh(other_public_bytes))
    }
}

/// Ed25519 signing key pair
#[derive(Debug, Clone)]
pub struct SigningKeyPair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl SigningKeyPair {
    /// Generate a new random signing key pair
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut secret_key_bytes = [0u8; 32];
        rng.fill_bytes(&mut secret_key_bytes);
        
        let signing_key = SigningKey::from_bytes(&secret_key_bytes);
        let verifying_key = signing_key.verifying_key();
        
        Self {
            signing_key,
            verifying_key,
        }
    }
    
    /// Create from private key bytes
    pub fn from_private_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(Error::Crypto("Signing key must be 32 bytes".to_string()));
        }
        
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(bytes);
        
        let signing_key = SigningKey::from_bytes(&key_bytes);
        let verifying_key = signing_key.verifying_key();
        
        Ok(Self {
            signing_key,
            verifying_key,
        })
    }
    
    /// Get private key bytes  
    pub fn private_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }
    
    /// Get public key bytes
    pub fn public_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }
    
    /// Get access to the signing key
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }
    
    /// Get access to the verifying key
    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }
}