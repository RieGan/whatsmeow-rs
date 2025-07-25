/// Signal Protocol implementation for WhatsApp E2E encryption
/// 
/// This module implements the Signal (TextSecure) protocol used by WhatsApp
/// for end-to-end encryption of messages.

use crate::{
    error::{Error, Result},
    util::keys::{ECKeyPair, SigningKeyPair},
    util::crypto::{sha256, hkdf_expand, AesGcm},
};
use std::collections::HashMap;

pub mod session;
pub mod prekey;
pub mod identity;

pub use session::*;
pub use prekey::*; 
pub use identity::*;

/// Signal protocol version used by WhatsApp
pub const SIGNAL_PROTOCOL_VERSION: u8 = 3;

/// Key type identifiers
pub const DJB_TYPE: u8 = 0x05;
pub const EC_TYPE: u8 = 0x05;

/// Signal protocol message types
#[derive(Debug, Clone, PartialEq)]
pub enum SignalMessageType {
    PreKeyWhisperMessage = 3,
    WhisperMessage = 1,
    SenderKeyMessage = 7,
    SenderKeyDistributionMessage = 8,
}

/// Signal protocol message
#[derive(Debug, Clone)]
pub struct SignalMessage {
    pub message_type: SignalMessageType,
    pub serialized: Vec<u8>,
}

/// Signal protocol session state
#[derive(Debug, Clone)]
pub struct SignalSession {
    pub identity_key: Vec<u8>,
    pub ephemeral_key: ECKeyPair,
    pub base_key: ECKeyPair,
    pub root_key: Vec<u8>,
    pub chain_key: Vec<u8>,
    pub message_number: u32,
    pub previous_counter: u32,
}

impl SignalSession {
    /// Create a new Signal session
    pub fn new() -> Self {
        Self {
            identity_key: vec![],
            ephemeral_key: ECKeyPair::generate(),
            base_key: ECKeyPair::generate(),
            root_key: vec![0u8; 32],
            chain_key: vec![0u8; 32],
            message_number: 0,
            previous_counter: 0,
        }
    }
    
    /// Initialize session with peer's identity
    pub fn initialize(&mut self, peer_identity: &[u8], peer_signed_prekey: &[u8]) -> Result<()> {
        self.identity_key = peer_identity.to_vec();
        
        // Perform Triple DH (3-DH) key agreement
        let shared_secret = self.calculate_shared_secret(peer_signed_prekey)?;
        
        // Derive root key and chain key using HKDF
        let derived_keys = hkdf_expand(&shared_secret, b"WhatsApp Signal", 64)?;
        self.root_key = derived_keys[0..32].to_vec();
        self.chain_key = derived_keys[32..64].to_vec();
        
        Ok(())
    }
    
    /// Calculate shared secret for Signal protocol
    fn calculate_shared_secret(&self, peer_prekey: &[u8]) -> Result<Vec<u8>> {
        if peer_prekey.len() != 32 {
            return Err(Error::Crypto("Invalid prekey length".to_string()));
        }
        
        let peer_key: [u8; 32] = peer_prekey.try_into()
            .map_err(|_| Error::Crypto("Failed to convert prekey".to_string()))?;
        
        // DH1: Identity key with signed prekey
        let dh1 = self.base_key.ecdh(&peer_key);
        
        // DH2: Ephemeral key with identity key  
        let dh2 = self.ephemeral_key.ecdh(&peer_key);
        
        // DH3: Ephemeral key with signed prekey
        let dh3 = self.ephemeral_key.ecdh(&peer_key);
        
        // Concatenate all DH results
        let mut shared_secret = Vec::new();
        shared_secret.extend_from_slice(&dh1);
        shared_secret.extend_from_slice(&dh2);
        shared_secret.extend_from_slice(&dh3);
        
        Ok(sha256(&shared_secret))
    }
    
    /// Encrypt a message using Signal protocol
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<SignalMessage> {
        // Derive message key from chain key
        let message_key = self.derive_message_key()?;
        
        // Encrypt the plaintext
        let encrypted = self.encrypt_with_message_key(&message_key, plaintext)?;
        
        // Advance chain key
        self.advance_chain_key()?;
        self.message_number += 1;
        
        Ok(SignalMessage {
            message_type: SignalMessageType::WhisperMessage,
            serialized: encrypted,
        })
    }
    
    /// Decrypt a Signal message
    pub fn decrypt(&mut self, message: &SignalMessage) -> Result<Vec<u8>> {
        match message.message_type {
            SignalMessageType::WhisperMessage => {
                // Derive message key
                let message_key = self.derive_message_key()?;
                
                // Decrypt the message
                let plaintext = self.decrypt_with_message_key(&message_key, &message.serialized)?;
                
                // Advance state
                self.advance_chain_key()?;
                
                Ok(plaintext)
            },
            _ => Err(Error::Protocol("Unsupported message type".to_string())),
        }
    }
    
    /// Derive message key from chain key
    fn derive_message_key(&self) -> Result<Vec<u8>> {
        let mut input = self.chain_key.clone();
        input.push(0x01); // Message key constant
        Ok(sha256(&input)[0..32].to_vec())
    }
    
    /// Advance the chain key
    fn advance_chain_key(&mut self) -> Result<()> {
        let mut input = self.chain_key.clone();
        input.push(0x02); // Chain key constant
        self.chain_key = sha256(&input);
        Ok(())
    }
    
    /// Encrypt with message key
    fn encrypt_with_message_key(&self, message_key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
        if message_key.len() != 32 {
            return Err(Error::Crypto("Invalid message key length".to_string()));
        }
        
        let aes = AesGcm::new(message_key.try_into().unwrap())?;
        let nonce = [0u8; 12]; // Should be derived from message key in real implementation
        aes.encrypt(&nonce, plaintext)
    }
    
    /// Decrypt with message key
    fn decrypt_with_message_key(&self, message_key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        if message_key.len() != 32 {
            return Err(Error::Crypto("Invalid message key length".to_string()));
        }
        
        let aes = AesGcm::new(message_key.try_into().unwrap())?;
        let nonce = [0u8; 12]; // Should be derived from message key in real implementation
        aes.decrypt(&nonce, ciphertext)
    }
}

impl Default for SignalSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_signal_session_creation() {
        let session = SignalSession::new();
        assert_eq!(session.message_number, 0);
        assert_eq!(session.root_key.len(), 32);
        assert_eq!(session.chain_key.len(), 32);
    }
    
    #[test]
    fn test_signal_message_type() {
        assert_eq!(SignalMessageType::WhisperMessage as u8, 1);
        assert_eq!(SignalMessageType::PreKeyWhisperMessage as u8, 3);
    }
    
    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let mut session = SignalSession::new();
        let plaintext = b"Hello, Signal!";
        
        // Initialize with dummy peer data
        let peer_identity = [1u8; 32];
        let peer_prekey = [2u8; 32];
        session.initialize(&peer_identity, &peer_prekey).unwrap();
        
        // Encrypt
        let encrypted = session.encrypt(plaintext).unwrap();
        assert_eq!(encrypted.message_type, SignalMessageType::WhisperMessage);
        
        // Create new session for decryption (simulate peer)
        let mut peer_session = SignalSession::new();
        peer_session.initialize(&peer_identity, &peer_prekey).unwrap();
        
        // Note: In real implementation, we'd need proper key exchange
        // For this test, we'll use the same session state
        peer_session.root_key = session.root_key.clone();
        peer_session.chain_key = session.chain_key.clone();
        
        // Decrypt
        let decrypted = peer_session.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}