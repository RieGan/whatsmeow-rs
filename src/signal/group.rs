/// Signal protocol group (sender key) cryptography for WhatsApp groups

use crate::{
    error::{Error, Result},
    signal::{SignalMessage, SignalMessageType},
    util::{
        keys::ECKeyPair,
        crypto::{sha256, AesGcm},
    },
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Sender key for group messaging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderKey {
    /// Sender key ID
    pub id: u32,
    /// Current iteration number
    pub iteration: u32,
    /// Chain key for this sender
    pub chain_key: [u8; 32],
    /// Public signing key for authentication
    pub signing_key: [u8; 32],
}

impl SenderKey {
    /// Create a new sender key
    pub fn new(id: u32, chain_key: [u8; 32], signing_key: [u8; 32]) -> Self {
        Self {
            id,
            iteration: 0,
            chain_key,
            signing_key,
        }
    }
    
    /// Derive message key from current chain key
    pub fn derive_message_key(&self) -> Result<[u8; 32]> {
        let mut input = self.chain_key.to_vec();
        input.push(0x01); // Message key constant
        let hash = sha256(&input);
        Ok(hash[0..32].try_into().unwrap())
    }
    
    /// Advance the chain key
    pub fn advance_chain_key(&mut self) -> Result<()> {
        let mut input = self.chain_key.to_vec();
        input.push(0x02); // Chain key constant
        let new_key = sha256(&input);
        self.chain_key.copy_from_slice(&new_key[0..32]);
        self.iteration += 1;
        Ok(())
    }
}

/// Sender key state for a group participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderKeyState {
    /// Sender key ID
    pub sender_key_id: u32,
    /// Current sender key
    pub sender_key: SenderKey,
    /// Message number counter
    pub message_number: u32,
}

impl SenderKeyState {
    /// Create new sender key state
    pub fn new(sender_key_id: u32, chain_key: [u8; 32], signing_key: [u8; 32]) -> Self {
        Self {
            sender_key_id,
            sender_key: SenderKey::new(sender_key_id, chain_key, signing_key),
            message_number: 0,
        }
    }
    
    /// Encrypt a message for the group
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<SignalMessage> {
        // Derive message key
        let message_key = self.sender_key.derive_message_key()?;
        
        // Encrypt the message
        let ciphertext = self.encrypt_with_message_key(&message_key, plaintext)?;
        
        // Create message data
        let mut message_data = Vec::new();
        message_data.extend_from_slice(&self.sender_key_id.to_be_bytes());
        message_data.extend_from_slice(&self.sender_key.iteration.to_be_bytes());
        message_data.extend_from_slice(&ciphertext);
        
        // Advance state
        self.sender_key.advance_chain_key()?;
        self.message_number += 1;
        
        Ok(SignalMessage {
            message_type: SignalMessageType::SenderKeyMessage,
            serialized: message_data,
        })
    }
    
    /// Decrypt a message from the group
    pub fn decrypt(&mut self, message: &SignalMessage) -> Result<Vec<u8>> {
        if message.message_type != SignalMessageType::SenderKeyMessage {
            return Err(Error::Protocol("Not a sender key message".to_string()));
        }
        
        if message.serialized.len() < 12 {
            return Err(Error::Protocol("Invalid sender key message format".to_string()));
        }
        
        // Parse message header
        let sender_key_id = u32::from_be_bytes([
            message.serialized[0], message.serialized[1],
            message.serialized[2], message.serialized[3]
        ]);
        let iteration = u32::from_be_bytes([
            message.serialized[4], message.serialized[5],
            message.serialized[6], message.serialized[7]
        ]);
        let ciphertext = &message.serialized[8..];
        
        // Verify sender key ID matches
        if sender_key_id != self.sender_key_id {
            return Err(Error::Protocol("Sender key ID mismatch".to_string()));
        }
        
        // Handle out-of-order messages
        if iteration < self.sender_key.iteration {
            return Err(Error::Protocol("Message iteration too old".to_string()));
        }
        
        // Fast-forward chain key if needed
        while self.sender_key.iteration < iteration {
            self.sender_key.advance_chain_key()?;
        }
        
        // Derive message key and decrypt
        let message_key = self.sender_key.derive_message_key()?;
        let plaintext = self.decrypt_with_message_key(&message_key, ciphertext)?;
        
        // Advance for next message
        self.sender_key.advance_chain_key()?;
        
        Ok(plaintext)
    }
    
    /// Encrypt with message key
    fn encrypt_with_message_key(&self, message_key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
        let aes = AesGcm::new(message_key)?;
        
        // Derive IV from message key
        let mut iv_input = message_key.to_vec();
        iv_input.extend_from_slice(b"IV");
        let iv_hash = sha256(&iv_input);
        let iv: [u8; 12] = iv_hash[0..12].try_into().unwrap();
        
        aes.encrypt(&iv, plaintext)
    }
    
    /// Decrypt with message key
    fn decrypt_with_message_key(&self, message_key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>> {
        let aes = AesGcm::new(message_key)?;
        
        // Derive IV from message key
        let mut iv_input = message_key.to_vec();
        iv_input.extend_from_slice(b"IV");
        let iv_hash = sha256(&iv_input);
        let iv: [u8; 12] = iv_hash[0..12].try_into().unwrap();
        
        aes.decrypt(&iv, ciphertext)
    }
}

/// Sender key distribution message for establishing group keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderKeyDistribution {
    /// Sender key ID
    pub id: u32,
    /// Iteration number
    pub iteration: u32,
    /// Chain key to distribute
    pub chain_key: [u8; 32],
    /// Signing key for authentication
    pub signing_key: [u8; 32],
}

impl SenderKeyDistribution {
    /// Create new sender key distribution message
    pub fn new(id: u32, iteration: u32, chain_key: [u8; 32], signing_key: [u8; 32]) -> Self {
        Self {
            id,
            iteration,
            chain_key,
            signing_key,
        }
    }
    
    /// Serialize the distribution message
    pub fn serialize(&self) -> Result<SignalMessage> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.id.to_be_bytes());
        data.extend_from_slice(&self.iteration.to_be_bytes());
        data.extend_from_slice(&self.chain_key);
        data.extend_from_slice(&self.signing_key);
        
        Ok(SignalMessage {
            message_type: SignalMessageType::SenderKeyDistributionMessage,
            serialized: data,
        })
    }
    
    /// Deserialize from signal message
    pub fn deserialize(message: &SignalMessage) -> Result<Self> {
        if message.message_type != SignalMessageType::SenderKeyDistributionMessage {
            return Err(Error::Protocol("Not a sender key distribution message".to_string()));
        }
        
        if message.serialized.len() != 72 { // 4 + 4 + 32 + 32
            return Err(Error::Protocol("Invalid distribution message length".to_string()));
        }
        
        let id = u32::from_be_bytes([
            message.serialized[0], message.serialized[1],
            message.serialized[2], message.serialized[3]
        ]);
        
        let iteration = u32::from_be_bytes([
            message.serialized[4], message.serialized[5],
            message.serialized[6], message.serialized[7]
        ]);
        
        let mut chain_key = [0u8; 32];
        chain_key.copy_from_slice(&message.serialized[8..40]);
        
        let mut signing_key = [0u8; 32];
        signing_key.copy_from_slice(&message.serialized[40..72]);
        
        Ok(Self {
            id,
            iteration,
            chain_key,
            signing_key,
        })
    }
}

/// Group session for managing sender keys
#[derive(Debug, Clone)]
pub struct GroupSession {
    /// Group ID
    pub group_id: String,
    /// Our sender key state
    pub our_sender_key: Option<SenderKeyState>,
    /// Other participants' sender keys
    pub participant_keys: HashMap<String, SenderKeyState>,
}

impl GroupSession {
    /// Create new group session
    pub fn new(group_id: String) -> Self {
        Self {
            group_id,
            our_sender_key: None,
            participant_keys: HashMap::new(),
        }
    }
    
    /// Initialize our sender key for this group
    pub fn initialize_sender_key(&mut self, sender_key_id: u32) -> Result<SenderKeyDistribution> {
        // Generate chain key and signing key
        let chain_key = {
            let mut rng = rand::thread_rng();
            let mut key = [0u8; 32];
            rand::RngCore::fill_bytes(&mut rng, &mut key);
            key
        };
        
        let signing_keypair = ECKeyPair::generate();
        let signing_key = signing_keypair.public_bytes();
        
        // Create our sender key state
        self.our_sender_key = Some(SenderKeyState::new(sender_key_id, chain_key, signing_key));
        
        // Create distribution message
        Ok(SenderKeyDistribution::new(sender_key_id, 0, chain_key, signing_key))
    }
    
    /// Process sender key distribution from a participant
    pub fn process_sender_key_distribution(
        &mut self,
        sender_address: &str,
        distribution: &SenderKeyDistribution,
    ) -> Result<()> {
        // Create sender key state for this participant
        let sender_key_state = SenderKeyState::new(
            distribution.id,
            distribution.chain_key,
            distribution.signing_key,
        );
        
        self.participant_keys.insert(sender_address.to_string(), sender_key_state);
        
        Ok(())
    }
    
    /// Encrypt message for the group
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<SignalMessage> {
        match &mut self.our_sender_key {
            Some(sender_key) => sender_key.encrypt(plaintext),
            None => Err(Error::Protocol("No sender key initialized".to_string())),
        }
    }
    
    /// Decrypt message from a group participant
    pub fn decrypt(&mut self, sender_address: &str, message: &SignalMessage) -> Result<Vec<u8>> {
        match self.participant_keys.get_mut(sender_address) {
            Some(sender_key) => sender_key.decrypt(message),
            None => Err(Error::Protocol(format!("No sender key for {}", sender_address))),
        }
    }
    
    /// Check if we have a sender key for a participant
    pub fn has_sender_key(&self, sender_address: &str) -> bool {
        self.participant_keys.contains_key(sender_address)
    }
    
    /// Get list of participants we have sender keys for
    pub fn get_participants(&self) -> Vec<String> {
        self.participant_keys.keys().cloned().collect()
    }
}

/// Group session store trait
pub trait GroupSessionStore {
    /// Load group session
    fn load_group_session(&self, group_id: &str) -> Option<GroupSession>;
    
    /// Store group session
    fn store_group_session(&mut self, group_session: GroupSession);
    
    /// Check if group session exists
    fn contains_group_session(&self, group_id: &str) -> bool;
    
    /// Delete group session
    fn delete_group_session(&mut self, group_id: &str);
}

/// In-memory group session store
#[derive(Debug, Default)]
pub struct MemoryGroupSessionStore {
    sessions: HashMap<String, GroupSession>,
}

impl MemoryGroupSessionStore {
    /// Create new memory group session store
    pub fn new() -> Self {
        Self::default()
    }
}

impl GroupSessionStore for MemoryGroupSessionStore {
    fn load_group_session(&self, group_id: &str) -> Option<GroupSession> {
        self.sessions.get(group_id).cloned()
    }
    
    fn store_group_session(&mut self, group_session: GroupSession) {
        self.sessions.insert(group_session.group_id.clone(), group_session);
    }
    
    fn contains_group_session(&self, group_id: &str) -> bool {
        self.sessions.contains_key(group_id)
    }
    
    fn delete_group_session(&mut self, group_id: &str) {
        self.sessions.remove(group_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sender_key_creation() {
        let chain_key = [1u8; 32];
        let signing_key = [2u8; 32];
        let sender_key = SenderKey::new(1, chain_key, signing_key);
        
        assert_eq!(sender_key.id, 1);
        assert_eq!(sender_key.iteration, 0);
        assert_eq!(sender_key.chain_key, chain_key);
        assert_eq!(sender_key.signing_key, signing_key);
    }
    
    #[test]
    fn test_sender_key_chain_advancement() {
        let mut sender_key = SenderKey::new(1, [1u8; 32], [2u8; 32]);
        let original_key = sender_key.chain_key;
        
        sender_key.advance_chain_key().unwrap();
        
        assert_eq!(sender_key.iteration, 1);
        assert_ne!(sender_key.chain_key, original_key);
    }
    
    #[test]
    fn test_sender_key_state_encrypt_decrypt() {
        let mut sender_state = SenderKeyState::new(1, [1u8; 32], [2u8; 32]);
        let plaintext = b"Hello group!";
        
        // Encrypt
        let encrypted = sender_state.encrypt(plaintext).unwrap();
        assert_eq!(encrypted.message_type, SignalMessageType::SenderKeyMessage);
        
        // Reset state for decryption (simulate peer)
        let mut peer_state = SenderKeyState::new(1, [1u8; 32], [2u8; 32]);
        
        // Decrypt
        let decrypted = peer_state.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_sender_key_distribution() {
        let distribution = SenderKeyDistribution::new(1, 0, [1u8; 32], [2u8; 32]);
        
        // Serialize
        let message = distribution.serialize().unwrap();
        assert_eq!(message.message_type, SignalMessageType::SenderKeyDistributionMessage);
        
        // Deserialize
        let deserialized = SenderKeyDistribution::deserialize(&message).unwrap();
        assert_eq!(deserialized.id, distribution.id);
        assert_eq!(deserialized.iteration, distribution.iteration);
        assert_eq!(deserialized.chain_key, distribution.chain_key);
        assert_eq!(deserialized.signing_key, distribution.signing_key);
    }
    
    #[test]
    fn test_group_session() {
        let mut group_session = GroupSession::new("test-group".to_string());
        
        // Initialize sender key
        let distribution = group_session.initialize_sender_key(1).unwrap();
        assert_eq!(distribution.id, 1);
        assert!(group_session.our_sender_key.is_some());
        
        // Process distribution from another participant
        let participant = "alice@example.com";
        group_session.process_sender_key_distribution(participant, &distribution).unwrap();
        assert!(group_session.has_sender_key(participant));
        
        // Test encryption/decryption
        let plaintext = b"Group message";
        let encrypted = group_session.encrypt(plaintext).unwrap();
        let decrypted = group_session.decrypt(participant, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_memory_group_session_store() {
        let mut store = MemoryGroupSessionStore::new();
        let group_id = "test-group";
        let session = GroupSession::new(group_id.to_string());
        
        // Store session
        store.store_group_session(session.clone());
        assert!(store.contains_group_session(group_id));
        
        // Load session
        let loaded = store.load_group_session(group_id).unwrap();
        assert_eq!(loaded.group_id, session.group_id);
        
        // Delete session
        store.delete_group_session(group_id);
        assert!(!store.contains_group_session(group_id));
    }
}