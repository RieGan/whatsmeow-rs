/// Connection management, error recovery, and rate limiting for WhatsApp client

pub mod manager;
pub mod retry;
pub mod rate_limit;

use crate::{
    error::{Error, Result},
    socket::NoiseSocket,
};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use serde::{Serialize, Deserialize};

/// Connection state tracking
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Attempting to connect
    Connecting,
    /// Successfully connected
    Connected,
    /// Connection lost, attempting to reconnect
    Reconnecting { attempt: u32, last_attempt: Instant },
    /// Failed to connect after max retries
    Failed { reason: String },
}

/// Connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Maximum number of reconnection attempts
    pub max_reconnect_attempts: u32,
    /// Initial reconnection delay
    pub initial_reconnect_delay: Duration,
    /// Maximum reconnection delay
    pub max_reconnect_delay: Duration,
    /// Backoff multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Keep-alive interval
    pub keepalive_interval: Duration,
    /// Max idle time before considering connection stale
    pub max_idle_time: Duration,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            max_reconnect_attempts: 10,
            initial_reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            connection_timeout: Duration::from_secs(30),
            keepalive_interval: Duration::from_secs(30),
            max_idle_time: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Total connection attempts
    pub total_attempts: u64,
    /// Successful connections
    pub successful_connections: u64,
    /// Failed connections
    pub failed_connections: u64,
    /// Current connection duration
    pub current_connection_duration: Option<Duration>,
    /// Last connection time
    pub last_connection_time: Option<Instant>,
    /// Last disconnection time
    pub last_disconnection_time: Option<Instant>,
    /// Total uptime
    pub total_uptime: Duration,
    /// Average connection duration
    pub average_connection_duration: Duration,
}

impl ConnectionStats {
    /// Record a connection attempt
    pub fn record_attempt(&mut self) {
        self.total_attempts += 1;
    }
    
    /// Record a successful connection
    pub fn record_success(&mut self) {
        self.successful_connections += 1;
        self.last_connection_time = Some(Instant::now());
    }
    
    /// Record a failed connection
    pub fn record_failure(&mut self) {
        self.failed_connections += 1;
    }
    
    /// Record disconnection
    pub fn record_disconnection(&mut self) {
        if let Some(connected_at) = self.last_connection_time {
            let connection_duration = connected_at.elapsed();
            self.total_uptime += connection_duration;
            self.current_connection_duration = Some(connection_duration);
            
            // Update average
            if self.successful_connections > 0 {
                self.average_connection_duration = 
                    self.total_uptime / self.successful_connections as u32;
            }
        }
        
        self.last_disconnection_time = Some(Instant::now());
        self.last_connection_time = None;
    }
    
    /// Get success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_attempts == 0 {
            0.0
        } else {
            (self.successful_connections as f64 / self.total_attempts as f64) * 100.0
        }
    }
    
    /// Check if connection is currently active
    pub fn is_connected(&self) -> bool {
        self.last_connection_time.is_some() && self.last_disconnection_time
            .map(|disc| self.last_connection_time.unwrap() > disc)
            .unwrap_or(true)
    }
}

/// Connection event types
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// Connection established
    Connected,
    /// Connection lost
    Disconnected { reason: String },
    /// Reconnection attempt started
    ReconnectAttempt { attempt: u32 },
    /// Reconnection successful
    Reconnected,
    /// Reconnection failed
    ReconnectFailed { attempt: u32, reason: String },
    /// Max reconnection attempts reached
    ReconnectExhausted,
    /// Keep-alive ping sent
    KeepAlivePing,
    /// Keep-alive pong received
    KeepAlivePong,
    /// Connection timeout
    Timeout,
    /// Rate limit hit
    RateLimited { retry_after: Duration },
}

/// Connection event handler trait
pub trait ConnectionEventHandler: Send + Sync {
    /// Handle connection event
    fn handle_event(&self, event: ConnectionEvent);
}

/// Simple logging event handler
pub struct LoggingEventHandler;

impl ConnectionEventHandler for LoggingEventHandler {
    fn handle_event(&self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Connected => {
                tracing::info!("WebSocket connection established");
            }
            ConnectionEvent::Disconnected { reason } => {
                tracing::warn!("WebSocket connection lost: {}", reason);
            }
            ConnectionEvent::ReconnectAttempt { attempt } => {
                tracing::info!("Attempting reconnection #{}", attempt);
            }
            ConnectionEvent::Reconnected => {
                tracing::info!("Successfully reconnected to WhatsApp");
            }
            ConnectionEvent::ReconnectFailed { attempt, reason } => {
                tracing::warn!("Reconnection attempt #{} failed: {}", attempt, reason);
            }
            ConnectionEvent::ReconnectExhausted => {
                tracing::error!("Max reconnection attempts reached, giving up");
            }
            ConnectionEvent::KeepAlivePing => {
                tracing::debug!("Sent keep-alive ping");
            }
            ConnectionEvent::KeepAlivePong => {
                tracing::debug!("Received keep-alive pong");
            }
            ConnectionEvent::Timeout => {
                tracing::warn!("Connection timeout");
            }
            ConnectionEvent::RateLimited { retry_after } => {
                tracing::warn!("Rate limited, retry after {:?}", retry_after);
            }
        }
    }
}

/// Calculate exponential backoff delay
pub fn calculate_backoff_delay(
    attempt: u32,
    initial_delay: Duration,
    max_delay: Duration,
    multiplier: f64,
) -> Duration {
    let delay_secs = initial_delay.as_secs_f64() * multiplier.powi(attempt as i32);
    let capped_delay = delay_secs.min(max_delay.as_secs_f64());
    Duration::from_secs_f64(capped_delay)
}

/// Jitter calculation for backoff
pub fn add_jitter(delay: Duration, jitter_factor: f64) -> Duration {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let jitter = rng.gen_range(0.0..jitter_factor);
    let jittered_secs = delay.as_secs_f64() * (1.0 + jitter);
    Duration::from_secs_f64(jittered_secs)
}

/// Check if error is recoverable
pub fn is_recoverable_error(error: &Error) -> bool {
    match error {
        // Network errors are usually recoverable
        Error::WebSocket(_) => true,
        Error::Connection(_) => true,
        Error::Disconnected(_) => true,
        
        // IO errors might be recoverable
        Error::Io(_) => true,
        
        // Protocol errors usually aren't
        Error::Protocol(_) => false,
        Error::Auth(_) => false,
        Error::Crypto(_) => false,
        
        // Other errors
        Error::NotLoggedIn => false,
        Error::InvalidJID(_) => false,
        Error::IQ { .. } => false,
        Error::Database(_) => false,
        
        // JSON/Protobuf errors usually indicate a bug
        Error::Json(_) => false,
        Error::ProtobufDecode(_) => false,
        Error::UrlParse(_) => false,
        Error::ElementMissing(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_connection_stats() {
        let mut stats = ConnectionStats::default();
        
        // Test initial state
        assert_eq!(stats.success_rate(), 0.0);
        assert!(!stats.is_connected());
        
        // Test recording attempts and successes
        stats.record_attempt();
        stats.record_success();
        assert_eq!(stats.success_rate(), 100.0);
        assert!(stats.is_connected());
        
        // Test recording failures
        stats.record_attempt();
        stats.record_failure();
        assert_eq!(stats.success_rate(), 50.0);
        
        // Test disconnection
        stats.record_disconnection();
        assert!(!stats.is_connected());
    }
    
    #[test]
    fn test_backoff_calculation() {
        let initial = Duration::from_secs(1);
        let max = Duration::from_secs(60);
        let multiplier = 2.0;
        
        // Test exponential growth
        assert_eq!(calculate_backoff_delay(0, initial, max, multiplier), Duration::from_secs(1));
        assert_eq!(calculate_backoff_delay(1, initial, max, multiplier), Duration::from_secs(2));
        assert_eq!(calculate_backoff_delay(2, initial, max, multiplier), Duration::from_secs(4));
        assert_eq!(calculate_backoff_delay(3, initial, max, multiplier), Duration::from_secs(8));
        
        // Test capping at max delay
        let large_delay = calculate_backoff_delay(10, initial, max, multiplier);
        assert!(large_delay <= max);
    }
    
    #[test]
    fn test_jitter() {
        let delay = Duration::from_secs(10);
        let jittered = add_jitter(delay, 0.1);
        
        // Jittered delay should be within 10% of original
        assert!(jittered >= delay);
        assert!(jittered <= Duration::from_secs_f64(delay.as_secs_f64() * 1.1));
    }
    
    #[test]
    fn test_recoverable_errors() {
        // Recoverable errors
        assert!(is_recoverable_error(&Error::Connection("test".to_string())));
        assert!(is_recoverable_error(&Error::Disconnected("test".to_string())));
        
        // Non-recoverable errors
        assert!(!is_recoverable_error(&Error::Auth("test".to_string())));
        assert!(!is_recoverable_error(&Error::Protocol("test".to_string())));
        assert!(!is_recoverable_error(&Error::InvalidJID("test".to_string())));
    }
    
    #[test]
    fn test_connection_config_defaults() {
        let config = ConnectionConfig::default();
        assert_eq!(config.max_reconnect_attempts, 10);
        assert_eq!(config.initial_reconnect_delay, Duration::from_secs(1));
        assert_eq!(config.max_reconnect_delay, Duration::from_secs(60));
        assert_eq!(config.backoff_multiplier, 2.0);
    }
}