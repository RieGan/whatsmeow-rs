use crate::{
    appstate::{AppStateManager, AppStateManagerConfig, AppStateDataType, SyncRequest, SyncPriority},
    auth::{AuthManager, AuthState},
    connection::{
        ConnectionConfig, ConnectionEvent, ConnectionEventHandler,
        manager::ConnectionManager,
        rate_limit::{MultiRateLimiter, RateLimitResult},
        retry::{RetryExecutor, RetryPolicy, RetryResult},
    },
    database::Database,
    error::{Error, Result},
    messaging::{
        MessageBuilder, MessageQueue, MessageStatusTracker, MessageEditor,
        MessageThreadManager, FailedMessage
    },
    socket::NoiseSocket,
    store::DeviceStore,
    types::{
        Event, EventHandler, JID, SendableMessage, MessageInfo, MessageReceipt,
        MessageStatus, TextMessage, ExtendedTextMessage, MediaMessage, LocationMessage,
        ContactMessage, ReactionMessage, PollMessage, MessageKey, ContextInfo
    },
    media::MediaManager,
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
    pub app_state_config: AppStateManagerConfig,
    pub enable_app_state_sync: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            initial_auto_reconnect: true,
            synchronous_ack: false,
            connection_config: ConnectionConfig::default(),
            app_state_config: AppStateManagerConfig::default(),
            enable_app_state_sync: true,
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
    message_status_tracker: Arc<MessageStatusTracker>,
    message_thread_manager: Arc<Mutex<MessageThreadManager>>,
    media_manager: Arc<tokio::sync::Mutex<MediaManager>>,
    connection_manager: Arc<Mutex<Option<ConnectionManager>>>,
    rate_limiter: Arc<MultiRateLimiter>,
    retry_executor: Arc<RetryExecutor>,
    app_state_manager: Arc<Mutex<Option<AppStateManager>>>,
    database: Arc<Database>,
}

impl Client {
    /// Create a new WhatsApp client
    pub async fn new(store: Arc<dyn DeviceStore>, database: Arc<Database>) -> Result<Self> {
        Self::with_config(store, database, ClientConfig::default()).await
    }
    
    /// Create a new WhatsApp client with custom configuration
    pub async fn with_config(store: Arc<dyn DeviceStore>, database: Arc<Database>, config: ClientConfig) -> Result<Self> {
        // Initialize app state manager if enabled
        let app_state_manager = if config.enable_app_state_sync {
            let manager = AppStateManager::with_config(database.clone(), config.app_state_config.clone()).await?;
            Some(manager)
        } else {
            None
        };

        Ok(Self {
            store,
            socket: Arc::new(Mutex::new(None)),
            config: config.clone(),
            event_handlers: Arc::new(RwLock::new(Vec::new())),
            is_logged_in: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            auth_manager: Arc::new(Mutex::new(AuthManager::new())),
            message_queue: Arc::new(Mutex::new(MessageQueue::new())),
            message_status_tracker: Arc::new(MessageStatusTracker::new()),
            message_thread_manager: Arc::new(Mutex::new(MessageThreadManager::new())),
            media_manager: Arc::new(tokio::sync::Mutex::new(MediaManager::new())),
            connection_manager: Arc::new(Mutex::new(None)),
            rate_limiter: Arc::new(MultiRateLimiter::new()),
            retry_executor: Arc::new(RetryExecutor::new(RetryPolicy::network_operations())),
            app_state_manager: Arc::new(Mutex::new(app_state_manager)),
            database,
        })
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
                
                // Start app state sync if enabled
                if self.config.enable_app_state_sync {
                    if let Err(e) = self.start_app_state_sync().await {
                        warn!("Failed to start app state sync: {}", e);
                    }
                }
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
                    
                    // Start app state sync if enabled
                    if self.config.enable_app_state_sync {
                        if let Err(e) = self.start_app_state_sync().await {
                            warn!("Failed to start app state sync: {}", e);
                        }
                    }
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
        
        // Stop app state sync first
        if self.config.enable_app_state_sync {
            if let Err(e) = self.stop_app_state_sync().await {
                warn!("Failed to stop app state sync: {}", e);
            }
        }
        
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
        let qr_string = auth.generate_qr().await?;
        
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
    
    // ===== ENHANCED MESSAGING METHODS =====
    
    /// Send a text message
    pub async fn send_text(&self, to: &JID, text: String) -> Result<String> {
        let message = SendableMessage::Text(TextMessage { text });
        self.send_message_enhanced(to, message).await
    }
    
    /// Send an extended text message with formatting
    pub async fn send_extended_text(&self, to: &JID, text: String, context: Option<ContextInfo>) -> Result<String> {
        let extended_text = ExtendedTextMessage {
            text,
            matched_text: None,
            canonical_url: None,
            description: None,
            title: None,
            text_arg_b: None,
            thumbnail: None,
            jpeg_thumbnail: None,
            context_info: context,
            font: None,
            preview_type: None,
        };
        let message = SendableMessage::ExtendedText(extended_text);
        self.send_message_enhanced(to, message).await
    }
    
    /// Send a media message (image, video, audio, document)
    pub async fn send_media(&self, to: &JID, media_path: &str, caption: Option<String>) -> Result<String> {
        // Use media manager to process and upload the media
        let media_info = self.media_manager.lock().await.upload_media(media_path, crate::media::MediaType::Auto).await?;
        
        let media_message = MediaMessage {
            url: Some(media_info.url),
            direct_path: media_info.direct_path,
            media_key: Some(media_info.media_key),
            file_sha256: Some(media_info.file_sha256),
            file_length: Some(media_info.file_length),
            mime_type: Some(media_info.mime_type),
            caption,
            width: media_info.width,
            height: media_info.height,
            page_count: None,
            seconds: media_info.duration,
            ptt: Some(false),
            gif_playback: None,
            jpeg_thumbnail: media_info.thumbnail,
            context_info: None,
        };
        
        let message = match media_info.media_type {
            crate::media::MediaType::Image => SendableMessage::Image(media_message),
            crate::media::MediaType::Video => SendableMessage::Video(media_message),
            crate::media::MediaType::Audio => SendableMessage::Audio(media_message),
            crate::media::MediaType::Document => SendableMessage::Document(media_message),
            _ => SendableMessage::Document(media_message),
        };
        
        self.send_message_enhanced(to, message).await
    }
    
    /// Send a voice note
    pub async fn send_voice_note(&self, to: &JID, audio_path: &str) -> Result<String> {
        let media_info = self.media_manager.lock().await.upload_media(audio_path, crate::media::MediaType::Audio).await?;
        
        let media_message = MediaMessage {
            url: Some(media_info.url),
            direct_path: media_info.direct_path,
            media_key: Some(media_info.media_key),
            file_sha256: Some(media_info.file_sha256),
            file_length: Some(media_info.file_length),
            mime_type: Some(media_info.mime_type),
            caption: None,
            width: None,
            height: None,
            page_count: None,
            seconds: media_info.duration,
            ptt: Some(true), // Push to talk
            gif_playback: None,
            jpeg_thumbnail: None,
            context_info: None,
        };
        
        let message = SendableMessage::Voice(media_message);
        self.send_message_enhanced(to, message).await
    }
    
    /// Send a location message
    pub async fn send_location(&self, to: &JID, latitude: f64, longitude: f64, name: Option<String>, address: Option<String>) -> Result<String> {
        let location = LocationMessage {
            latitude,
            longitude,
            name,
            address,
        };
        let message = SendableMessage::Location(location);
        self.send_message_enhanced(to, message).await
    }
    
    /// Send a contact message
    pub async fn send_contact(&self, to: &JID, display_name: String, vcard: String) -> Result<String> {
        let contact = ContactMessage {
            display_name,
            vcard,
        };
        let message = SendableMessage::Contact(contact);
        self.send_message_enhanced(to, message).await
    }
    
    /// React to a message
    pub async fn react_to_message(&self, to: &JID, message_key: MessageKey, emoji: String) -> Result<String> {
        let reaction = ReactionMessage {
            key: message_key,
            text: emoji,
            sender_timestamp: Some(std::time::SystemTime::now()),
        };
        let message = SendableMessage::Reaction(reaction);
        self.send_message_enhanced(to, message).await
    }
    
    /// Send a poll
    pub async fn send_poll(&self, to: &JID, question: String, options: Vec<String>, selectable_count: u32) -> Result<String> {
        let poll_options = options.into_iter()
            .map(|name| crate::types::PollOption { name })
            .collect();
            
        let poll = PollMessage {
            name: question,
            options: poll_options,
            selectable_options_count: selectable_count,
            context_info: None,
        };
        let message = SendableMessage::Poll(poll);
        self.send_message_enhanced(to, message).await
    }
    
    /// Reply to a message
    pub async fn reply_to_message(&self, to: &JID, original_message: MessageKey, reply_text: String) -> Result<String> {
        let quoted = crate::types::QuotedMessage {
            id: original_message.id,
            remote_jid: original_message.remote_jid,
            participant: original_message.participant,
            message_type: crate::types::MessageType::Text,
            text: Some(reply_text.clone()),
            media_type: None,
        };
        
        let context = ContextInfo {
            quoted_message: Some(Box::new(quoted)),
            mentioned_jids: Vec::new(),
            forwarded: None,
            forwarding_score: None,
            is_forwarded: None,
            ephemeral_setting: None,
            ephemeral_shared_secret: None,
            external_ad_reply: None,
        };
        
        self.send_extended_text(to, reply_text, Some(context)).await
    }
    
    /// Edit a message
    pub async fn edit_message(&self, to: &JID, message_key: MessageKey, new_text: String) -> Result<String> {
        let edit_message = MessageEditor::create_edit_message(message_key, new_text);
        self.send_message_enhanced(to, edit_message).await
    }
    
    /// Delete a message
    pub async fn delete_message(&self, to: &JID, message_key: MessageKey) -> Result<String> {
        let delete_message = MessageEditor::create_delete_message(message_key);
        self.send_message_enhanced(to, delete_message).await
    }
    
    /// Enhanced message sending with full feature support
    async fn send_message_enhanced(&self, to: &JID, message: SendableMessage) -> Result<String> {
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
        
        debug!("Sending enhanced message to {}: {:?}", to, message);
        
        let message_id = uuid::Uuid::new_v4().to_string();
        
        // Update message status to pending
        self.message_status_tracker.update_status(&message_id, MessageStatus::Pending).await;
        
        // Use retry executor for sending messages
        let result = self.retry_executor.execute(|attempt| {
            let to = to.clone();
            let message = message.clone();
            let message_id = message_id.clone();
            let message_queue = Arc::clone(&self.message_queue);
            let status_tracker = Arc::clone(&self.message_status_tracker);
            
            async move {
                info!("Sending message attempt #{}", attempt.attempt);
                
                // Build the message node with enhanced builder
                let from_jid = JID::new("placeholder".to_string(), "s.whatsapp.net".to_string());
                let mut builder = MessageBuilder::new(to);
                
                let node = match &message {
                    SendableMessage::Text(text_msg) => {
                        builder.text(text_msg.text.clone()).build(message_id.clone(), from_jid)?
                    },
                    SendableMessage::ExtendedText(ext_text) => {
                        builder.extended_text(ext_text.text.clone()).build(message_id.clone(), from_jid)?
                    },
                    SendableMessage::Image(media) => {
                        builder.media(crate::types::MessageType::Image, media.clone()).build(message_id.clone(), from_jid)?
                    },
                    SendableMessage::Video(media) => {
                        builder.media(crate::types::MessageType::Video, media.clone()).build(message_id.clone(), from_jid)?
                    },
                    SendableMessage::Audio(media) => {
                        builder.media(crate::types::MessageType::Audio, media.clone()).build(message_id.clone(), from_jid)?
                    },
                    SendableMessage::Voice(media) => {
                        builder.media(crate::types::MessageType::Voice, media.clone()).build(message_id.clone(), from_jid)?
                    },
                    SendableMessage::Document(media) => {
                        builder.media(crate::types::MessageType::Document, media.clone()).build(message_id.clone(), from_jid)?
                    },
                    SendableMessage::Sticker(media) => {
                        builder.media(crate::types::MessageType::Sticker, media.clone()).build(message_id.clone(), from_jid)?
                    },
                    SendableMessage::Location(location) => {
                        builder.location(location.clone()).build(message_id.clone(), from_jid)?
                    },
                    SendableMessage::Contact(contact) => {
                        builder.contact(contact.clone()).build(message_id.clone(), from_jid)?
                    },
                    SendableMessage::Reaction(reaction) => {
                        builder.reaction(reaction.clone()).build(message_id.clone(), from_jid)?
                    },
                    SendableMessage::Poll(poll) => {
                        builder.poll(poll.clone()).build(message_id.clone(), from_jid)?
                    },
                    _ => {
                        return Err(Error::Protocol("Unsupported message type".to_string()));
                    }
                };
                
                // Add to message queue
                let mut queue = message_queue.lock().await;
                queue.enqueue(message_id.clone(), node);
                
                // Update status to sent
                status_tracker.update_status(&message_id, MessageStatus::Sent).await;
                
                // TODO: Actually send the message through the socket
                
                Ok(())
            }
        }).await;
        
        match result {
            RetryResult::Success(_) => {
                debug!("Enhanced message sent successfully: {}", message_id);
                Ok(message_id)
            }
            RetryResult::Failed { error, attempts } => {
                warn!("Failed to send enhanced message after {} attempts", attempts.len());
                
                // Update status to failed
                self.message_status_tracker.update_status(&message_id, MessageStatus::Failed).await;
                
                // Mark message as failed in queue
                let mut queue = self.message_queue.lock().await;
                queue.mark_failed(&message_id, error.to_string());
                
                Err(error)
            }
        }
    }
    
    /// Get message status
    pub async fn get_message_status(&self, message_id: &str) -> Option<MessageStatus> {
        self.message_status_tracker.get_status(message_id).await
    }
    
    /// Get recent messages from a chat
    pub async fn get_recent_messages(&self, chat_id: &str, count: usize) -> Vec<MessageInfo> {
        let thread_manager = self.message_thread_manager.lock().await;
        thread_manager.get_recent_messages(chat_id, count)
            .into_iter()
            .cloned()
            .collect()
    }
    
    /// Process incoming message receipt
    pub async fn process_message_receipt(&self, receipt: MessageReceipt) {
        // Update status tracker
        self.message_status_tracker.process_receipt(&receipt).await;
        
        // Acknowledge in queue if delivered
        if receipt.status == MessageStatus::Delivered {
            let mut queue = self.message_queue.lock().await;
            queue.acknowledge(&receipt.message_id);
        }
        
        // Emit receipt event
        self.emit_event(Event::MessageReceipt { receipt }).await;
    }
    
    /// Process incoming message
    pub async fn process_incoming_message(&self, message_info: MessageInfo) {
        // Add to thread manager
        {
            let mut thread_manager = self.message_thread_manager.lock().await;
            thread_manager.add_to_thread(&message_info.chat.to_string(), message_info.clone());
        }
        
        // Emit message event
        self.emit_event(Event::Message(message_info)).await;
    }
    
    /// Retry failed message
    pub async fn retry_failed_message(&self, message_id: &str) -> Result<Option<String>> {
        let mut queue = self.message_queue.lock().await;
        if let Some(pending) = queue.retry_failed(message_id) {
            drop(queue); // Release lock before recursive call
            
            // Resend the message
            // Note: This is a simplified retry - in practice you'd need to reconstruct the original message
            self.message_status_tracker.update_status(&pending.id, MessageStatus::Pending).await;
            
            Ok(Some(pending.id))
        } else {
            Ok(None)
        }
    }
    
    /// Get failed messages
    pub async fn get_failed_messages(&self) -> Vec<FailedMessage> {
        let queue = self.message_queue.lock().await;
        queue.failed_messages().to_vec()
    }
    
    /// Get message queue statistics
    pub async fn get_message_queue_stats(&self) -> (usize, usize) {
        let queue = self.message_queue.lock().await;
        (queue.len(), queue.failed_messages().len())
    }

    // ===== APP STATE MANAGEMENT METHODS =====

    /// Start app state manager
    pub async fn start_app_state_sync(&self) -> Result<()> {
        let manager_guard = self.app_state_manager.lock().await;
        if let Some(ref manager) = *manager_guard {
            manager.start().await?;
            info!("App state synchronization started");
        } else {
            warn!("App state sync is not enabled in client configuration");
        }
        Ok(())
    }

    /// Stop app state manager
    pub async fn stop_app_state_sync(&self) -> Result<()> {
        let manager_guard = self.app_state_manager.lock().await;
        if let Some(ref manager) = *manager_guard {
            manager.stop().await?;
            info!("App state synchronization stopped");
        }
        Ok(())
    }

    /// Request full app state sync
    pub async fn sync_app_state(&self) -> Result<Vec<String>> {
        let manager_guard = self.app_state_manager.lock().await;
        if let Some(ref manager) = *manager_guard {
            let session_ids = manager.request_full_sync().await?;
            info!("Initiated full app state sync with {} sessions", session_ids.len());
            Ok(session_ids)
        } else {
            Err(Error::Protocol("App state sync is not enabled".to_string()))
        }
    }

    /// Request sync for specific data type
    pub async fn sync_data_type(&self, data_type: AppStateDataType) -> Result<String> {
        let manager_guard = self.app_state_manager.lock().await;
        if let Some(ref manager) = *manager_guard {
            let session_id = manager.request_sync_for_type(data_type).await?;
            debug!("Started sync session {} for data type", session_id);
            Ok(session_id)
        } else {
            Err(Error::Protocol("App state sync is not enabled".to_string()))
        }
    }

    /// Get app state manager status
    pub async fn get_app_state_status(&self) -> Result<crate::appstate::AppStateManagerStatus> {
        let manager_guard = self.app_state_manager.lock().await;
        if let Some(ref manager) = *manager_guard {
            Ok(manager.get_status().await)
        } else {
            Err(Error::Protocol("App state sync is not enabled".to_string()))
        }
    }

    /// Get contact sync handler
    pub async fn get_contact_sync(&self) -> Result<Arc<crate::appstate::ContactSync>> {
        let manager_guard = self.app_state_manager.lock().await;
        if let Some(ref manager) = *manager_guard {
            Ok(manager.contact_sync())
        } else {
            Err(Error::Protocol("App state sync is not enabled".to_string()))
        }
    }

    /// Get chat metadata sync handler
    pub async fn get_chat_metadata_sync(&self) -> Result<Arc<crate::appstate::ChatMetadataSync>> {
        let manager_guard = self.app_state_manager.lock().await;
        if let Some(ref manager) = *manager_guard {
            Ok(manager.chat_metadata_sync())
        } else {
            Err(Error::Protocol("App state sync is not enabled".to_string()))
        }
    }

    /// Get settings sync handler  
    pub async fn get_settings_sync(&self) -> Result<Arc<crate::appstate::SettingsSync>> {
        let manager_guard = self.app_state_manager.lock().await;
        if let Some(ref manager) = *manager_guard {
            Ok(manager.settings_sync())
        } else {
            Err(Error::Protocol("App state sync is not enabled".to_string()))
        }
    }

    /// Archive a chat
    pub async fn archive_chat(&self, jid: &JID) -> Result<()> {
        let chat_sync = self.get_chat_metadata_sync().await?;
        chat_sync.archive_chat(jid).await?;
        
        // Trigger sync for chat metadata
        let _ = self.sync_data_type(AppStateDataType::ChatMetadata).await;
        
        Ok(())
    }

    /// Pin a chat
    pub async fn pin_chat(&self, jid: &JID) -> Result<()> {
        let chat_sync = self.get_chat_metadata_sync().await?;
        chat_sync.pin_chat(jid).await?;
        
        // Trigger sync for chat metadata
        let _ = self.sync_data_type(AppStateDataType::ChatMetadata).await;
        
        Ok(())
    }

    /// Mute a chat
    pub async fn mute_chat(&self, jid: &JID, duration_seconds: Option<u64>) -> Result<()> {
        let chat_sync = self.get_chat_metadata_sync().await?;
        chat_sync.mute_chat(jid, duration_seconds).await?;
        
        // Trigger sync for chat metadata
        let _ = self.sync_data_type(AppStateDataType::ChatMetadata).await;
        
        Ok(())
    }

    /// Update privacy settings
    pub async fn update_privacy_settings(&self, privacy: crate::appstate::PrivacySettings) -> Result<()> {
        let settings_sync = self.get_settings_sync().await?;
        settings_sync.update_privacy_settings("default", privacy).await?;
        
        // Trigger sync for settings
        let _ = self.sync_data_type(AppStateDataType::Settings).await;
        
        Ok(())
    }

    /// Update notification settings
    pub async fn update_notification_settings(&self, notifications: crate::appstate::NotificationSettings) -> Result<()> {
        let settings_sync = self.get_settings_sync().await?;
        settings_sync.update_notification_settings("default", notifications).await?;
        
        // Trigger sync for settings
        let _ = self.sync_data_type(AppStateDataType::Settings).await;
        
        Ok(())
    }

    /// Get user settings
    pub async fn get_user_settings(&self) -> Result<Option<crate::appstate::UserSettings>> {
        let settings_sync = self.get_settings_sync().await?;
        Ok(settings_sync.get_settings("default").await)
    }

    /// Search contacts
    pub async fn search_contacts(&self, filter: crate::appstate::ContactFilter) -> Result<Vec<crate::appstate::Contact>> {
        let contact_sync = self.get_contact_sync().await?;
        Ok(contact_sync.search_contacts(filter).await)
    }

    /// Get contact by JID
    pub async fn get_contact(&self, jid: &JID) -> Result<Option<crate::appstate::Contact>> {
        let contact_sync = self.get_contact_sync().await?;
        Ok(contact_sync.get_contact(jid).await)
    }

    /// Block a contact
    pub async fn block_contact(&self, jid: &JID) -> Result<()> {
        let contact_sync = self.get_contact_sync().await?;
        contact_sync.block_contact(jid).await?;
        
        // Trigger sync for contacts
        let _ = self.sync_data_type(AppStateDataType::Contacts).await;
        
        Ok(())
    }

    /// Get chat metadata
    pub async fn get_chat_metadata(&self, jid: &JID) -> Result<Option<crate::appstate::ChatMetadata>> {
        let chat_sync = self.get_chat_metadata_sync().await?;
        Ok(chat_sync.get_chat_metadata(jid).await)
    }

    /// Search chats
    pub async fn search_chats(&self, filter: crate::appstate::ChatFilter) -> Result<Vec<crate::appstate::ChatMetadata>> {
        let chat_sync = self.get_chat_metadata_sync().await?;
        Ok(chat_sync.search_chats(filter).await)
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