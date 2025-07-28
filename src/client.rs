use crate::{
    auth::{AuthManager, AuthState},
    connection::{
        ConnectionConfig, ConnectionEvent, ConnectionEventHandler,
        manager::ConnectionManager,
        rate_limit::{MultiRateLimiter, RateLimitResult},
        retry::{RetryExecutor, RetryPolicy, RetryResult},
    },
    error::{Error, Result},
    messaging::{MessageBuilder, MessageQueue},
    socket::NoiseSocket,
    store::DeviceStore,
    types::{Event, EventHandler, JID, SendableMessage},
};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

/// Configuration for the WhatsApp client
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub auto_reconnect: bool,
    pub initial_auto_reconnect: bool,
    pub synchronous_ack: bool,
    pub connection_config: ConnectionConfig,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            initial_auto_reconnect: true,
            synchronous_ack: false,
            connection_config: ConnectionConfig::default(),
        }
    }
}

/// Main WhatsApp client
pub struct Client {
    store: Arc<dyn DeviceStore>,
    socket: Arc<Mutex<Option<NoiseSocket>>>,
    config: ClientConfig,
    event_handlers: Arc<RwLock<Vec<EventHandler>>>,
    is_logged_in: Arc<std::sync::atomic::AtomicBool>,
    auth_manager: Arc<Mutex<AuthManager>>,
    message_queue: Arc<Mutex<MessageQueue>>,
    connection_manager: Arc<Mutex<Option<ConnectionManager>>>,
    rate_limiter: Arc<MultiRateLimiter>,
    retry_executor: Arc<RetryExecutor>,
}

impl Client {
    /// Create a new WhatsApp client
    pub fn new(store: Arc<dyn DeviceStore>) -> Self {
        Self::with_config(store, ClientConfig::default())
    }
    
    /// Create a new WhatsApp client with custom configuration
    pub fn with_config(store: Arc<dyn DeviceStore>, config: ClientConfig) -> Self {
        Self {
            store,
            socket: Arc::new(Mutex::new(None)),
            config: config.clone(),
            event_handlers: Arc::new(RwLock::new(Vec::new())),
            is_logged_in: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            auth_manager: Arc::new(Mutex::new(AuthManager::new())),
            message_queue: Arc::new(Mutex::new(MessageQueue::new())),
            connection_manager: Arc::new(Mutex::new(None)),
            rate_limiter: Arc::new(MultiRateLimiter::new()),
            retry_executor: Arc::new(RetryExecutor::new(RetryPolicy::network_operations())),
        }
    }
    
    /// Add an event handler
    pub async fn add_event_handler(&self, handler: EventHandler) {
        let mut handlers = self.event_handlers.write().await;
        handlers.push(handler);
    }
    
    /// Connect to WhatsApp
    pub async fn connect(&self) -> Result<()> {
        info!("Connecting to WhatsApp...");
        
        if self.config.auto_reconnect {
            // Use connection manager for automatic reconnection
            let mut manager_guard = self.connection_manager.lock().await;
            
            if manager_guard.is_none() {
                let mut connection_manager = ConnectionManager::new(self.config.connection_config.clone());
                
                // Add client event handler to bridge connection events to client events
                connection_manager.add_event_handler(Box::new(ClientConnectionEventHandler {
                    client_event_emitter: Arc::new({
                        let handlers = Arc::clone(&self.event_handlers);
                        move |event: Event| {
                            let handlers = Arc::clone(&handlers);
                            tokio::spawn(async move {
                                let handlers = handlers.read().await;
                                for handler in handlers.iter() {
                                    if !handler(event.clone()) {
                                        break;
                                    }
                                }
                            });
                        }
                    }),
                }));
                
                connection_manager.start().await?;
                *manager_guard = Some(connection_manager);
            }
            
            if let Some(ref manager) = *manager_guard {
                let _: Result<()> = manager.connect().await;
                
                // Wait for connection with timeout
                manager.wait_for_connection(self.config.connection_config.connection_timeout).await?;
                
                self.emit_event(Event::Connected).await;
                info!("Successfully connected to WhatsApp with connection manager");
            }
        } else {
            // Manual connection without reconnection management
            let result = self.retry_executor.execute(|attempt| {
                let socket_arc = Arc::clone(&self.socket);
                async move {
                    info!("Connection attempt #{}", attempt.attempt);
                    
                    // Create and connect socket
                    let mut socket = NoiseSocket::new().await?;
                    socket.connect().await?;
                    
                    // Perform Noise handshake
                    info!("Performing Noise protocol handshake...");
                    if let Err(e) = socket.perform_handshake().await {
                        debug!("Handshake attempt completed with result: {}", e);
                    }
                    
                    // Store the socket
                    let mut socket_guard = socket_arc.lock().await;
                    *socket_guard = Some(socket);
                    
                    Ok(())
                }
            }).await;
            
            match result {
                RetryResult::Success(_) => {
                    self.emit_event(Event::Connected).await;
                    info!("Successfully connected to WhatsApp WebSocket");
                }
                RetryResult::Failed { error, attempts } => {
                    warn!("Failed to connect after {} attempts", attempts.len());
                    return Err(error);
                }
            }
        }
        
        Ok(())
    }
    
    /// Disconnect from WhatsApp
    pub async fn disconnect(&self) -> Result<()> {
        info!("Disconnecting from WhatsApp...");
        
        // If using connection manager, disconnect through that
        let manager_guard = self.connection_manager.lock().await;
        if let Some(ref manager) = *manager_guard {
            let _: Result<()> = manager.disconnect().await;
        }
        
        let mut socket_guard = self.socket.lock().await;
        if let Some(socket) = socket_guard.take() {
            socket.close().await?;
        }
        
        self.is_logged_in.store(false, std::sync::atomic::Ordering::SeqCst);
        self.emit_event(Event::Disconnected { 
            reason: "Manual disconnect".to_string() 
        }).await;
        
        Ok(())
    }
    
    /// Check if the client is logged in
    pub fn is_logged_in(&self) -> bool {
        self.is_logged_in.load(std::sync::atomic::Ordering::SeqCst)
    }
    
    /// Generate QR code for authentication
    pub async fn generate_qr(&self) -> Result<String> {
        let mut auth = self.auth_manager.lock().await;
        let qr_string = auth.generate_qr()?;
        
        self.emit_event(Event::QRCode { code: qr_string.clone() }).await;
        
        Ok(qr_string)
    }
    
    /// Get current authentication state
    pub async fn auth_state(&self) -> AuthState {
        let auth = self.auth_manager.lock().await;
        auth.state().clone()
    }
    
    /// Send a message
    pub async fn send_message(&self, to: &JID, message: SendableMessage) -> Result<String> {
        if !self.is_logged_in() {
            return Err(Error::NotLoggedIn);
        }
        
        // Apply rate limiting for message sending
        match self.rate_limiter.wait_for_rate_limit("messages").await {
            RateLimitResult::Allowed => {
                debug!("Message sending allowed by rate limiter");
            }
            RateLimitResult::Limited { retry_after } => {
                warn!("Message sending rate limited, waited {:?}", retry_after);
            }
        }
        
        debug!("Sending message to {}: {:?}", to, message);
        
        let message_id = uuid::Uuid::new_v4().to_string();
        
        // Use retry executor for sending messages
        let result = self.retry_executor.execute(|attempt| {
            let to = to.clone();
            let message = message.clone();
            let message_id = message_id.clone();
            let message_queue = Arc::clone(&self.message_queue);
            
            async move {
                info!("Sending message attempt #{}", attempt.attempt);
                
                // Build the message node
                let from_jid = JID::new("placeholder".to_string(), "s.whatsapp.net".to_string());
                let builder = MessageBuilder::new(to);
                
                let node = match message {
                    SendableMessage::Text(text_msg) => {
                        builder.text(text_msg.text).build(message_id.clone(), from_jid)?
                    },
                    _ => {
                        return Err(Error::Protocol("Unsupported message type".to_string()));
                    }
                };
                
                // Add to message queue
                let mut queue = message_queue.lock().await;
                queue.enqueue(message_id.clone(), node);
                
                // TODO: Actually send the message through the socket
                
                Ok(())
            }
        }).await;
        
        match result {
            RetryResult::Success(_) => {
                debug!("Message sent successfully: {}", message_id);
                Ok(message_id)
            }
            RetryResult::Failed { error, attempts } => {
                warn!("Failed to send message after {} attempts", attempts.len());
                Err(error)
            }
        }
    }
    
    /// Start listening for events
    pub async fn start_listening(&self) -> Result<()> {
        info!("Starting event listener...");
        
        // TODO: Implement actual event listening loop
        // This should read from the socket and emit events
        
        Ok(())
    }
    
    /// Emit an event to all handlers
    async fn emit_event(&self, event: Event) {
        let handlers = self.event_handlers.read().await;
        for handler in handlers.iter() {
            if !handler(event.clone()) {
                break;
            }
        }
    }
    
    /// Get connection statistics
    pub async fn get_connection_stats(&self) -> Option<crate::connection::ConnectionStats> {
        let manager_guard = self.connection_manager.lock().await;
        manager_guard.as_ref().map(|manager| manager.get_stats())
    }
    
    /// Get rate limit status
    pub async fn get_rate_limit_status(&self) -> std::collections::HashMap<String, crate::connection::rate_limit::RateLimitStatus> {
        self.rate_limiter.get_all_status().await
    }
    
    /// Force reconnection
    pub async fn reconnect(&self) -> Result<()> {
        let manager_guard = self.connection_manager.lock().await;
        if let Some(ref manager) = *manager_guard {
            let result: Result<()> = manager.reconnect().await;
            result
        } else {
            Err(Error::Connection("Connection manager not initialized".to_string()))
        }
    }
}

/// Event handler that bridges connection events to client events
struct ClientConnectionEventHandler {
    client_event_emitter: Arc<dyn Fn(Event) + Send + Sync>,
}

impl ConnectionEventHandler for ClientConnectionEventHandler {
    fn handle_event(&self, event: ConnectionEvent) {
        let client_event = match event {
            ConnectionEvent::Connected => Event::Connected,
            ConnectionEvent::Disconnected { reason } => Event::Disconnected { reason },
            ConnectionEvent::Reconnected => Event::Connected,
            ConnectionEvent::ReconnectAttempt { attempt } => {
                info!("Connection reconnect attempt #{}", attempt);
                return; // Don't emit client event for this
            }
            ConnectionEvent::ReconnectFailed { attempt, reason } => {
                warn!("Connection reconnect attempt #{} failed: {}", attempt, reason);
                return; // Don't emit client event for this
            }
            ConnectionEvent::ReconnectExhausted => {
                Event::Disconnected { reason: "Max reconnection attempts reached".to_string() }
            }
            ConnectionEvent::Timeout => {
                Event::Disconnected { reason: "Connection timeout".to_string() }
            }
            _ => return, // Don't emit client events for other connection events
        };
        
        (self.client_event_emitter)(client_event);
    }
}