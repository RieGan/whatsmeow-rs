/// Enhanced QR code system for WhatsApp authentication
/// 
/// This module provides a complete QR code implementation matching the WhatsApp protocol,
/// including QR generation, channel management, timeout handling, and refresh cycles.

use crate::{
    error::{Error, Result}, 
    util::keys::{ECKeyPair, SigningKeyPair},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, watch};
use tracing::{debug, warn, info};

/// QR code event types
#[derive(Debug, Clone, PartialEq)]
pub enum QREvent {
    /// New QR code generated
    Code { 
        code: String, 
        timeout: Duration,
        expires_at: Instant,
    },
    /// QR code scanning succeeded
    Success,
    /// QR code operation timed out
    Timeout,
    /// Pairing error occurred
    Error(String),
    /// Client version is outdated
    ClientOutdated,
    /// QR scanned but without multidevice support
    ScannedWithoutMultidevice,
    /// Unexpected state during pairing
    UnexpectedState,
}

/// Enhanced QR data with WhatsApp protocol compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QRData {
    /// Reference ID from server
    pub ref_id: String,
    /// Noise protocol public key
    pub noise_public_key: Vec<u8>,
    /// Identity public key
    pub identity_public_key: Vec<u8>,
    /// Advertisement secret key
    pub adv_secret: Vec<u8>,
    /// Generation timestamp
    pub generated_at: SystemTime,
    /// Expiration time
    pub expires_at: SystemTime,
}

impl QRData {
    /// Generate new QR data with proper WhatsApp format
    pub fn new(ref_id: String, noise_keypair: &ECKeyPair, identity_keypair: &SigningKeyPair, adv_secret: Vec<u8>) -> Self {
        let now = SystemTime::now();
        let expires_at = now + Duration::from_secs(20); // QR codes expire after 20 seconds
        
        Self {
            ref_id,
            noise_public_key: noise_keypair.public_bytes().to_vec(),
            identity_public_key: identity_keypair.public_bytes().to_vec(),
            adv_secret,
            generated_at: now,
            expires_at,
        }
    }
    
    /// Create QR data from individual components (for server responses)
    pub fn from_components(
        ref_id: String,
        noise_public_key: Vec<u8>,
        identity_public_key: Vec<u8>, 
        adv_secret: Vec<u8>
    ) -> Self {
        let now = SystemTime::now();
        let expires_at = now + Duration::from_secs(20);
        
        Self {
            ref_id,
            noise_public_key,
            identity_public_key,
            adv_secret,
            generated_at: now,
            expires_at,
        }
    }
    
    /// Generate QR code string in WhatsApp format
    /// Format: "ref_id,noise_key_base64,identity_key_base64,adv_secret_base64"
    pub fn to_qr_string(&self) -> String {
        let noise_b64 = STANDARD.encode(&self.noise_public_key);
        let identity_b64 = STANDARD.encode(&self.identity_public_key);
        let adv_b64 = STANDARD.encode(&self.adv_secret);
        
        format!("{},{},{},{}", self.ref_id, noise_b64, identity_b64, adv_b64)
    }
    
    /// Parse QR code string into QR data
    pub fn from_qr_string(qr_string: &str) -> Result<Self> {
        let parts: Vec<&str> = qr_string.split(',').collect();
        if parts.len() != 4 {
            return Err(Error::Protocol("Invalid QR code format, expected 4 parts".to_string()));
        }
        
        let ref_id = parts[0].to_string();
        let noise_public_key = STANDARD.decode(parts[1])
            .map_err(|e| Error::Protocol(format!("Invalid noise key encoding: {}", e)))?;
        let identity_public_key = STANDARD.decode(parts[2])
            .map_err(|e| Error::Protocol(format!("Invalid identity key encoding: {}", e)))?;
        let adv_secret = STANDARD.decode(parts[3])
            .map_err(|e| Error::Protocol(format!("Invalid ADV secret encoding: {}", e)))?;
            
        Ok(Self::from_components(ref_id, noise_public_key, identity_public_key, adv_secret))
    }
    
    /// Check if QR code has expired
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }
    
    /// Get remaining time until expiration
    pub fn time_until_expiration(&self) -> Option<Duration> {
        self.expires_at.duration_since(SystemTime::now()).ok()
    }
}

/// QR channel configuration
#[derive(Debug, Clone)]
pub struct QRChannelConfig {
    /// Initial timeout for first QR code (60 seconds)
    pub initial_timeout: Duration,
    /// Standard timeout for subsequent QR codes (20 seconds)
    pub standard_timeout: Duration,
    /// Maximum number of QR codes to generate
    pub max_codes: usize,
    /// Buffer size for the event channel
    pub channel_buffer_size: usize,
}

impl Default for QRChannelConfig {
    fn default() -> Self {
        Self {
            initial_timeout: Duration::from_secs(60),
            standard_timeout: Duration::from_secs(20),
            max_codes: 6, // WhatsApp typically sends 6 QR codes max
            channel_buffer_size: 16,
        }
    }
}

/// QR channel manager for handling QR code lifecycle
pub struct QRChannel {
    config: QRChannelConfig,
    noise_keypair: ECKeyPair,
    identity_keypair: SigningKeyPair,
    adv_secret: Vec<u8>,
    event_sender: mpsc::Sender<QREvent>,
    event_receiver: mpsc::Receiver<QREvent>,
    shutdown_sender: watch::Sender<bool>,
    shutdown_receiver: watch::Receiver<bool>,
    is_active: bool,
    codes_generated: usize,
}

impl QRChannel {
    /// Create a new QR channel with default configuration
    pub fn new() -> Self {
        Self::with_config(QRChannelConfig::default())
    }
    
    /// Create a new QR channel with custom configuration
    pub fn with_config(config: QRChannelConfig) -> Self {
        let (event_sender, event_receiver) = mpsc::channel(config.channel_buffer_size);
        let (shutdown_sender, shutdown_receiver) = watch::channel(false);
        
        Self {
            config,
            noise_keypair: ECKeyPair::generate(),
            identity_keypair: SigningKeyPair::generate(),
            adv_secret: crate::util::crypto::random_bytes(32),
            event_sender,
            event_receiver,
            shutdown_sender,
            shutdown_receiver,
            is_active: false,
            codes_generated: 0,
        }
    }
    
    /// Create QR channel with existing keys
    pub fn with_keys(
        config: QRChannelConfig,
        noise_keypair: ECKeyPair,
        identity_keypair: SigningKeyPair,
        adv_secret: Vec<u8>
    ) -> Self {
        let (event_sender, event_receiver) = mpsc::channel(config.channel_buffer_size);
        let (shutdown_sender, shutdown_receiver) = watch::channel(false);
        
        Self {
            config,
            noise_keypair,
            identity_keypair,
            adv_secret,
            event_sender,
            event_receiver,
            shutdown_sender,
            shutdown_receiver,
            is_active: false,
            codes_generated: 0,
        }
    }
    
    /// Start the QR code generation process
    pub async fn start(&mut self, ref_codes: Vec<String>) -> Result<()> {
        if self.is_active {
            return Err(Error::Auth("QR channel is already active".to_string()));
        }
        
        if ref_codes.is_empty() {
            return Err(Error::Auth("No reference codes provided".to_string()));
        }
        
        self.is_active = true;
        self.codes_generated = 0;
        
        info!("Starting QR channel with {} reference codes", ref_codes.len());
        
        // Start QR generation task
        let event_sender = self.event_sender.clone();
        let mut shutdown_receiver = self.shutdown_receiver.clone();
        let config = self.config.clone();
        let noise_keypair = self.noise_keypair.clone();
        let identity_keypair = self.identity_keypair.clone();
        let adv_secret = self.adv_secret.clone();
        
        tokio::spawn(async move {
            Self::qr_generation_task(
                ref_codes,
                config,
                noise_keypair,
                identity_keypair,
                adv_secret,
                event_sender,
                shutdown_receiver,
            ).await;
        });
        
        Ok(())
    }
    
    /// QR code generation background task
    async fn qr_generation_task(
        ref_codes: Vec<String>,
        config: QRChannelConfig,
        noise_keypair: ECKeyPair,
        identity_keypair: SigningKeyPair,
        adv_secret: Vec<u8>,
        event_sender: mpsc::Sender<QREvent>,
        mut shutdown_receiver: watch::Receiver<bool>,
    ) {
        let mut codes_iter = ref_codes.into_iter();
        let mut codes_sent = 0;
        
        while codes_sent < config.max_codes {
            // Check for shutdown signal
            if *shutdown_receiver.borrow() {
                debug!("QR generation task received shutdown signal");
                break;
            }
            
            // Get next reference code
            let ref_id = match codes_iter.next() {
                Some(code) => code,
                None => {
                    warn!("Ran out of reference codes, sending timeout event");
                    let _ = event_sender.send(QREvent::Timeout).await;
                    break;
                }
            };
            
            // Generate QR data
            let qr_data = QRData::new(ref_id, &noise_keypair, &identity_keypair, adv_secret.clone());
            let qr_string = qr_data.to_qr_string();
            
            // Determine timeout (first code gets longer timeout)
            let timeout = if codes_sent == 0 {
                config.initial_timeout
            } else {
                config.standard_timeout
            };
            
            let expires_at = Instant::now() + timeout;
            
            debug!("Generating QR code {} with timeout {:?}", codes_sent + 1, timeout);
            
            // Send QR code event
            let event = QREvent::Code {
                code: qr_string,
                timeout,
                expires_at,
            };
            
            if let Err(e) = event_sender.send(event).await {
                warn!("Failed to send QR code event: {}", e);
                break;
            }
            
            codes_sent += 1;
            
            // Wait for timeout or shutdown
            tokio::select! {
                _ = tokio::time::sleep(timeout) => {
                    debug!("QR code {} timed out, generating next", codes_sent);
                }
                _ = shutdown_receiver.changed() => {
                    if *shutdown_receiver.borrow() {
                        debug!("QR generation task shutting down");
                        break;
                    }
                }
            }
        }
        
        // If we exhausted all codes, send timeout
        if codes_sent >= config.max_codes {
            warn!("Exhausted all QR codes, sending timeout event");
            let _ = event_sender.send(QREvent::Timeout).await;
        }
        
        debug!("QR generation task completed");
    }
    
    /// Get the next QR event
    pub async fn next_event(&mut self) -> Option<QREvent> {
        self.event_receiver.recv().await
    }
    
    /// Stop the QR channel
    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_active {
            return Ok(());
        }
        
        debug!("Stopping QR channel");
        
        self.is_active = false;
        if let Err(e) = self.shutdown_sender.send(true) {
            warn!("Failed to send shutdown signal: {}", e);
        }
        
        // Drain any remaining events
        while let Ok(_) = self.event_receiver.try_recv() {
            // Drain the channel
        }
        
        Ok(())
    }
    
    /// Signal successful pairing
    pub async fn signal_success(&self) -> Result<()> {
        self.event_sender.send(QREvent::Success).await
            .map_err(|e| Error::Auth(format!("Failed to signal success: {}", e)))
    }
    
    /// Signal pairing error
    pub async fn signal_error(&self, error: String) -> Result<()> {
        self.event_sender.send(QREvent::Error(error)).await
            .map_err(|e| Error::Auth(format!("Failed to signal error: {}", e)))
    }
    
    /// Signal client outdated
    pub async fn signal_client_outdated(&self) -> Result<()> {
        self.event_sender.send(QREvent::ClientOutdated).await
            .map_err(|e| Error::Auth(format!("Failed to signal client outdated: {}", e)))
    }
    
    /// Signal scanned without multidevice
    pub async fn signal_scanned_without_multidevice(&self) -> Result<()> {
        self.event_sender.send(QREvent::ScannedWithoutMultidevice).await
            .map_err(|e| Error::Auth(format!("Failed to signal scanned without multidevice: {}", e)))
    }
    
    /// Signal unexpected state
    pub async fn signal_unexpected_state(&self) -> Result<()> {
        self.event_sender.send(QREvent::UnexpectedState).await
            .map_err(|e| Error::Auth(format!("Failed to signal unexpected state: {}", e)))
    }
    
    /// Get noise keypair
    pub fn noise_keypair(&self) -> &ECKeyPair {
        &self.noise_keypair
    }
    
    /// Get identity keypair
    pub fn identity_keypair(&self) -> &SigningKeyPair {
        &self.identity_keypair
    }
    
    /// Get ADV secret
    pub fn adv_secret(&self) -> &[u8] {
        &self.adv_secret
    }
    
    /// Check if channel is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }
    
    /// Get number of codes generated so far
    pub fn codes_generated(&self) -> usize {
        self.codes_generated
    }
}

impl Default for QRChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// QR channel iterator for convenient usage
pub struct QRChannelIterator {
    channel: QRChannel,
}

impl QRChannelIterator {
    /// Create a new QR channel iterator
    pub fn new(channel: QRChannel) -> Self {
        Self { channel }
    }
    
    /// Start the iterator with reference codes
    pub async fn start(&mut self, ref_codes: Vec<String>) -> Result<()> {
        self.channel.start(ref_codes).await
    }
    
    /// Stop the iterator
    pub async fn stop(&mut self) -> Result<()> {
        self.channel.stop().await
    }
}

impl QRChannelIterator {
    /// Get the next QR event
    pub async fn next(&mut self) -> Option<QREvent> {
        self.channel.next_event().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;
    
    #[test]
    fn test_qr_data_creation() {
        let noise_keypair = ECKeyPair::generate();
        let identity_keypair = SigningKeyPair::generate();
        let adv_secret = vec![1, 2, 3, 4];
        let ref_id = "test-ref-123".to_string();
        
        let qr_data = QRData::new(ref_id.clone(), &noise_keypair, &identity_keypair, adv_secret.clone());
        
        assert_eq!(qr_data.ref_id, ref_id);
        assert_eq!(qr_data.noise_public_key, noise_keypair.public_bytes());
        assert_eq!(qr_data.identity_public_key, identity_keypair.public_bytes());
        assert_eq!(qr_data.adv_secret, adv_secret);
        assert!(!qr_data.is_expired()); // Should not be expired immediately
    }
    
    #[test]
    fn test_qr_string_round_trip() {
        let noise_keypair = ECKeyPair::generate();
        let identity_keypair = SigningKeyPair::generate();
        let adv_secret = vec![1, 2, 3, 4];
        let ref_id = "test-ref-123".to_string();
        
        let original = QRData::new(ref_id, &noise_keypair, &identity_keypair, adv_secret);
        let qr_string = original.to_qr_string();
        let parsed = QRData::from_qr_string(&qr_string).unwrap();
        
        assert_eq!(original.ref_id, parsed.ref_id);
        assert_eq!(original.noise_public_key, parsed.noise_public_key);
        assert_eq!(original.identity_public_key, parsed.identity_public_key);
        assert_eq!(original.adv_secret, parsed.adv_secret);
    }
    
    #[tokio::test]
    async fn test_qr_channel_creation() {
        let mut qr_channel = QRChannel::new();
        assert!(!qr_channel.is_active());
        assert_eq!(qr_channel.codes_generated(), 0);
        
        // Test starting with empty codes
        let result = qr_channel.start(vec![]).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_qr_channel_with_codes() {
        let mut qr_channel = QRChannel::with_config(QRChannelConfig {
            initial_timeout: Duration::from_millis(100),
            standard_timeout: Duration::from_millis(50),
            max_codes: 2,
            channel_buffer_size: 16,
        });
        
        let ref_codes = vec!["ref1".to_string(), "ref2".to_string()];
        qr_channel.start(ref_codes).await.unwrap();
        
        assert!(qr_channel.is_active());
        
        // Should receive first QR code
        let event = timeout(Duration::from_millis(200), qr_channel.next_event()).await.unwrap();
        match event {
            Some(QREvent::Code { code, timeout: _, expires_at: _ }) => {
                assert!(code.contains("ref1"));
            }
            other => panic!("Expected QR code event, got {:?}", other),
        }
        
        qr_channel.stop().await.unwrap();
        assert!(!qr_channel.is_active());
    }
    
    #[tokio::test]
    async fn test_qr_channel_timeout() {
        let mut qr_channel = QRChannel::with_config(QRChannelConfig {
            initial_timeout: Duration::from_millis(50),
            standard_timeout: Duration::from_millis(50),
            max_codes: 1,
            channel_buffer_size: 16,
        });
        
        let ref_codes = vec!["ref1".to_string()];
        qr_channel.start(ref_codes).await.unwrap();
        
        // Should receive QR code first
        let event = timeout(Duration::from_millis(200), qr_channel.next_event()).await.unwrap();
        assert!(matches!(event, Some(QREvent::Code { .. })));
        
        // Should receive timeout after codes are exhausted
        let event = timeout(Duration::from_millis(200), qr_channel.next_event()).await.unwrap();
        assert!(matches!(event, Some(QREvent::Timeout)));
        
        qr_channel.stop().await.unwrap();
    }
    
    #[test]
    fn test_qr_data_expiration() {
        let noise_keypair = ECKeyPair::generate();
        let identity_keypair = SigningKeyPair::generate();
        let adv_secret = vec![1, 2, 3, 4];
        let ref_id = "test-ref-123".to_string();
        
        let mut qr_data = QRData::new(ref_id, &noise_keypair, &identity_keypair, adv_secret);
        
        // Manually set expiration to past
        qr_data.expires_at = SystemTime::now() - Duration::from_secs(1);
        
        assert!(qr_data.is_expired());
        assert!(qr_data.time_until_expiration().is_none());
    }
}