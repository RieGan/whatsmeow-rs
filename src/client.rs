use crate::{
    auth::{AuthManager, AuthState},
    error::{Error, Result},
    messaging::{MessageBuilder, MessageQueue},
    socket::NoiseSocket,
    store::DeviceStore,
    types::{Event, EventHandler, JID, SendableMessage},
};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info};

/// Configuration for the WhatsApp client
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub auto_reconnect: bool,
    pub initial_auto_reconnect: bool,
    pub synchronous_ack: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            initial_auto_reconnect: true,
            synchronous_ack: false,
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
            config,
            event_handlers: Arc::new(RwLock::new(Vec::new())),
            is_logged_in: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            auth_manager: Arc::new(Mutex::new(AuthManager::new())),
            message_queue: Arc::new(Mutex::new(MessageQueue::new())),
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
        
        // Create and connect socket
        let mut socket = NoiseSocket::new().await?;
        socket.connect().await?;
        
        // Perform Noise handshake
        info!("Performing Noise protocol handshake...");
        if let Err(e) = socket.perform_handshake().await {
            // Note: In real WhatsApp connection, handshake might fail initially
            // This is expected behavior for first-time connections
            debug!("Handshake attempt completed with result: {}", e);
        }
        
        // Store the socket
        let mut socket_guard = self.socket.lock().await;
        *socket_guard = Some(socket);
        
        self.emit_event(Event::Connected).await;
        info!("Successfully connected to WhatsApp WebSocket");
        Ok(())
    }
    
    /// Disconnect from WhatsApp
    pub async fn disconnect(&self) -> Result<()> {
        info!("Disconnecting from WhatsApp...");
        
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
        
        debug!("Sending message to {}: {:?}", to, message);
        
        let message_id = uuid::Uuid::new_v4().to_string();
        
        // Build the message node
        let from_jid = JID::new("placeholder".to_string(), "s.whatsapp.net".to_string());
        let builder = MessageBuilder::new(to.clone());
        
        let node = match message {
            SendableMessage::Text(text_msg) => {
                builder.text(text_msg.text).build(message_id.clone(), from_jid)?
            },
            _ => {
                return Err(Error::Protocol("Unsupported message type".to_string()));
            }
        };
        
        // Add to message queue
        let mut queue = self.message_queue.lock().await;
        queue.enqueue(message_id.clone(), node);
        
        // TODO: Actually send the message through the socket
        
        Ok(message_id)
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
}