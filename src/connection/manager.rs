/// Connection manager with automatic reconnection and error recovery

use super::{
    ConnectionState, ConnectionConfig, ConnectionStats, ConnectionEvent, 
    ConnectionEventHandler, LoggingEventHandler, calculate_backoff_delay, 
    is_recoverable_error,
};
use crate::{
    error::{Error, Result},
    socket::NoiseSocket,
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    sync::{broadcast, mpsc, RwLock},
    time::{sleep, timeout},
    task::JoinHandle,
};

/// Connection manager that handles automatic reconnection
pub struct ConnectionManager {
    /// Current connection state
    state: Arc<RwLock<ConnectionState>>,
    /// Connection configuration
    config: ConnectionConfig,
    /// Connection statistics
    stats: Arc<Mutex<ConnectionStats>>,
    /// Event handlers
    event_handlers: Arc<RwLock<Vec<Box<dyn ConnectionEventHandler>>>>,
    /// Event broadcaster
    event_sender: broadcast::Sender<ConnectionEvent>,
    /// Command channel for controlling the manager
    command_sender: Option<mpsc::UnboundedSender<ConnectionCommand>>,
    /// Background task handle
    task_handle: Option<JoinHandle<()>>,
}

/// Commands for controlling the connection manager
#[derive(Debug)]
enum ConnectionCommand {
    /// Connect to WhatsApp
    Connect,
    /// Disconnect from WhatsApp
    Disconnect,
    /// Force reconnection
    Reconnect,
    /// Update configuration
    UpdateConfig(ConnectionConfig),
    /// Shutdown the manager
    Shutdown,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(config: ConnectionConfig) -> Self {
        let (event_sender, _) = broadcast::channel(100);
        
        let mut manager = Self {
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            config,
            stats: Arc::new(Mutex::new(ConnectionStats::default())),
            event_handlers: Arc::new(RwLock::new(Vec::new())),
            event_sender,
            command_sender: None,
            task_handle: None,
        };
        
        // Add default logging handler
        manager.add_event_handler(Box::new(LoggingEventHandler));
        
        manager
    }
    
    /// Start the connection manager
    pub async fn start(&mut self) -> Result<()> {
        if self.task_handle.is_some() {
            return Err(Error::Connection("Connection manager already started".to_string()));
        }
        
        let (command_sender, command_receiver) = mpsc::unbounded_channel();
        self.command_sender = Some(command_sender);
        
        // Clone necessary data for the background task
        let state = Arc::clone(&self.state);
        let config = self.config.clone();
        let stats = Arc::clone(&self.stats);
        let event_handlers = Arc::clone(&self.event_handlers);
        let event_sender = self.event_sender.clone();
        
        // Start background connection management task
        let handle = tokio::spawn(async move {
            connection_management_task(
                state,
                config,
                stats,
                event_handlers,
                event_sender,
                command_receiver,
            ).await;
        });
        
        self.task_handle = Some(handle);
        
        Ok(())
    }
    
    /// Stop the connection manager
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(sender) = &self.command_sender {
            let _ = sender.send(ConnectionCommand::Shutdown);
        }
        
        if let Some(handle) = self.task_handle.take() {
            handle.await.map_err(|e| Error::Connection(format!("Failed to stop connection manager: {}", e)))?;
        }
        
        self.command_sender = None;
        
        Ok(())
    }
    
    /// Connect to WhatsApp
    pub async fn connect(&self) -> Result<()> {
        if let Some(sender) = &self.command_sender {
            sender.send(ConnectionCommand::Connect)
                .map_err(|e| Error::Connection(format!("Failed to send connect command: {}", e)))?;
        }
        Ok(())
    }
    
    /// Disconnect from WhatsApp
    pub async fn disconnect(&self) -> Result<()> {
        if let Some(sender) = &self.command_sender {
            sender.send(ConnectionCommand::Disconnect)
                .map_err(|e| Error::Connection(format!("Failed to send disconnect command: {}", e)))?;
        }
        Ok(())
    }
    
    /// Force reconnection
    pub async fn reconnect(&self) -> Result<()> {
        if let Some(sender) = &self.command_sender {
            sender.send(ConnectionCommand::Reconnect)
                .map_err(|e| Error::Connection(format!("Failed to send reconnect command: {}", e)))?;
        }
        Ok(())
    }
    
    /// Update connection configuration
    pub async fn update_config(&mut self, config: ConnectionConfig) -> Result<()> {
        self.config = config.clone();
        
        if let Some(sender) = &self.command_sender {
            sender.send(ConnectionCommand::UpdateConfig(config))
                .map_err(|e| Error::Connection(format!("Failed to send config update: {}", e)))?;
        }
        
        Ok(())
    }
    
    /// Get current connection state
    pub async fn get_state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }
    
    /// Get connection statistics
    pub fn get_stats(&self) -> ConnectionStats {
        self.stats.lock().unwrap().clone()
    }
    
    /// Add event handler
    pub fn add_event_handler(&mut self, handler: Box<dyn ConnectionEventHandler>) {
        let handlers = Arc::clone(&self.event_handlers);
        tokio::spawn(async move {
            handlers.write().await.push(handler);
        });
    }
    
    /// Subscribe to connection events
    pub fn subscribe_events(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.event_sender.subscribe()
    }
    
    /// Check if currently connected
    pub async fn is_connected(&self) -> bool {
        matches!(*self.state.read().await, ConnectionState::Connected)
    }
    
    /// Wait for connection to be established
    pub async fn wait_for_connection(&self, timeout_duration: Duration) -> Result<()> {
        let start = Instant::now();
        
        while start.elapsed() < timeout_duration {
            if self.is_connected().await {
                return Ok(());
            }
            
            sleep(Duration::from_millis(100)).await;
        }
        
        Err(Error::Connection("Timeout waiting for connection".to_string()))
    }
}

impl Drop for ConnectionManager {
    fn drop(&mut self) {
        if let Some(sender) = &self.command_sender {
            let _ = sender.send(ConnectionCommand::Shutdown);
        }
    }
}

/// Background task that handles connection management
async fn connection_management_task(
    state: Arc<RwLock<ConnectionState>>,
    mut config: ConnectionConfig,
    stats: Arc<Mutex<ConnectionStats>>,
    event_handlers: Arc<RwLock<Vec<Box<dyn ConnectionEventHandler>>>>,
    event_sender: broadcast::Sender<ConnectionEvent>,
    mut command_receiver: mpsc::UnboundedReceiver<ConnectionCommand>,
) {
    let mut current_socket: Option<NoiseSocket> = None;
    let mut keepalive_handle: Option<JoinHandle<()>> = None;
    
    loop {
        tokio::select! {
            // Handle commands
            command = command_receiver.recv() => {
                match command {
                    Some(ConnectionCommand::Connect) => {
                        if matches!(*state.read().await, ConnectionState::Disconnected) {
                            attempt_connection(
                                &state,
                                &config,
                                &stats,
                                &event_handlers,
                                &event_sender,
                                &mut current_socket,
                                &mut keepalive_handle,
                            ).await;
                        }
                    }
                    Some(ConnectionCommand::Disconnect) => {
                        disconnect(
                            &state,
                            &stats,
                            &event_handlers,
                            &event_sender,
                            &mut current_socket,
                            &mut keepalive_handle,
                        ).await;
                    }
                    Some(ConnectionCommand::Reconnect) => {
                        // Force reconnection
                        disconnect(
                            &state,
                            &stats,
                            &event_handlers,
                            &event_sender,
                            &mut current_socket,
                            &mut keepalive_handle,
                        ).await;
                        
                        attempt_connection(
                            &state,
                            &config,
                            &stats,
                            &event_handlers,
                            &event_sender,
                            &mut current_socket,
                            &mut keepalive_handle,
                        ).await;
                    }
                    Some(ConnectionCommand::UpdateConfig(new_config)) => {
                        config = new_config;
                    }
                    Some(ConnectionCommand::Shutdown) | None => {
                        disconnect(
                            &state,
                            &stats,
                            &event_handlers,
                            &event_sender,
                            &mut current_socket,
                            &mut keepalive_handle,
                        ).await;
                        break;
                    }
                }
            }
            
            // Handle connection monitoring
            _ = sleep(Duration::from_secs(5)) => {
                // Check if we need to reconnect
                let current_state = state.read().await.clone();
                
                match current_state {
                    ConnectionState::Reconnecting { attempt, last_attempt } => {
                        if attempt < config.max_reconnect_attempts {
                            let delay = calculate_backoff_delay(
                                attempt,
                                config.initial_reconnect_delay,
                                config.max_reconnect_delay,
                                config.backoff_multiplier,
                            );
                            
                            if last_attempt.elapsed() >= delay {
                                attempt_reconnection(
                                    &state,
                                    &config,
                                    &stats,
                                    &event_handlers,
                                    &event_sender,
                                    &mut current_socket,
                                    &mut keepalive_handle,
                                    attempt + 1,
                                ).await;
                            }
                        } else {
                            // Max attempts reached
                            *state.write().await = ConnectionState::Failed {
                                reason: "Max reconnection attempts reached".to_string()
                            };
                            
                            let event = ConnectionEvent::ReconnectExhausted;
                            broadcast_event(&event_handlers, &event_sender, event).await;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    
    tracing::info!("Connection management task stopped");
}

/// Attempt to establish a connection
async fn attempt_connection(
    state: &Arc<RwLock<ConnectionState>>,
    config: &ConnectionConfig,
    stats: &Arc<Mutex<ConnectionStats>>,
    event_handlers: &Arc<RwLock<Vec<Box<dyn ConnectionEventHandler>>>>,
    event_sender: &broadcast::Sender<ConnectionEvent>,
    current_socket: &mut Option<NoiseSocket>,
    keepalive_handle: &mut Option<JoinHandle<()>>,
) {
    *state.write().await = ConnectionState::Connecting;
    stats.lock().unwrap().record_attempt();
    
    match timeout(config.connection_timeout, connect_to_whatsapp()).await {
        Ok(Ok(socket)) => {
            *current_socket = Some(socket);
            *state.write().await = ConnectionState::Connected;
            stats.lock().unwrap().record_success();
            
            // Start keep-alive task
            *keepalive_handle = Some(start_keepalive_task(
                config.keepalive_interval,
                event_sender.clone(),
            ));
            
            let event = ConnectionEvent::Connected;
            broadcast_event(event_handlers, event_sender, event).await;
        }
        Ok(Err(error)) => {
            stats.lock().unwrap().record_failure();
            
            if is_recoverable_error(&error) {
                *state.write().await = ConnectionState::Reconnecting {
                    attempt: 0,
                    last_attempt: Instant::now(),
                };
            } else {
                *state.write().await = ConnectionState::Failed {
                    reason: error.to_string(),
                };
            }
            
            let event = ConnectionEvent::Disconnected {
                reason: error.to_string(),
            };
            broadcast_event(event_handlers, event_sender, event).await;
        }
        Err(_) => {
            // Timeout
            stats.lock().unwrap().record_failure();
            *state.write().await = ConnectionState::Reconnecting {
                attempt: 0,
                last_attempt: Instant::now(),
            };
            
            let event = ConnectionEvent::Timeout;
            broadcast_event(event_handlers, event_sender, event).await;
        }
    }
}

/// Attempt to reconnect
async fn attempt_reconnection(
    state: &Arc<RwLock<ConnectionState>>,
    config: &ConnectionConfig,
    stats: &Arc<Mutex<ConnectionStats>>,
    event_handlers: &Arc<RwLock<Vec<Box<dyn ConnectionEventHandler>>>>,
    event_sender: &broadcast::Sender<ConnectionEvent>,
    current_socket: &mut Option<NoiseSocket>,
    keepalive_handle: &mut Option<JoinHandle<()>>,
    attempt: u32,
) {
    let event = ConnectionEvent::ReconnectAttempt { attempt };
    broadcast_event(event_handlers, event_sender, event).await;
    
    stats.lock().unwrap().record_attempt();
    
    match timeout(config.connection_timeout, connect_to_whatsapp()).await {
        Ok(Ok(socket)) => {
            *current_socket = Some(socket);
            *state.write().await = ConnectionState::Connected;
            stats.lock().unwrap().record_success();
            
            // Start keep-alive task
            *keepalive_handle = Some(start_keepalive_task(
                config.keepalive_interval,
                event_sender.clone(),
            ));
            
            let event = ConnectionEvent::Reconnected;
            broadcast_event(event_handlers, event_sender, event).await;
        }
        Ok(Err(error)) => {
            stats.lock().unwrap().record_failure();
            
            *state.write().await = ConnectionState::Reconnecting {
                attempt,
                last_attempt: Instant::now(),
            };
            
            let event = ConnectionEvent::ReconnectFailed {
                attempt,
                reason: error.to_string(),
            };
            broadcast_event(event_handlers, event_sender, event).await;
        }
        Err(_) => {
            // Timeout
            stats.lock().unwrap().record_failure();
            *state.write().await = ConnectionState::Reconnecting {
                attempt,
                last_attempt: Instant::now(),
            };
            
            let event = ConnectionEvent::ReconnectFailed {
                attempt,
                reason: "Connection timeout".to_string(),
            };
            broadcast_event(event_handlers, event_sender, event).await;
        }
    }
}

/// Disconnect from WhatsApp
async fn disconnect(
    state: &Arc<RwLock<ConnectionState>>,
    stats: &Arc<Mutex<ConnectionStats>>,
    event_handlers: &Arc<RwLock<Vec<Box<dyn ConnectionEventHandler>>>>,
    event_sender: &broadcast::Sender<ConnectionEvent>,
    current_socket: &mut Option<NoiseSocket>,
    keepalive_handle: &mut Option<JoinHandle<()>>,
) {
    // Stop keep-alive task
    if let Some(handle) = keepalive_handle.take() {
        handle.abort();
    }
    
    // Close socket
    if let Some(socket) = current_socket.take() {
        // TODO: Gracefully close the socket
        drop(socket);
    }
    
    *state.write().await = ConnectionState::Disconnected;
    stats.lock().unwrap().record_disconnection();
    
    let event = ConnectionEvent::Disconnected {
        reason: "Manual disconnect".to_string(),
    };
    broadcast_event(event_handlers, event_sender, event).await;
}

/// Placeholder for actual WhatsApp connection logic
async fn connect_to_whatsapp() -> Result<NoiseSocket> {
    // TODO: Implement actual connection logic
    // This would involve:
    // 1. Creating WebSocket connection to WhatsApp servers
    // 2. Performing Noise protocol handshake
    // 3. Authenticating with stored credentials
    
    // For now, simulate connection delay and potential failure
    sleep(Duration::from_millis(500)).await;
    
    // Simulate occasional connection failures for testing
    use rand::Rng;
    if rand::thread_rng().gen_bool(0.1) {
        return Err(Error::Connection("Simulated connection failure".to_string()));
    }
    
    // Return a placeholder socket
    Ok(NoiseSocket::new().await?)
}

/// Start keep-alive task
fn start_keepalive_task(
    interval: Duration,
    event_sender: broadcast::Sender<ConnectionEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval_timer = tokio::time::interval(interval);
        
        loop {
            interval_timer.tick().await;
            
            // Send keep-alive ping
            let _ = event_sender.send(ConnectionEvent::KeepAlivePing);
            
            // TODO: Implement actual keep-alive logic
            // This would involve sending a ping message and waiting for pong
            
            // Simulate pong response
            tokio::time::sleep(Duration::from_millis(100)).await;
            let _ = event_sender.send(ConnectionEvent::KeepAlivePong);
        }
    })
}

/// Broadcast event to all handlers
async fn broadcast_event(
    event_handlers: &Arc<RwLock<Vec<Box<dyn ConnectionEventHandler>>>>,
    event_sender: &broadcast::Sender<ConnectionEvent>,
    event: ConnectionEvent,
) {
    // Send to broadcast channel
    let _ = event_sender.send(event.clone());
    
    // Send to registered handlers
    let handlers = event_handlers.read().await;
    for handler in handlers.iter() {
        handler.handle_event(event.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
    
    #[tokio::test]
    async fn test_connection_manager_lifecycle() {
        let config = ConnectionConfig::default();
        let mut manager = ConnectionManager::new(config);
        
        // Start manager
        manager.start().await.unwrap();
        assert!(manager.task_handle.is_some());
        
        // Stop manager
        manager.stop().await.unwrap();
        assert!(manager.task_handle.is_none());
    }
    
    #[tokio::test]
    async fn test_connection_stats_tracking() {
        let config = ConnectionConfig::default();
        let manager = ConnectionManager::new(config);
        
        let initial_stats = manager.get_stats();
        assert_eq!(initial_stats.total_attempts, 0);
        assert_eq!(initial_stats.successful_connections, 0);
        assert_eq!(initial_stats.failed_connections, 0);
    }
    
    #[tokio::test]
    async fn test_event_subscription() {
        let config = ConnectionConfig::default();
        let manager = ConnectionManager::new(config);
        
        let mut event_receiver = manager.subscribe_events();
        
        // This would normally trigger events, but since we haven't implemented
        // the actual connection logic, we just test the subscription mechanism
        tokio::select! {
            _ = event_receiver.recv() => {
                // Event received
            }
            _ = sleep(Duration::from_millis(100)) => {
                // Timeout - expected since no events are being generated
            }
        }
    }
}