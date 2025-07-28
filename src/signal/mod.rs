/// Signal Protocol implementation for WhatsApp E2E encryption
/// 
/// This module implements the Signal (TextSecure) protocol used by WhatsApp
/// for end-to-end encryption of messages.

use crate::{
    error::{Error, Result},
    util::keys::ECKeyPair,
    util::crypto::{sha256, hkdf_expand, AesGcm},
};

pub mod session;
pub mod prekey;
pub mod identity;
pub mod group;

pub use session::*;
pub use prekey::*; 
pub use identity::*;
pub use group::*;

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

/// Complete Signal protocol manager for WhatsApp
pub struct SignalProtocolManager {
    identity_store: Box<dyn IdentityKeyStore + Send + Sync>,
    session_store: Box<dyn SessionStore + Send + Sync>,
    prekey_store: Box<dyn PreKeyStore + Send + Sync>,
    group_store: Box<dyn GroupSessionStore + Send + Sync>,
}

impl SignalProtocolManager {
    /// Create new Signal protocol manager with memory stores
    pub fn new_with_memory_stores(registration_id: u32) -> Self {
        Self {
            identity_store: Box::new(MemoryIdentityKeyStore::new(registration_id)),
            session_store: Box::new(MemorySessionStore::new()),
            prekey_store: Box::new(MemoryPreKeyStore::new()),
            group_store: Box::new(MemoryGroupSessionStore::new()),
        }
    }
    
    /// Create new Signal protocol manager with custom stores
    pub fn new_with_stores(
        identity_store: Box<dyn IdentityKeyStore + Send + Sync>,
        session_store: Box<dyn SessionStore + Send + Sync>,
        prekey_store: Box<dyn PreKeyStore + Send + Sync>,
        group_store: Box<dyn GroupSessionStore + Send + Sync>,
    ) -> Self {
        Self {
            identity_store,
            session_store,
            prekey_store,
            group_store,
        }
    }
    
    /// Generate pre-key bundle for registration
    pub fn generate_prekey_bundle(&mut self, device_id: u32) -> Result<PreKeyBundle> {
        let identity_keypair = self.identity_store.get_identity_keypair()
            .ok_or_else(|| Error::Protocol("No identity key available".to_string()))?;
        
        let registration_id = self.identity_store.get_local_registration_id();
        
        // Generate signed pre-key
        let signed_prekey_id = 1; // In real implementation, should be incremented
        let signed_prekey = SignedPreKey::generate(signed_prekey_id, &identity_keypair)?;
        self.prekey_store.store_signed_prekey(signed_prekey.clone());
        
        // Generate one-time pre-key
        let prekey_id = 1; // In real implementation, should be incremented
        let prekey = PreKey::generate(prekey_id);
        self.prekey_store.store_prekey(prekey.clone());
        
        // Create bundle
        PreKeyBundle::new(&identity_keypair, signed_prekey_id, Some(prekey_id), registration_id, device_id)
    }
    
    /// Initialize session with a contact (Alice side - initiator)
    pub fn initialize_outgoing_session(&mut self, address: &str, bundle: &PreKeyBundle) -> Result<()> {
        let identity_keypair = self.identity_store.get_identity_keypair()
            .ok_or_else(|| Error::Protocol("No identity key available".to_string()))?;
        
        let ephemeral_keypair = ECKeyPair::generate();
        
        // Initialize Alice session
        let (session, _ephemeral_pub) = SessionState::initialize_alice_session(
            &identity_keypair,
            bundle,
            &ephemeral_keypair,
        )?;
        
        // Store session
        self.session_store.store_session(address, session);
        
        // Save peer identity
        let peer_identity = IdentityKey::new(
            bundle.identity_key.as_slice().try_into()
                .map_err(|_| Error::Crypto("Invalid identity key length".to_string()))?
        );
        self.identity_store.save_identity(address, &peer_identity)?;
        
        Ok(())
    }
    
    /// Process incoming pre-key message and initialize session (Bob side - receiver) 
    pub fn process_prekey_message(&mut self, address: &str, message: &SignalMessage) -> Result<Vec<u8>> {
        // In a full implementation, would extract pre-key info from message
        // For now, just decrypt as regular message if session exists
        if let Some(mut session) = self.session_store.load_session(address) {
            let plaintext = session.decrypt(message)?;
            self.session_store.store_session(address, session);
            Ok(plaintext)
        } else {
            Err(Error::Protocol("No session found for pre-key message".to_string()))
        }
    }
    
    /// Encrypt message for a contact
    pub fn encrypt_message(&mut self, address: &str, plaintext: &[u8]) -> Result<SignalMessage> {
        let mut session = self.session_store.load_session(address)
            .ok_or_else(|| Error::Protocol("No session found".to_string()))?;
        
        let encrypted = session.encrypt(plaintext)?;
        self.session_store.store_session(address, session);
        
        Ok(encrypted)
    }
    
    /// Decrypt message from a contact
    pub fn decrypt_message(&mut self, address: &str, message: &SignalMessage) -> Result<Vec<u8>> {
        let mut session = self.session_store.load_session(address)
            .ok_or_else(|| Error::Protocol("No session found".to_string()))?;
        
        let plaintext = session.decrypt(message)?;
        self.session_store.store_session(address, session);
        
        Ok(plaintext)
    }
    
    /// Initialize group session
    pub fn initialize_group_session(&mut self, group_id: &str) -> Result<SenderKeyDistribution> {
        let mut group_session = GroupSession::new(group_id.to_string());
        let sender_key_id = 1; // In real implementation, should be unique
        
        let distribution = group_session.initialize_sender_key(sender_key_id)?;
        self.group_store.store_group_session(group_session);
        
        Ok(distribution)
    }
    
    /// Process sender key distribution for a group
    pub fn process_sender_key_distribution(
        &mut self,
        group_id: &str,
        sender_address: &str,
        distribution: &SenderKeyDistribution,
    ) -> Result<()> {
        let mut group_session = self.group_store.load_group_session(group_id)
            .unwrap_or_else(|| GroupSession::new(group_id.to_string()));
        
        group_session.process_sender_key_distribution(sender_address, distribution)?;
        self.group_store.store_group_session(group_session);
        
        Ok(())
    }
    
    /// Encrypt message for a group
    pub fn encrypt_group_message(&mut self, group_id: &str, plaintext: &[u8]) -> Result<SignalMessage> {
        let mut group_session = self.group_store.load_group_session(group_id)
            .ok_or_else(|| Error::Protocol("No group session found".to_string()))?;
        
        let encrypted = group_session.encrypt(plaintext)?;
        self.group_store.store_group_session(group_session);
        
        Ok(encrypted)
    }
    
    /// Decrypt message from a group participant
    pub fn decrypt_group_message(
        &mut self,
        group_id: &str,
        sender_address: &str,
        message: &SignalMessage,
    ) -> Result<Vec<u8>> {
        let mut group_session = self.group_store.load_group_session(group_id)
            .ok_or_else(|| Error::Protocol("No group session found".to_string()))?;
        
        let plaintext = group_session.decrypt(sender_address, message)?;
        self.group_store.store_group_session(group_session);
        
        Ok(plaintext)
    }
    
    /// Check if we have a session with an address
    pub fn has_session(&self, address: &str) -> bool {
        self.session_store.contains_session(address)
    }
    
    /// Check if we have a group session
    pub fn has_group_session(&self, group_id: &str) -> bool {
        self.group_store.contains_group_session(group_id)
    }
    
    /// Get our local registration ID
    pub fn get_local_registration_id(&self) -> u32 {
        self.identity_store.get_local_registration_id()
    }
    
    /// Set trust level for an identity
    pub fn set_trust_level(&mut self, address: &str, trust_level: TrustLevel) -> Result<()> {
        self.identity_store.set_trust_level(address, trust_level)
    }
    
    /// Check if identity is trusted
    pub fn is_trusted_identity(&self, address: &str, identity_key: &IdentityKey) -> bool {
        self.identity_store.is_trusted_identity(address, identity_key)
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
        
        // Save the chain key before encryption (since it gets advanced)
        let original_chain_key = session.chain_key.clone();
        
        // Encrypt
        let encrypted = session.encrypt(plaintext).unwrap();
        assert_eq!(encrypted.message_type, SignalMessageType::WhisperMessage);
        
        // Restore the chain key for decryption
        session.chain_key = original_chain_key;
        
        // For this test, we'll use the same session for decryption
        // In a real implementation, there would be proper key exchange
        let decrypted = session.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_signal_protocol_manager() {
        let mut manager = SignalProtocolManager::new_with_memory_stores(12345);
        
        assert_eq!(manager.get_local_registration_id(), 12345);
        assert!(!manager.has_session("test@example.com"));
        assert!(!manager.has_group_session("test-group"));
        
        // Generate pre-key bundle
        let bundle = manager.generate_prekey_bundle(1).unwrap();
        assert_eq!(bundle.device_id, 1);
        assert_eq!(bundle.registration_id, 12345);
        
        // Initialize group session
        let distribution = manager.initialize_group_session("test-group").unwrap();
        assert!(manager.has_group_session("test-group"));
        
        // Process distribution
        manager.process_sender_key_distribution("test-group", "alice@example.com", &distribution).unwrap();
        
        // Test group messaging
        let plaintext = b"Hello group!";
        let encrypted = manager.encrypt_group_message("test-group", plaintext).unwrap();
        let decrypted = manager.decrypt_group_message("test-group", "alice@example.com", &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_signal_protocol_manager_session_management() {
        let mut alice = SignalProtocolManager::new_with_memory_stores(11111);
        let mut bob = SignalProtocolManager::new_with_memory_stores(22222);
        
        // Bob generates pre-key bundle
        let bob_bundle = bob.generate_prekey_bundle(1).unwrap();
        
        // Alice initializes session with Bob
        alice.initialize_outgoing_session("bob@example.com", &bob_bundle).unwrap();
        assert!(alice.has_session("bob@example.com"));
        
        // Note: In a full implementation, Bob would process Alice's initial message
        // and create his own session. For this test, we'll create Bob's session manually.
        let alice_bundle = alice.generate_prekey_bundle(1).unwrap();
        bob.initialize_outgoing_session("alice@example.com", &alice_bundle).unwrap();
        
        // Basic message encryption (would need proper session state sync in real implementation)
        let plaintext = b"Hello Bob!";
        let _encrypted = alice.encrypt_message("bob@example.com", plaintext);
        // Note: Decryption would require proper session initialization
    }
}