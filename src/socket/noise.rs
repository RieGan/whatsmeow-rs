use crate::{
    error::{Error, Result},
    util::crypto::{AesGcm, hkdf_expand, sha256},
};
use std::sync::atomic::{AtomicU32, Ordering};

/// Noise protocol handshake implementation for WhatsApp
pub struct NoiseHandshake {
    hash: Vec<u8>,
    salt: Vec<u8>,
    key: Option<AesGcm>,
    counter: AtomicU32,
    completed: bool,
}

impl NoiseHandshake {
    /// Create a new noise handshake
    pub fn new() -> Self {
        Self {
            hash: Vec::new(),
            salt: Vec::new(),
            key: None,
            counter: AtomicU32::new(0),
            completed: false,
        }
    }
    
    /// Start the handshake with a pattern and header
    pub fn start(&mut self, pattern: &str, header: &[u8]) -> Result<()> {
        let pattern_bytes = pattern.as_bytes();
        
        self.hash = if pattern_bytes.len() == 32 {
            pattern_bytes.to_vec()
        } else {
            sha256(pattern_bytes)
        };
        
        self.salt = self.hash.clone();
        self.key = Some(AesGcm::new(&self.hash)?);
        self.authenticate(header);
        Ok(())
    }
    
    /// Authenticate data by mixing it into the hash
    pub fn authenticate(&mut self, data: &[u8]) {
        let mut combined = self.hash.clone();
        combined.extend_from_slice(data);
        self.hash = sha256(&combined);
    }
    
    /// Encrypt plaintext using the current key
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let key = self.key.as_ref().ok_or_else(|| {
            Error::Crypto("Handshake not started".to_string())
        })?;
        
        let nonce = self.generate_iv();
        let ciphertext = key.encrypt(&nonce, plaintext)?;
        self.authenticate(&ciphertext);
        Ok(ciphertext)
    }
    
    /// Decrypt ciphertext using the current key
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let key = self.key.as_ref().ok_or_else(|| {
            Error::Crypto("Handshake not started".to_string())
        })?;
        
        let nonce = self.generate_iv();
        let plaintext = key.decrypt(&nonce, ciphertext)?;
        self.authenticate(ciphertext);
        Ok(plaintext)
    }
    
    /// Mix shared secret into the key material
    pub fn mix_shared_secret_into_key(&mut self, private_key: &[u8; 32], public_key: &[u8; 32]) -> Result<()> {
        use crate::util::keys::ECKeyPair;
        
        // Create key pair from private key and perform ECDH
        let keypair = ECKeyPair::from_private_bytes(private_key)?;
        let shared_secret = keypair.ecdh(public_key);
        
        self.mix_into_key(&shared_secret)
    }
    
    /// Mix data into the key material
    pub fn mix_into_key(&mut self, data: &[u8]) -> Result<()> {
        self.counter.store(0, Ordering::SeqCst);
        
        let (write_key, read_key) = self.extract_and_expand(&self.salt, data)?;
        self.salt = write_key.clone();
        self.key = Some(AesGcm::new(&read_key)?);
        
        Ok(())
    }
    
    /// Finish the handshake and derive final keys
    pub fn finish(&self) -> Result<(Vec<u8>, Vec<u8>)> {
        self.extract_and_expand(&self.salt, &[])
    }
    
    /// Extract and expand key material using HKDF
    fn extract_and_expand(&self, salt: &[u8], data: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        // Derive 64 bytes total (32 for write key, 32 for read key)
        let expanded = hkdf_expand(data, salt, 64)?;
        
        let write_key = expanded[0..32].to_vec();
        let read_key = expanded[32..64].to_vec();
        
        Ok((write_key, read_key))
    }
    
    /// Generate IV for encryption/decryption
    fn generate_iv(&self) -> [u8; 12] {
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut iv = [0u8; 12];
        iv[8..12].copy_from_slice(&counter.to_be_bytes());
        iv
    }
    
    /// Check if handshake is completed
    pub fn is_completed(&self) -> bool {
        self.completed
    }
    
    /// Create initial client handshake message
    pub fn create_client_init(&mut self) -> Result<Vec<u8>> {
        use crate::util::keys::ECKeyPair;
        
        // Initialize handshake with WhatsApp Noise pattern
        let pattern = "Noise_XX_25519_AESGCM_SHA256";
        let header = b"WA";
        self.start(pattern, header)?;
        
        // Generate ephemeral key pair
        let ephemeral_keypair = ECKeyPair::generate();
        
        // Build initial message: header + public key
        let mut message = Vec::new();
        message.extend_from_slice(header);
        message.extend_from_slice(&ephemeral_keypair.public_bytes());
        
        tracing::debug!("Created client init message of {} bytes", message.len());
        Ok(message)
    }
    
    /// Process server response during handshake
    pub fn process_server_response(&mut self, response: &[u8]) -> Result<()> {
        if response.len() < 32 {
            return Err(Error::Auth("Invalid server response length".to_string()));
        }
        
        // Extract server's ephemeral public key
        let server_public_key = &response[0..32];
        let payload = &response[32..];
        
        tracing::debug!("Processing server response: {} bytes", response.len());
        
        // Mix the server's public key into our handshake state
        self.authenticate(server_public_key);
        
        // If there's payload, decrypt it
        if !payload.is_empty() {
            let _decrypted = self.decrypt(payload)?;
            tracing::debug!("Decrypted server payload");
        }
        
        Ok(())
    }
    
    /// Create client finish message
    pub fn create_client_finish(&mut self) -> Result<Vec<u8>> {
        use crate::util::keys::ECKeyPair;
        
        // Generate static key pair for authentication
        let static_keypair = ECKeyPair::generate();
        
        // Create finish message with static public key
        let mut message = Vec::new();
        message.extend_from_slice(&static_keypair.public_bytes());
        
        // Encrypt the message
        let encrypted = self.encrypt(&message)?;
        
        // Mark handshake as completed
        self.completed = true;
        
        tracing::debug!("Created client finish message of {} bytes", encrypted.len());
        Ok(encrypted)
    }
}

impl Default for NoiseHandshake {
    fn default() -> Self {
        Self::new()
    }
}