/// Signal protocol session management

use crate::{
    error::{Error, Result},
    signal::{
        prekey::{PreKeyBundle, PreKey, SignedPreKey},
        SignalMessage, SignalMessageType, SIGNAL_PROTOCOL_VERSION,
    },
    util::{
        keys::{ECKeyPair, SigningKeyPair},
        crypto::{sha256, hkdf_expand, AesGcm},
    },
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Helper function to advance chain key
fn advance_chain_key_impl(chain_key: &mut [u8; 32]) -> Result<()> {
    let mut input = chain_key.to_vec();
    input.push(0x02); // Chain key constant
    let new_key = sha256(&input);
    chain_key.copy_from_slice(&new_key[0..32]);
    Ok(())
}

/// Double Ratchet session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Session version
    pub version: u8,
    /// Local identity key
    pub local_identity_key: [u8; 32],
    /// Remote identity key
    pub remote_identity_key: [u8; 32],
    /// Root key for Double Ratchet
    pub root_key: [u8; 32],
    /// Current sending chain key
    pub sending_chain_key: Option<ChainState>,
    /// Current receiving chain key
    pub receiving_chain_key: Option<ChainState>,
    /// Message number counter
    pub send_message_number: u32,
    /// Previous counter for out-of-order messages
    pub previous_counter: u32,
    /// Receiving chains for handling out-of-order messages
    pub receiving_chains: HashMap<Vec<u8>, ChainState>,
    /// Pending pre-key if this is a new session
    pub pending_prekey: Option<PendingPreKey>,
}

/// Chain state for Double Ratchet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainState {
    /// Chain key for this chain
    pub chain_key: [u8; 32],
    /// Message number in this chain
    pub message_number: u32,
    /// Public key for this chain (for receiving chains)
    pub ephemeral_public: Option<[u8; 32]>,
}

/// Pending pre-key information for new sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPreKey {
    pub signed_prekey_id: u32,
    pub prekey_id: Option<u32>,
    pub base_key: [u8; 32],
}

impl SessionState {
    /// Create a new session state
    pub fn new(
        local_identity: [u8; 32],
        remote_identity: [u8; 32],
        root_key: [u8; 32],
    ) -> Self {
        Self {
            version: SIGNAL_PROTOCOL_VERSION,
            local_identity_key: local_identity,
            remote_identity_key: remote_identity,
            root_key,
            sending_chain_key: None,
            receiving_chain_key: None,
            send_message_number: 0,
            previous_counter: 0,
            receiving_chains: HashMap::new(),
            pending_prekey: None,
        }
    }
    
    /// Initialize session from a pre-key bundle (Alice side)
    pub fn initialize_alice_session(
        local_identity: &SigningKeyPair,
        bundle: &PreKeyBundle,
        ephemeral_keypair: &ECKeyPair,
    ) -> Result<(Self, [u8; 32])> {
        // Validate bundle
        bundle.validate()?;
        
        let remote_identity: [u8; 32] = bundle.identity_key.as_slice().try_into()
            .map_err(|_| Error::Crypto("Invalid identity key length".to_string()))?;
        
        // Calculate shared secret using Triple DH
        let shared_secret = Self::calculate_alice_shared_secret(
            local_identity,
            bundle,
            ephemeral_keypair,
        )?;
        
        // Derive root key and chain key
        let master_key = hkdf_expand(&shared_secret, b"WhatsApp Signal", 64)?;
        let root_key: [u8; 32] = master_key[0..32].try_into().unwrap();
        let chain_key: [u8; 32] = master_key[32..64].try_into().unwrap();
        
        let mut session = Self::new(
            local_identity.public_bytes(),
            remote_identity,
            root_key,
        );
        
        // Set up sending chain
        session.sending_chain_key = Some(ChainState {
            chain_key,
            message_number: 0,
            ephemeral_public: Some(ephemeral_keypair.public_bytes()),
        });
        
        // Set pending pre-key info
        session.pending_prekey = Some(PendingPreKey {
            signed_prekey_id: bundle.signed_prekey.id,
            prekey_id: bundle.prekey.as_ref().map(|pk| pk.id),
            base_key: ephemeral_keypair.public_bytes(),
        });
        
        Ok((session, ephemeral_keypair.public_bytes()))
    }
    
    /// Initialize session from received pre-key message (Bob side)
    pub fn initialize_bob_session(
        local_identity: &SigningKeyPair,
        signed_prekey: &SignedPreKey,
        prekey: Option<&PreKey>,
        sender_ephemeral: &[u8; 32],
        sender_identity: &[u8; 32],
    ) -> Result<Self> {
        // Calculate shared secret using Triple DH
        let shared_secret = Self::calculate_bob_shared_secret(
            local_identity,
            signed_prekey,
            prekey,
            sender_ephemeral,
            sender_identity,
        )?;
        
        // Derive root key and chain key
        let master_key = hkdf_expand(&shared_secret, b"WhatsApp Signal", 64)?;
        let root_key: [u8; 32] = master_key[0..32].try_into().unwrap();
        let chain_key: [u8; 32] = master_key[32..64].try_into().unwrap();
        
        let mut session = Self::new(
            local_identity.public_bytes(),
            *sender_identity,
            root_key,
        );
        
        // Set up receiving chain for sender's ephemeral key
        session.receiving_chains.insert(
            sender_ephemeral.to_vec(),
            ChainState {
                chain_key,
                message_number: 0,
                ephemeral_public: Some(*sender_ephemeral),
            }
        );
        
        Ok(session)
    }
    
    /// Calculate shared secret for Alice (initiator)
    fn calculate_alice_shared_secret(
        identity_keypair: &SigningKeyPair,
        bundle: &PreKeyBundle,
        ephemeral_keypair: &ECKeyPair,
    ) -> Result<Vec<u8>> {
        let signed_prekey_pub: [u8; 32] = bundle.signed_prekey.public_key();
        
        // DH1: Our identity key * Their signed prekey
        let identity_ec = ECKeyPair::from_private_bytes(&identity_keypair.private_bytes())?;
        let dh1 = identity_ec.ecdh(&signed_prekey_pub);
        
        // DH2: Our ephemeral key * Their identity key  
        let their_identity: [u8; 32] = bundle.identity_key.as_slice().try_into()
            .map_err(|_| Error::Crypto("Invalid identity key".to_string()))?;
        let dh2 = ephemeral_keypair.ecdh(&their_identity);
        
        // DH3: Our ephemeral key * Their signed prekey
        let dh3 = ephemeral_keypair.ecdh(&signed_prekey_pub);
        
        let mut shared_secret = Vec::new();
        shared_secret.extend_from_slice(&dh1);
        shared_secret.extend_from_slice(&dh2);
        shared_secret.extend_from_slice(&dh3);
        
        // DH4: Our ephemeral key * Their one-time prekey (if present)
        if let Some(prekey) = &bundle.prekey {
            let prekey_pub = prekey.public_key();
            let dh4 = ephemeral_keypair.ecdh(&prekey_pub);
            shared_secret.extend_from_slice(&dh4);
        }
        
        Ok(sha256(&shared_secret))
    }
    
    /// Calculate shared secret for Bob (receiver)
    fn calculate_bob_shared_secret(
        identity_keypair: &SigningKeyPair,
        signed_prekey: &SignedPreKey,
        prekey: Option<&PreKey>,
        sender_ephemeral: &[u8; 32],
        sender_identity: &[u8; 32],
    ) -> Result<Vec<u8>> {
        // DH1: Their ephemeral key * Our signed prekey
        let dh1 = signed_prekey.keypair.ecdh(sender_ephemeral);
        
        // DH2: Their ephemeral key * Our identity key
        let identity_ec = ECKeyPair::from_private_bytes(&identity_keypair.private_bytes())?;
        let dh2 = identity_ec.ecdh(sender_identity);
        
        // DH3: Their ephemeral key * Our signed prekey (same as DH1)
        let dh3 = signed_prekey.keypair.ecdh(sender_ephemeral);
        
        let mut shared_secret = Vec::new();
        shared_secret.extend_from_slice(&dh1);
        shared_secret.extend_from_slice(&dh2);
        shared_secret.extend_from_slice(&dh3);
        
        // DH4: Their ephemeral key * Our one-time prekey (if used)
        if let Some(prekey) = prekey {
            let dh4 = prekey.keypair.ecdh(sender_ephemeral);
            shared_secret.extend_from_slice(&dh4);
        }
        
        Ok(sha256(&shared_secret))
    }
    
    /// Encrypt a message using the current session
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<SignalMessage> {
        // Get or create sending chain
        if self.sending_chain_key.is_none() {
            self.perform_dh_ratchet()?;
        }
        
        let (_message_key, message_number, ciphertext) = {
            let chain = self.sending_chain_key.as_ref().unwrap();
            let message_key = self.derive_message_key(&chain.chain_key)?;
            let ciphertext = self.encrypt_with_message_key(&message_key, plaintext)?;
            (message_key, chain.message_number, ciphertext)
        };
        
        // Create signal message
        let mut message_data = Vec::new();
        message_data.extend_from_slice(&self.version.to_be_bytes());
        message_data.extend_from_slice(&message_number.to_be_bytes());
        message_data.extend_from_slice(&self.previous_counter.to_be_bytes());
        message_data.extend_from_slice(&ciphertext);
        
        // Advance chain state
        if let Some(chain) = &mut self.sending_chain_key {
            advance_chain_key_impl(&mut chain.chain_key)?;
            chain.message_number += 1;
        }
        self.send_message_number += 1;
        
        let message_type = if self.pending_prekey.is_some() {
            SignalMessageType::PreKeyWhisperMessage
        } else {
            SignalMessageType::WhisperMessage
        };
        
        Ok(SignalMessage {
            message_type,
            serialized: message_data,
        })
    }
    
    /// Decrypt a received Signal message
    pub fn decrypt(&mut self, message: &SignalMessage) -> Result<Vec<u8>> {
        match message.message_type {
            SignalMessageType::PreKeyWhisperMessage => {
                self.decrypt_prekey_message(message)
            },
            SignalMessageType::WhisperMessage => {
                self.decrypt_whisper_message(message)
            },
            _ => Err(Error::Protocol("Unsupported message type".to_string())),
        }
    }
    
    /// Decrypt a pre-key message
    fn decrypt_prekey_message(&mut self, message: &SignalMessage) -> Result<Vec<u8>> {
        // For now, treat as whisper message
        // In full implementation, would extract pre-key info and initialize session
        self.decrypt_whisper_message(message)
    }
    
    /// Decrypt a whisper message
    fn decrypt_whisper_message(&mut self, message: &SignalMessage) -> Result<Vec<u8>> {
        if message.serialized.len() < 12 {
            return Err(Error::Protocol("Invalid message format".to_string()));
        }
        
        // Parse message header
        let version = message.serialized[0];
        let _message_number = u32::from_be_bytes([
            message.serialized[1], message.serialized[2], 
            message.serialized[3], message.serialized[4]
        ]);
        let _previous_counter = u32::from_be_bytes([
            message.serialized[5], message.serialized[6],
            message.serialized[7], message.serialized[8]
        ]);
        let ciphertext = &message.serialized[12..];
        
        if version != self.version {
            return Err(Error::Protocol("Version mismatch".to_string()));
        }
        
        // Derive message key and decrypt
        let (_message_key, plaintext) = {
            let chain_key = if let Some(chain) = &self.receiving_chain_key {
                &chain.chain_key
            } else if let Some(chain) = &self.sending_chain_key {
                // For testing, allow same session to decrypt its own messages
                &chain.chain_key
            } else {
                return Err(Error::Protocol("No receiving chain available".to_string()));
            };
            
            let message_key = self.derive_message_key(chain_key)?;
            let plaintext = self.decrypt_with_message_key(&message_key, ciphertext)?;
            (message_key, plaintext)
        };
        
        // Advance chain
        if let Some(chain) = &mut self.receiving_chain_key {
            advance_chain_key_impl(&mut chain.chain_key)?;
        }
        
        Ok(plaintext)
    }
    
    /// Perform DH ratchet step to create new sending chain
    fn perform_dh_ratchet(&mut self) -> Result<()> {
        // Generate new ephemeral key pair
        let new_ephemeral = ECKeyPair::generate();
        
        // Would need to exchange this with peer and derive new root/chain keys
        // For now, create a placeholder chain
        self.sending_chain_key = Some(ChainState {
            chain_key: self.root_key.clone(),
            message_number: 0,
            ephemeral_public: Some(new_ephemeral.public_bytes()),
        });
        
        Ok(())
    }
    
    /// Derive message key from chain key
    fn derive_message_key(&self, chain_key: &[u8; 32]) -> Result<[u8; 32]> {
        let mut input = chain_key.to_vec();
        input.push(0x01); // Message key constant
        let hash = sha256(&input);
        Ok(hash[0..32].try_into().unwrap())
    }
    
    /// Advance chain key using HMAC
    fn advance_chain_key(&self, chain_key: &mut [u8; 32]) -> Result<()> {
        let mut input = chain_key.to_vec();
        input.push(0x02); // Chain key constant
        let new_key = sha256(&input);
        chain_key.copy_from_slice(&new_key[0..32]);
        Ok(())
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
    
    /// Check if session has pending pre-key
    pub fn has_pending_prekey(&self) -> bool {
        self.pending_prekey.is_some()
    }
    
    /// Clear pending pre-key after successful message exchange
    pub fn clear_pending_prekey(&mut self) {
        self.pending_prekey = None;
    }
}

/// Session store trait for managing Signal sessions
pub trait SessionStore {
    /// Load session for address
    fn load_session(&self, address: &str) -> Option<SessionState>;
    
    /// Store session for address
    fn store_session(&mut self, address: &str, session: SessionState);
    
    /// Check if session exists for address
    fn contains_session(&self, address: &str) -> bool;
    
    /// Delete session for address
    fn delete_session(&mut self, address: &str);
    
    /// Get all session addresses
    fn get_sub_device_sessions(&self, base_address: &str) -> Vec<String>;
}

/// In-memory session store implementation
#[derive(Debug, Default)]
pub struct MemorySessionStore {
    sessions: HashMap<String, SessionState>,
}

impl MemorySessionStore {
    /// Create new memory session store
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionStore for MemorySessionStore {
    fn load_session(&self, address: &str) -> Option<SessionState> {
        self.sessions.get(address).cloned()
    }
    
    fn store_session(&mut self, address: &str, session: SessionState) {
        self.sessions.insert(address.to_string(), session);
    }
    
    fn contains_session(&self, address: &str) -> bool {
        self.sessions.contains_key(address)
    }
    
    fn delete_session(&mut self, address: &str) {
        self.sessions.remove(address);
    }
    
    fn get_sub_device_sessions(&self, base_address: &str) -> Vec<String> {
        self.sessions.keys()
            .filter(|addr| addr.starts_with(base_address))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::prekey::PreKeyBundle;
    
    #[test]
    fn test_session_state_creation() {
        let local_identity = [1u8; 32];
        let remote_identity = [2u8; 32];
        let root_key = [3u8; 32];
        
        let session = SessionState::new(local_identity, remote_identity, root_key);
        
        assert_eq!(session.version, SIGNAL_PROTOCOL_VERSION);
        assert_eq!(session.local_identity_key, local_identity);
        assert_eq!(session.remote_identity_key, remote_identity);
        assert_eq!(session.root_key, root_key);
        assert_eq!(session.send_message_number, 0);
    }
    
    #[test]
    fn test_alice_session_initialization() {
        let alice_identity = SigningKeyPair::generate();
        let bob_identity = SigningKeyPair::generate();
        let ephemeral = ECKeyPair::generate();
        
        // Create Bob's bundle
        let bundle = PreKeyBundle::new(&bob_identity, 1, Some(2), 12345, 1).unwrap();
        
        // Alice initializes session
        let result = SessionState::initialize_alice_session(&alice_identity, &bundle, &ephemeral);
        assert!(result.is_ok());
        
        let (session, _ephemeral_pub) = result.unwrap();
        assert!(session.has_pending_prekey());
        assert_eq!(session.local_identity_key, alice_identity.public_bytes());
    }
    
    #[test]
    fn test_memory_session_store() {
        let mut store = MemorySessionStore::new();
        let address = "test@example.com";
        
        let session = SessionState::new([1u8; 32], [2u8; 32], [3u8; 32]);
        
        // Store session
        store.store_session(address, session.clone());
        assert!(store.contains_session(address));
        
        // Load session
        let loaded = store.load_session(address).unwrap();
        assert_eq!(loaded.local_identity_key, session.local_identity_key);
        
        // Delete session
        store.delete_session(address);
        assert!(!store.contains_session(address));
    }
    
    #[test]
    fn test_chain_state() {
        let chain = ChainState {
            chain_key: [1u8; 32],
            message_number: 5,
            ephemeral_public: Some([2u8; 32]),
        };
        
        assert_eq!(chain.message_number, 5);
        assert_eq!(chain.ephemeral_public.unwrap(), [2u8; 32]);
    }
    
    #[test]
    fn test_pending_prekey() {
        let pending = PendingPreKey {
            signed_prekey_id: 1,
            prekey_id: Some(2),
            base_key: [3u8; 32],
        };
        
        assert_eq!(pending.signed_prekey_id, 1);
        assert_eq!(pending.prekey_id, Some(2));
        assert_eq!(pending.base_key, [3u8; 32]);
    }
}