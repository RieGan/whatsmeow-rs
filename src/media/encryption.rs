/// Media-specific encryption and decryption for WhatsApp

use crate::{
    error::{Error, Result},
    util::crypto::{sha256, AesGcm, random_bytes},
};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

/// Media encryption key derivation constants
const MEDIA_KEY_EXPANSION: &[u8] = b"WhatsApp Media Keys";
const MEDIA_IV_EXPANSION: &[u8] = b"WhatsApp Media IVs";
const MEDIA_MAC_EXPANSION: &[u8] = b"WhatsApp Media MACs";

/// Media encryption type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MediaEncryptionType {
    /// Standard AES-256-CBC encryption
    AesCbc,
    /// AES-256-GCM encryption with authentication
    AesGcm,
    /// ChaCha20-Poly1305 encryption
    ChaCha20Poly1305,
}

impl Default for MediaEncryptionType {
    fn default() -> Self {
        MediaEncryptionType::AesCbc
    }
}

/// Media encryption context
#[derive(Debug, Clone)]
pub struct MediaEncryptionContext {
    /// Encryption type to use
    pub encryption_type: MediaEncryptionType,
    /// Enable compression before encryption
    pub enable_compression: bool,
    /// Compression level (0-9)
    pub compression_level: u32,
}

impl Default for MediaEncryptionContext {
    fn default() -> Self {
        Self {
            encryption_type: MediaEncryptionType::default(),
            enable_compression: true,
            compression_level: 6,
        }
    }
}

/// Encrypted media data container
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncryptedMediaData {
    /// Encrypted data bytes
    pub encrypted_data: Vec<u8>,
    /// Media encryption key (32 bytes)
    pub media_key: Vec<u8>,
    /// Initialization vector/nonce
    pub iv: Vec<u8>,
    /// Authentication tag (for authenticated encryption)
    pub auth_tag: Option<Vec<u8>>,
    /// Original data size (before compression/encryption)
    pub original_size: u64,
    /// Compressed size (if compression was used)
    pub compressed_size: Option<u64>,
    /// SHA256 hash of original data
    pub original_hash: Vec<u8>,
    /// SHA256 hash of encrypted data
    pub encrypted_hash: Vec<u8>,
    /// Encryption type used
    pub encryption_type: MediaEncryptionType,
    /// Whether data was compressed
    pub was_compressed: bool,
}

impl EncryptedMediaData {
    /// Validate the encrypted media data structure
    pub fn validate(&self) -> bool {
        !self.encrypted_data.is_empty() &&
        self.media_key.len() == 32 &&
        !self.iv.is_empty() &&
        !self.original_hash.is_empty() &&
        self.original_hash.len() == 32 &&
        !self.encrypted_hash.is_empty() &&
        self.encrypted_hash.len() == 32 &&
        self.original_size > 0
    }
}

/// Media encryption service
pub struct MediaEncryption {
    context: MediaEncryptionContext,
}

impl MediaEncryption {
    /// Create new media encryption service
    pub fn new(context: MediaEncryptionContext) -> Self {
        Self { context }
    }
    
    /// Create with default settings
    pub fn default() -> Self {
        Self::new(MediaEncryptionContext::default())
    }
    
    /// Encrypt media data
    pub fn encrypt(&self, data: &[u8]) -> Result<EncryptedMediaData> {
        if data.is_empty() {
            return Err(Error::Crypto("Cannot encrypt empty data".to_string()));
        }
        
        let original_size = data.len() as u64;
        let original_hash = sha256(data);
        
        // Optionally compress data
        let (data_to_encrypt, compressed_size, was_compressed) = if self.context.enable_compression {
            match self.compress_data(data) {
                Ok(compressed) => {
                    let compressed_size = compressed.len() as u64;
                    // Only use compression if it actually reduces size significantly
                    if compressed_size < (original_size * 90 / 100) {
                        (compressed, Some(compressed_size), true)
                    } else {
                        (data.to_vec(), None, false)
                    }
                },
                Err(_) => (data.to_vec(), None, false),
            }
        } else {
            (data.to_vec(), None, false)
        };
        
        // Generate media key and IV
        let media_key = self.generate_media_key();
        let iv = self.generate_iv(&self.context.encryption_type);
        
        // Encrypt data based on type
        let (encrypted_data, auth_tag) = match self.context.encryption_type {
            MediaEncryptionType::AesCbc => {
                let encrypted = self.encrypt_aes_cbc(&data_to_encrypt, &media_key, &iv)?;
                (encrypted, None)
            },
            MediaEncryptionType::AesGcm => {
                let (encrypted, tag) = self.encrypt_aes_gcm(&data_to_encrypt, &media_key, &iv)?;
                (encrypted, Some(tag))
            },
            MediaEncryptionType::ChaCha20Poly1305 => {
                let (encrypted, tag) = self.encrypt_chacha20_poly1305(&data_to_encrypt, &media_key, &iv)?;
                (encrypted, Some(tag))
            },
        };
        
        let encrypted_hash = sha256(&encrypted_data);
        
        Ok(EncryptedMediaData {
            encrypted_data,
            media_key,
            iv,
            auth_tag,
            original_size,
            compressed_size,
            original_hash,
            encrypted_hash,
            encryption_type: self.context.encryption_type.clone(),
            was_compressed,
        })
    }
    
    /// Decrypt media data
    pub fn decrypt(&self, encrypted_media: &EncryptedMediaData) -> Result<Vec<u8>> {
        // Validate encrypted media data
        if !encrypted_media.validate() {
            return Err(Error::Crypto("Invalid encrypted media data".to_string()));
        }
        
        // Verify encrypted data hash
        let encrypted_hash = sha256(&encrypted_media.encrypted_data);
        if encrypted_hash != encrypted_media.encrypted_hash {
            return Err(Error::Crypto("Encrypted data hash mismatch".to_string()));
        }
        
        // Decrypt data based on type
        let decrypted_data = match encrypted_media.encryption_type {
            MediaEncryptionType::AesCbc => {
                self.decrypt_aes_cbc(
                    &encrypted_media.encrypted_data,
                    &encrypted_media.media_key,
                    &encrypted_media.iv,
                )?
            },
            MediaEncryptionType::AesGcm => {
                let auth_tag = encrypted_media.auth_tag.as_ref()
                    .ok_or_else(|| Error::Crypto("Missing auth tag for AES-GCM".to_string()))?;
                self.decrypt_aes_gcm(
                    &encrypted_media.encrypted_data,
                    &encrypted_media.media_key,
                    &encrypted_media.iv,
                    auth_tag,
                )?
            },
            MediaEncryptionType::ChaCha20Poly1305 => {
                let auth_tag = encrypted_media.auth_tag.as_ref()
                    .ok_or_else(|| Error::Crypto("Missing auth tag for ChaCha20-Poly1305".to_string()))?;
                self.decrypt_chacha20_poly1305(
                    &encrypted_media.encrypted_data,
                    &encrypted_media.media_key,
                    &encrypted_media.iv,
                    auth_tag,
                )?
            },
        };
        
        // Decompress if needed
        let final_data = if encrypted_media.was_compressed {
            self.decompress_data(&decrypted_data)?
        } else {
            decrypted_data
        };
        
        // Verify original data hash
        let original_hash = sha256(&final_data);
        if original_hash != encrypted_media.original_hash {
            return Err(Error::Crypto("Original data hash mismatch".to_string()));
        }
        
        // Verify size
        if final_data.len() as u64 != encrypted_media.original_size {
            return Err(Error::Crypto("Decrypted data size mismatch".to_string()));
        }
        
        Ok(final_data)
    }
    
    /// Generate media encryption key
    fn generate_media_key(&self) -> Vec<u8> {
        random_bytes(32)
    }
    
    /// Generate IV based on encryption type
    fn generate_iv(&self, encryption_type: &MediaEncryptionType) -> Vec<u8> {
        match encryption_type {
            MediaEncryptionType::AesCbc => random_bytes(16),
            MediaEncryptionType::AesGcm => random_bytes(12),
            MediaEncryptionType::ChaCha20Poly1305 => random_bytes(12),
        }
    }
    
    /// Encrypt using AES-256-CBC
    fn encrypt_aes_cbc(&self, data: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
        if key.len() != 32 {
            return Err(Error::Crypto("AES key must be 32 bytes".to_string()));
        }
        if iv.len() != 16 {
            return Err(Error::Crypto("AES-CBC IV must be 16 bytes".to_string()));
        }
        
        // For simplicity, use AES-GCM with zero nonce for CBC simulation
        // In a real implementation, you'd use proper AES-CBC
        let aes = AesGcm::new(key.try_into().unwrap())?;
        let nonce: [u8; 12] = iv[0..12].try_into().unwrap();
        
        aes.encrypt(&nonce, data)
    }
    
    /// Decrypt using AES-256-CBC
    fn decrypt_aes_cbc(&self, encrypted_data: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
        if key.len() != 32 {
            return Err(Error::Crypto("AES key must be 32 bytes".to_string()));
        }
        if iv.len() != 16 {
            return Err(Error::Crypto("AES-CBC IV must be 16 bytes".to_string()));
        }
        
        let aes = AesGcm::new(key.try_into().unwrap())?;
        let nonce: [u8; 12] = iv[0..12].try_into().unwrap();
        
        aes.decrypt(&nonce, encrypted_data)
    }
    
    /// Encrypt using AES-256-GCM
    fn encrypt_aes_gcm(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        if key.len() != 32 {
            return Err(Error::Crypto("AES key must be 32 bytes".to_string()));
        }
        if nonce.len() != 12 {
            return Err(Error::Crypto("AES-GCM nonce must be 12 bytes".to_string()));
        }
        
        let aes = AesGcm::new(key.try_into().unwrap())?;
        let nonce_array: [u8; 12] = nonce.try_into().unwrap();
        
        // Our AesGcm implementation returns encrypted data with tag appended
        let encrypted_with_tag = aes.encrypt(&nonce_array, data)?;
        
        // Split encrypted data and tag (last 16 bytes)
        if encrypted_with_tag.len() < 16 {
            return Err(Error::Crypto("Invalid encrypted data length".to_string()));
        }
        
        let split_point = encrypted_with_tag.len() - 16;
        let encrypted_data = encrypted_with_tag[..split_point].to_vec();
        let tag = encrypted_with_tag[split_point..].to_vec();
        
        Ok((encrypted_data, tag))
    }
    
    /// Decrypt using AES-256-GCM
    fn decrypt_aes_gcm(&self, encrypted_data: &[u8], key: &[u8], nonce: &[u8], tag: &[u8]) -> Result<Vec<u8>> {
        if key.len() != 32 {
            return Err(Error::Crypto("AES key must be 32 bytes".to_string()));
        }
        if nonce.len() != 12 {
            return Err(Error::Crypto("AES-GCM nonce must be 12 bytes".to_string()));
        }
        if tag.len() != 16 {
            return Err(Error::Crypto("AES-GCM tag must be 16 bytes".to_string()));
        }
        
        let aes = AesGcm::new(key.try_into().unwrap())?;
        let nonce_array: [u8; 12] = nonce.try_into().unwrap();
        
        // Combine encrypted data and tag for decryption
        let mut encrypted_with_tag = encrypted_data.to_vec();
        encrypted_with_tag.extend_from_slice(tag);
        
        aes.decrypt(&nonce_array, &encrypted_with_tag)
    }
    
    /// Encrypt using ChaCha20-Poly1305 (placeholder implementation)
    fn encrypt_chacha20_poly1305(&self, data: &[u8], key: &[u8], nonce: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        if key.len() != 32 {
            return Err(Error::Crypto("ChaCha20 key must be 32 bytes".to_string()));
        }
        if nonce.len() != 12 {
            return Err(Error::Crypto("ChaCha20-Poly1305 nonce must be 12 bytes".to_string()));
        }
        
        // For now, use AES-GCM as a placeholder
        // In a real implementation, you'd use proper ChaCha20-Poly1305
        self.encrypt_aes_gcm(data, key, nonce)
    }
    
    /// Decrypt using ChaCha20-Poly1305 (placeholder implementation)
    fn decrypt_chacha20_poly1305(&self, encrypted_data: &[u8], key: &[u8], nonce: &[u8], tag: &[u8]) -> Result<Vec<u8>> {
        if key.len() != 32 {
            return Err(Error::Crypto("ChaCha20 key must be 32 bytes".to_string()));
        }
        if nonce.len() != 12 {
            return Err(Error::Crypto("ChaCha20-Poly1305 nonce must be 12 bytes".to_string()));
        }
        
        // For now, use AES-GCM as a placeholder
        self.decrypt_aes_gcm(encrypted_data, key, nonce, tag)
    }
    
    /// Compress data using deflate
    fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        use flate2::{Compression, write::ZlibEncoder};
        use std::io::Write;
        
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(self.context.compression_level));
        encoder.write_all(data)
            .map_err(|e| Error::Crypto(format!("Compression failed: {}", e)))?;
        
        encoder.finish()
            .map_err(|e| Error::Crypto(format!("Compression finish failed: {}", e)))
    }
    
    /// Decompress data using deflate
    fn decompress_data(&self, compressed_data: &[u8]) -> Result<Vec<u8>> {
        use flate2::read::ZlibDecoder;
        use std::io::Read;
        
        let mut decoder = ZlibDecoder::new(compressed_data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)
            .map_err(|e| Error::Crypto(format!("Decompression failed: {}", e)))?;
        
        Ok(decompressed)
    }
    
    /// Derive media keys using HKDF-like expansion
    pub fn derive_media_keys(master_key: &[u8], info: &[u8]) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        if master_key.len() != 32 {
            return Err(Error::Crypto("Master key must be 32 bytes".to_string()));
        }
        
        // Derive encryption key
        let mut enc_key_input = Vec::new();
        enc_key_input.extend_from_slice(master_key);
        enc_key_input.extend_from_slice(MEDIA_KEY_EXPANSION);
        enc_key_input.extend_from_slice(info);
        let enc_key = sha256(&enc_key_input)[0..32].to_vec();
        
        // Derive IV
        let mut iv_input = Vec::new();
        iv_input.extend_from_slice(master_key);
        iv_input.extend_from_slice(MEDIA_IV_EXPANSION);
        iv_input.extend_from_slice(info);
        let iv = sha256(&iv_input)[0..16].to_vec();
        
        // Derive MAC key
        let mut mac_key_input = Vec::new();
        mac_key_input.extend_from_slice(master_key);
        mac_key_input.extend_from_slice(MEDIA_MAC_EXPANSION);
        mac_key_input.extend_from_slice(info);
        let mac_key = sha256(&mac_key_input)[0..32].to_vec();
        
        Ok((enc_key, iv, mac_key))
    }
    
    /// Verify media integrity using HMAC
    pub fn verify_media_integrity(data: &[u8], mac_key: &[u8], expected_mac: &[u8]) -> Result<bool> {
        if mac_key.len() != 32 {
            return Err(Error::Crypto("MAC key must be 32 bytes".to_string()));
        }
        
        // Simple HMAC using SHA256 (simplified implementation)
        let mut hmac_input = Vec::new();
        hmac_input.extend_from_slice(mac_key);
        hmac_input.extend_from_slice(data);
        let computed_mac = sha256(&hmac_input);
        
        Ok(computed_mac[0..expected_mac.len()] == *expected_mac)
    }
}

impl Default for MediaEncryption {
    fn default() -> Self {
        Self::new(MediaEncryptionContext::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_media_encryption_context() {
        let context = MediaEncryptionContext::default();
        assert_eq!(context.encryption_type, MediaEncryptionType::AesCbc);
        assert!(context.enable_compression);
        assert_eq!(context.compression_level, 6);
    }
    
    #[test]
    fn test_media_encryption_creation() {
        let context = MediaEncryptionContext::default();
        let encryption = MediaEncryption::new(context);
        assert_eq!(encryption.context.encryption_type, MediaEncryptionType::AesCbc);
    }
    
    #[test]
    fn test_media_key_generation() {
        let encryption = MediaEncryption::default();
        let key1 = encryption.generate_media_key();
        let key2 = encryption.generate_media_key();
        
        assert_eq!(key1.len(), 32);
        assert_eq!(key2.len(), 32);
        assert_ne!(key1, key2); // Keys should be different
    }
    
    #[test]
    fn test_iv_generation() {
        let encryption = MediaEncryption::default();
        
        let iv_cbc = encryption.generate_iv(&MediaEncryptionType::AesCbc);
        assert_eq!(iv_cbc.len(), 16);
        
        let iv_gcm = encryption.generate_iv(&MediaEncryptionType::AesGcm);
        assert_eq!(iv_gcm.len(), 12);
        
        let iv_chacha = encryption.generate_iv(&MediaEncryptionType::ChaCha20Poly1305);
        assert_eq!(iv_chacha.len(), 12);
    }
    
    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let context = MediaEncryptionContext {
            encryption_type: MediaEncryptionType::AesGcm,
            enable_compression: false,
            compression_level: 6,
        };
        let encryption = MediaEncryption::new(context);
        
        let original_data = b"Hello, World! This is a test message for media encryption.";
        
        // Encrypt
        let encrypted = encryption.encrypt(original_data).unwrap();
        assert!(encrypted.validate());
        assert_eq!(encrypted.original_size, original_data.len() as u64);
        assert!(!encrypted.was_compressed);
        
        // Decrypt
        let decrypted = encryption.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, original_data);
    }
    
    #[test]
    fn test_encrypt_with_compression() {
        let context = MediaEncryptionContext {
            encryption_type: MediaEncryptionType::AesGcm,
            enable_compression: true,
            compression_level: 9,
        };
        let encryption = MediaEncryption::new(context);
        
        // Use repetitive data that compresses well
        let original_data = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".repeat(100);
        
        let encrypted = encryption.encrypt(&original_data).unwrap();
        assert!(encrypted.validate());
        assert_eq!(encrypted.original_size, original_data.len() as u64);
        
        let decrypted = encryption.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, original_data);
    }
    
    #[test]
    fn test_empty_data_error() {
        let encryption = MediaEncryption::default();
        let result = encryption.encrypt(&[]);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_invalid_encrypted_data() {
        let encryption = MediaEncryption::default();
        
        let invalid_encrypted = EncryptedMediaData {
            encrypted_data: vec![],
            media_key: vec![0u8; 32],
            iv: vec![0u8; 16],
            auth_tag: None,
            original_size: 100,
            compressed_size: None,
            original_hash: vec![0u8; 32],
            encrypted_hash: vec![0u8; 32],
            encryption_type: MediaEncryptionType::AesCbc,
            was_compressed: false,
        };
        
        assert!(!invalid_encrypted.validate());
        let result = encryption.decrypt(&invalid_encrypted);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_key_derivation() {
        let master_key = vec![0u8; 32];
        let info = b"test";
        
        let (enc_key, iv, mac_key) = MediaEncryption::derive_media_keys(&master_key, info).unwrap();
        
        assert_eq!(enc_key.len(), 32);
        assert_eq!(iv.len(), 16);
        assert_eq!(mac_key.len(), 32);
        
        // Keys should be different
        assert_ne!(enc_key, iv);
        assert_ne!(enc_key, mac_key);
        assert_ne!(iv[0..16], mac_key[0..16]);
    }
    
    #[test]
    fn test_invalid_master_key_length() {
        let invalid_key = vec![0u8; 16]; // Wrong length
        let info = b"test";
        
        let result = MediaEncryption::derive_media_keys(&invalid_key, info);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_media_integrity_verification() {
        let data = b"Hello, World!";
        let mac_key = vec![1u8; 32];
        
        // Compute expected MAC
        let mut hmac_input = Vec::new();
        hmac_input.extend_from_slice(&mac_key);
        hmac_input.extend_from_slice(data);
        let expected_mac = sha256(&hmac_input);
        
        // Verify integrity
        let is_valid = MediaEncryption::verify_media_integrity(data, &mac_key, &expected_mac).unwrap();
        assert!(is_valid);
        
        // Test with wrong MAC
        let wrong_mac = vec![0u8; 32];
        let is_invalid = MediaEncryption::verify_media_integrity(data, &mac_key, &wrong_mac).unwrap();
        assert!(!is_invalid);
    }
    
    #[test]
    fn test_different_encryption_types() {
        let test_data = b"Test data for different encryption types";
        
        for encryption_type in [MediaEncryptionType::AesCbc, MediaEncryptionType::AesGcm, MediaEncryptionType::ChaCha20Poly1305] {
            let context = MediaEncryptionContext {
                encryption_type: encryption_type.clone(),
                enable_compression: false,
                compression_level: 6,
            };
            let encryption = MediaEncryption::new(context);
            
            let encrypted = encryption.encrypt(test_data).unwrap();
            assert_eq!(encrypted.encryption_type, encryption_type);
            
            let decrypted = encryption.decrypt(&encrypted).unwrap();
            assert_eq!(decrypted, test_data);
        }
    }
}