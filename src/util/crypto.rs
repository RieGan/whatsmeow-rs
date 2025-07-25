use crate::error::{Error, Result};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use hkdf::Hkdf;
use ring::digest;
use sha2::Sha256;

#[cfg(test)]
mod tests;

/// AES-GCM encryption utility
pub struct AesGcm {
    cipher: Aes256Gcm,
}

impl AesGcm {
    /// Create a new AES-GCM cipher with the given key
    pub fn new(key: &[u8]) -> Result<Self> {
        if key.len() != 32 {
            return Err(Error::Crypto("AES key must be 32 bytes".to_string()));
        }
        
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| Error::Crypto(format!("Failed to create AES cipher: {}", e)))?;
        
        Ok(Self { cipher })
    }
    
    /// Encrypt data with the given nonce
    pub fn encrypt(&self, nonce: &[u8], data: &[u8]) -> Result<Vec<u8>> {
        if nonce.len() != 12 {
            return Err(Error::Crypto("Nonce must be 12 bytes".to_string()));
        }
        
        let nonce = Nonce::from_slice(nonce);
        self.cipher
            .encrypt(nonce, data)
            .map_err(|e| Error::Crypto(format!("Encryption failed: {}", e)))
    }
    
    /// Decrypt data with the given nonce
    pub fn decrypt(&self, nonce: &[u8], data: &[u8]) -> Result<Vec<u8>> {
        if nonce.len() != 12 {
            return Err(Error::Crypto("Nonce must be 12 bytes".to_string()));
        }
        
        let nonce = Nonce::from_slice(nonce);
        self.cipher
            .decrypt(nonce, data)
            .map_err(|e| Error::Crypto(format!("Decryption failed: {}", e)))
    }
}

/// HKDF key derivation
pub fn hkdf_expand(key: &[u8], info: &[u8], length: usize) -> Result<Vec<u8>> {
    let hk = Hkdf::<Sha256>::new(None, key);
    let mut output = vec![0u8; length];
    hk.expand(info, &mut output)
        .map_err(|e| Error::Crypto(format!("HKDF expansion failed: {}", e)))?;
    Ok(output)
}

/// SHA-256 hash
pub fn sha256(data: &[u8]) -> Vec<u8> {
    digest::digest(&digest::SHA256, data).as_ref().to_vec()
}

/// Generate random bytes
pub fn random_bytes(length: usize) -> Vec<u8> {
    use ring::rand::{SecureRandom, SystemRandom};
    
    let rng = SystemRandom::new();
    let mut bytes = vec![0u8; length];
    rng.fill(&mut bytes).unwrap();
    bytes
}