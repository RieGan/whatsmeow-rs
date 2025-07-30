use crate::{
    binary::{Node, NodeContent},
    error::{Error, Result},
    types::{
        JID, SendableMessage, TextMessage, ExtendedTextMessage, MessageInfo, MessageType,
        MediaMessage, LocationMessage, ContactMessage, ReactionMessage, PollMessage,
        QuotedMessage, GroupInviteMessage, ProtocolMessage, MessageReceipt, MessageStatus,
        ContextInfo, MessageKey, ProtocolMessageType
    },
    proto::ProtoUtils,
    media::MediaManager,
};
use std::collections::HashMap;
use std::time::SystemTime;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use base64;

/// Message builder for creating WhatsApp messages
pub struct MessageBuilder {
    to: JID,
    message_type: MessageType,
    content: Option<SendableMessage>,
    context_info: Option<ContextInfo>,
    quoted_message: Option<QuotedMessage>,
    ephemeral_expiration: Option<u32>,
}

impl MessageBuilder {
    /// Create a new message builder
    pub fn new(to: JID) -> Self {
        Self {
            to,
            message_type: MessageType::Text,
            content: None,
            context_info: None,
            quoted_message: None,
            ephemeral_expiration: None,
        }
    }
    
    /// Set context information (for replies, mentions, etc.)
    pub fn with_context(mut self, context_info: ContextInfo) -> Self {
        self.context_info = Some(context_info);
        self
    }
    
    /// Set quoted message (for replies)
    pub fn reply_to(mut self, quoted_message: QuotedMessage) -> Self {
        self.quoted_message = Some(quoted_message);
        self
    }
    
    /// Set ephemeral expiration (for disappearing messages)
    pub fn ephemeral(mut self, expiration_seconds: u32) -> Self {
        self.ephemeral_expiration = Some(expiration_seconds);
        self
    }
    
    /// Set text message content
    pub fn text(mut self, text: String) -> Self {
        self.message_type = MessageType::Text;
        self.content = Some(SendableMessage::Text(TextMessage { text }));
        self
    }
    
    /// Set extended text message with formatting
    pub fn extended_text(mut self, text: String) -> Self {
        self.message_type = MessageType::Text;
        let extended_text = ExtendedTextMessage {
            text,
            matched_text: None,
            canonical_url: None,
            description: None,
            title: None,
            text_arg_b: None,
            thumbnail: None,
            jpeg_thumbnail: None,
            context_info: self.context_info.clone(),
            font: None,
            preview_type: None,
        };
        self.content = Some(SendableMessage::ExtendedText(extended_text));
        self
    }
    
    /// Set media message (image, video, audio, document)
    pub fn media(mut self, media_type: MessageType, media: MediaMessage) -> Self {
        self.message_type = media_type.clone();
        match media_type {
            MessageType::Image => self.content = Some(SendableMessage::Image(media)),
            MessageType::Video => self.content = Some(SendableMessage::Video(media)),
            MessageType::Audio => self.content = Some(SendableMessage::Audio(media)),
            MessageType::Voice => self.content = Some(SendableMessage::Voice(media)),
            MessageType::Document => self.content = Some(SendableMessage::Document(media)),
            MessageType::Sticker => self.content = Some(SendableMessage::Sticker(media)),
            _ => {} // Invalid media type
        }
        self
    }
    
    /// Set location message
    pub fn location(mut self, location: LocationMessage) -> Self {
        self.message_type = MessageType::Location;
        self.content = Some(SendableMessage::Location(location));
        self
    }
    
    /// Set contact message
    pub fn contact(mut self, contact: ContactMessage) -> Self {
        self.message_type = MessageType::Contact;
        self.content = Some(SendableMessage::Contact(contact));
        self
    }
    
    /// Set reaction message
    pub fn reaction(mut self, reaction: ReactionMessage) -> Self {
        self.message_type = MessageType::Reaction;
        self.content = Some(SendableMessage::Reaction(reaction));
        self
    }
    
    /// Set poll message
    pub fn poll(mut self, poll: PollMessage) -> Self {
        self.message_type = MessageType::Poll;
        self.content = Some(SendableMessage::Poll(poll));
        self
    }
    
    /// Build the message into a WhatsApp node
    pub fn build(&self, message_id: String, from_jid: JID) -> Result<Node> {
        let content = self.content.as_ref()
            .ok_or_else(|| Error::Protocol("No message content set".to_string()))?;
            
        match content {
            SendableMessage::Text(text_msg) => {
                self.build_text_message(message_id, from_jid, &text_msg.text)
            },
            SendableMessage::ExtendedText(ext_text) => {
                self.build_extended_text_message(message_id, from_jid, ext_text)
            },
            SendableMessage::Image(media) => {
                self.build_media_message(message_id, from_jid, "image", media)
            },
            SendableMessage::Video(media) => {
                self.build_media_message(message_id, from_jid, "video", media)
            },
            SendableMessage::Audio(media) => {
                self.build_media_message(message_id, from_jid, "audio", media)
            },
            SendableMessage::Voice(media) => {
                self.build_media_message(message_id, from_jid, "ptt", media)
            },
            SendableMessage::Document(media) => {
                self.build_media_message(message_id, from_jid, "document", media)
            },
            SendableMessage::Sticker(media) => {
                self.build_media_message(message_id, from_jid, "sticker", media)
            },
            SendableMessage::Location(location) => {
                self.build_location_message(message_id, from_jid, location)
            },
            SendableMessage::Contact(contact) => {
                self.build_contact_message(message_id, from_jid, contact)
            },
            SendableMessage::Reaction(reaction) => {
                self.build_reaction_message(message_id, from_jid, reaction)
            },
            SendableMessage::Poll(poll) => {
                self.build_poll_message(message_id, from_jid, poll)
            },
            _ => Err(Error::Protocol("Unsupported message type".to_string())),
        }
    }
    
    /// Build common message attributes
    fn build_message_attrs(&self, message_id: String, from_jid: JID, msg_type: &str) -> HashMap<String, String> {
        let mut attrs = HashMap::new();
        attrs.insert("id".to_string(), message_id);
        attrs.insert("type".to_string(), msg_type.to_string());
        attrs.insert("to".to_string(), self.to.to_string());
        attrs.insert("from".to_string(), from_jid.to_string());
        
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        attrs.insert("t".to_string(), timestamp.to_string());
        
        if let Some(expiration) = self.ephemeral_expiration {
            attrs.insert("ephemeral".to_string(), expiration.to_string());
        }
        
        attrs
    }
    
    /// Build a text message node using protobuf structures
    fn build_text_message(&self, message_id: String, from_jid: JID, text: &str) -> Result<Node> {
        let attrs = self.build_message_attrs(message_id.clone(), from_jid, "text");
        
        // Create protobuf message key
        let _message_key = ProtoUtils::create_message_key(
            &self.to.to_string(),
            &message_id,
            true // from_me = true since we're sending
        );
        
        // Create protobuf text message
        let text_message = ProtoUtils::create_text_message(text);
        
        // Create the message content with protobuf binary data
        let proto_data = ProtoUtils::text_to_bytes(&text_message);
        let mut children = vec![Node {
            tag: "body".to_string(),
            attrs: HashMap::new(),
            content: NodeContent::Binary(proto_data),
        }];
        
        // Add context info if present
        if let Some(context) = &self.context_info {
            if let Some(quoted) = &context.quoted_message {
                children.push(self.build_quoted_node(quoted)?);
            }
        }
        
        Ok(Node {
            tag: "message".to_string(),
            attrs,
            content: NodeContent::Children(children),
        })
    }
    
    /// Build an extended text message node
    fn build_extended_text_message(&self, message_id: String, from_jid: JID, ext_text: &ExtendedTextMessage) -> Result<Node> {
        let attrs = self.build_message_attrs(message_id, from_jid, "text");
        
        let mut ext_attrs = HashMap::new();
        if let Some(url) = &ext_text.canonical_url {
            ext_attrs.insert("url".to_string(), url.clone());
        }
        if let Some(title) = &ext_text.title {
            ext_attrs.insert("title".to_string(), title.clone());
        }
        if let Some(desc) = &ext_text.description {
            ext_attrs.insert("description".to_string(), desc.clone());
        }
        
        let ext_text_node = Node {
            tag: "extendedTextMessage".to_string(),
            attrs: ext_attrs,
            content: NodeContent::Text(ext_text.text.clone()),
        };
        
        Ok(Node {
            tag: "message".to_string(),
            attrs,
            content: NodeContent::Children(vec![ext_text_node]),
        })
    }
    
    /// Build a media message node
    fn build_media_message(&self, message_id: String, from_jid: JID, media_type: &str, media: &MediaMessage) -> Result<Node> {
        let attrs = self.build_message_attrs(message_id, from_jid, media_type);
        
        let mut media_attrs = HashMap::new();
        if let Some(url) = &media.url {
            media_attrs.insert("url".to_string(), url.clone());
        }
        if let Some(mime) = &media.mime_type {
            media_attrs.insert("mimetype".to_string(), mime.clone());
        }
        if let Some(length) = media.file_length {
            media_attrs.insert("fileSha256".to_string(), base64::encode(media.file_sha256.as_ref().unwrap_or(&vec![])));
            media_attrs.insert("fileLength".to_string(), length.to_string());
        }
        if let Some(width) = media.width {
            media_attrs.insert("width".to_string(), width.to_string());
        }
        if let Some(height) = media.height {
            media_attrs.insert("height".to_string(), height.to_string());
        }
        if let Some(seconds) = media.seconds {
            media_attrs.insert("seconds".to_string(), seconds.to_string());
        }
        if media.ptt.unwrap_or(false) {
            media_attrs.insert("ptt".to_string(), "true".to_string());
        }
        
        let mut children = vec![];
        
        let media_node = Node {
            tag: format!("{}Message", media_type),
            attrs: media_attrs,
            content: if let Some(caption) = &media.caption {
                NodeContent::Text(caption.clone())
            } else {
                NodeContent::Text(String::new())
            },
        };
        children.push(media_node);
        
        Ok(Node {
            tag: "message".to_string(),
            attrs,
            content: NodeContent::Children(children),
        })
    }
    
    /// Build a location message node
    fn build_location_message(&self, message_id: String, from_jid: JID, location: &LocationMessage) -> Result<Node> {
        let attrs = self.build_message_attrs(message_id, from_jid, "location");
        
        let mut loc_attrs = HashMap::new();
        loc_attrs.insert("degreesLatitude".to_string(), location.latitude.to_string());
        loc_attrs.insert("degreesLongitude".to_string(), location.longitude.to_string());
        
        if let Some(name) = &location.name {
            loc_attrs.insert("name".to_string(), name.clone());
        }
        if let Some(address) = &location.address {
            loc_attrs.insert("address".to_string(), address.clone());
        }
        
        let location_node = Node {
            tag: "locationMessage".to_string(),
            attrs: loc_attrs,
            content: NodeContent::Text(String::new()),
        };
        
        Ok(Node {
            tag: "message".to_string(),
            attrs,
            content: NodeContent::Children(vec![location_node]),
        })
    }
    
    /// Build a contact message node
    fn build_contact_message(&self, message_id: String, from_jid: JID, contact: &ContactMessage) -> Result<Node> {
        let attrs = self.build_message_attrs(message_id, from_jid, "contact");
        
        let mut contact_attrs = HashMap::new();
        contact_attrs.insert("displayName".to_string(), contact.display_name.clone());
        
        let contact_node = Node {
            tag: "contactMessage".to_string(),
            attrs: contact_attrs,
            content: NodeContent::Text(contact.vcard.clone()),
        };
        
        Ok(Node {
            tag: "message".to_string(),
            attrs,
            content: NodeContent::Children(vec![contact_node]),
        })
    }
    
    /// Build a reaction message node
    fn build_reaction_message(&self, message_id: String, from_jid: JID, reaction: &ReactionMessage) -> Result<Node> {
        let attrs = self.build_message_attrs(message_id, from_jid, "reaction");
        
        let mut reaction_attrs = HashMap::new();
        reaction_attrs.insert("text".to_string(), reaction.text.clone());
        reaction_attrs.insert("key".to_string(), format!("{}-{}", reaction.key.remote_jid, reaction.key.id));
        
        let reaction_node = Node {
            tag: "reactionMessage".to_string(),
            attrs: reaction_attrs,
            content: NodeContent::Text(String::new()),
        };
        
        Ok(Node {
            tag: "message".to_string(),
            attrs,
            content: NodeContent::Children(vec![reaction_node]),
        })
    }
    
    /// Build a poll message node
    fn build_poll_message(&self, message_id: String, from_jid: JID, poll: &PollMessage) -> Result<Node> {
        let attrs = self.build_message_attrs(message_id, from_jid, "poll");
        
        let mut poll_attrs = HashMap::new();
        poll_attrs.insert("name".to_string(), poll.name.clone());
        poll_attrs.insert("selectableCount".to_string(), poll.selectable_options_count.to_string());
        
        let mut option_nodes = vec![];
        for option in &poll.options {
            let option_node = Node {
                tag: "option".to_string(),
                attrs: HashMap::new(),
                content: NodeContent::Text(option.name.clone()),
            };
            option_nodes.push(option_node);
        }
        
        let poll_node = Node {
            tag: "pollCreationMessage".to_string(),
            attrs: poll_attrs,
            content: NodeContent::Children(option_nodes),
        };
        
        Ok(Node {
            tag: "message".to_string(),
            attrs,
            content: NodeContent::Children(vec![poll_node]),
        })
    }
    
    /// Build a quoted message node
    fn build_quoted_node(&self, quoted: &QuotedMessage) -> Result<Node> {
        let mut quoted_attrs = HashMap::new();
        quoted_attrs.insert("id".to_string(), quoted.id.clone());
        quoted_attrs.insert("remoteJid".to_string(), quoted.remote_jid.to_string());
        
        if let Some(participant) = &quoted.participant {
            quoted_attrs.insert("participant".to_string(), participant.to_string());
        }
        
        Ok(Node {
            tag: "quotedMessage".to_string(),
            attrs: quoted_attrs,
            content: if let Some(text) = &quoted.text {
                NodeContent::Text(text.clone())
            } else {
                NodeContent::Text(String::new())
            },
        })
    }
}

/// Message processor for handling incoming messages
pub struct MessageProcessor;

impl MessageProcessor {
    /// Process an incoming message node
    pub fn process_message(node: &Node) -> Result<MessageInfo> {
        let id = node.get_attr("id")
            .ok_or_else(|| Error::Protocol("Message missing ID".to_string()))?
            .clone();
            
        let from_str = node.get_attr("from")
            .ok_or_else(|| Error::Protocol("Message missing from".to_string()))?;
        let sender: JID = from_str.parse()?;
        
        let to_str = node.get_attr("to")
            .ok_or_else(|| Error::Protocol("Message missing to".to_string()))?;
        let chat: JID = to_str.parse()?;
        
        let timestamp_str = node.get_attr("t")
            .ok_or_else(|| Error::Protocol("Message missing timestamp".to_string()))?;
        let timestamp = SystemTime::UNIX_EPOCH + 
            std::time::Duration::from_secs(timestamp_str.parse().unwrap_or(0));
            
        let message_type = match node.get_attr("type").map(|s| s.as_str()) {
            Some("text") => MessageType::Text,
            Some("image") => MessageType::Image,
            Some("video") => MessageType::Video,
            Some("audio") => MessageType::Audio,
            Some("ptt") => MessageType::Voice,
            Some("document") => MessageType::Document,
            Some("sticker") => MessageType::Sticker,
            Some("location") => MessageType::Location,
            Some("liveLocation") => MessageType::LiveLocation,
            Some("contact") => MessageType::Contact,
            Some("contactsArray") => MessageType::ContactsArray,
            Some("reaction") => MessageType::Reaction,
            Some("poll") => MessageType::Poll,
            Some("pollUpdate") => MessageType::PollUpdate,
            Some("groupInvite") => MessageType::GroupInvite,
            Some("call") => MessageType::Call,
            Some("protocol") => MessageType::ProtocolMessage,
            _ => MessageType::Unknown,
        };
        
        // TODO: Determine if message is from us
        let from_me = false;
        
        Ok(MessageInfo {
            id,
            chat,
            sender,
            timestamp,
            message_type,
            from_me,
        })
    }
}

/// Failed message information
#[derive(Debug, Clone)]
pub struct FailedMessage {
    pub message: PendingMessage,
    pub error: String,
    pub failed_at: SystemTime,
}

/// Enhanced message queue with receipt tracking
pub struct MessageQueue {
    pending: Vec<PendingMessage>,
    receipts: HashMap<String, MessageReceipt>,
    failed_messages: Vec<FailedMessage>,
}

#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub id: String,
    pub node: Node,
    pub retry_count: u8,
    pub created_at: SystemTime,
}

impl MessageQueue {
    /// Create a new message queue
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            receipts: HashMap::new(),
            failed_messages: Vec::new(),
        }
    }
    
    /// Add a receipt for a message
    pub fn add_receipt(&mut self, receipt: MessageReceipt) {
        self.receipts.insert(receipt.message_id.clone(), receipt);
    }
    
    /// Get receipt for a message
    pub fn get_receipt(&self, message_id: &str) -> Option<&MessageReceipt> {
        self.receipts.get(message_id)
    }
    
    /// Mark a message as failed
    pub fn mark_failed(&mut self, message_id: &str, error: String) {
        if let Some(pos) = self.pending.iter().position(|msg| msg.id == message_id) {
            let failed_message = FailedMessage {
                message: self.pending.remove(pos),
                error,
                failed_at: SystemTime::now(),
            };
            self.failed_messages.push(failed_message);
        }
    }
    
    /// Get failed messages
    pub fn failed_messages(&self) -> &[FailedMessage] {
        &self.failed_messages
    }
    
    /// Retry a failed message
    pub fn retry_failed(&mut self, message_id: &str) -> Option<PendingMessage> {
        if let Some(pos) = self.failed_messages.iter().position(|msg| msg.message.id == message_id) {
            let mut failed = self.failed_messages.remove(pos);
            failed.message.retry_count += 1;
            failed.message.created_at = SystemTime::now();
            let pending = failed.message.clone();
            self.pending.push(failed.message);
            Some(pending)
        } else {
            None
        }
    }
    
    /// Add a message to the queue
    pub fn enqueue(&mut self, id: String, node: Node) {
        let pending = PendingMessage {
            id,
            node,
            retry_count: 0,
            created_at: SystemTime::now(),
        };
        self.pending.push(pending);
    }
    
    /// Get the next message to send
    pub fn next(&mut self) -> Option<PendingMessage> {
        self.pending.pop()
    }
    
    /// Mark a message as acknowledged
    pub fn acknowledge(&mut self, message_id: &str) {
        self.pending.retain(|msg| msg.id != message_id);
    }
    
    /// Get pending message count
    pub fn len(&self) -> usize {
        self.pending.len()
    }
    
    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Message status tracker for handling receipts and delivery status
pub struct MessageStatusTracker {
    message_status: Arc<RwLock<HashMap<String, MessageStatus>>>,
    status_callbacks: Vec<Box<dyn Fn(&str, MessageStatus) + Send + Sync>>,
}

impl MessageStatusTracker {
    /// Create a new message status tracker
    pub fn new() -> Self {
        Self {
            message_status: Arc::new(RwLock::new(HashMap::new())),
            status_callbacks: Vec::new(),
        }
    }
    
    /// Update message status
    pub async fn update_status(&self, message_id: &str, status: MessageStatus) {
        {
            let mut statuses = self.message_status.write().await;
            statuses.insert(message_id.to_string(), status.clone());
        }
        
        // Notify callbacks
        for callback in &self.status_callbacks {
            callback(message_id, status.clone());
        }
    }
    
    /// Get message status
    pub async fn get_status(&self, message_id: &str) -> Option<MessageStatus> {
        let statuses = self.message_status.read().await;
        statuses.get(message_id).cloned()
    }
    
    /// Add status callback
    pub fn add_status_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str, MessageStatus) + Send + Sync + 'static,
    {
        self.status_callbacks.push(Box::new(callback));
    }
    
    /// Process receipt message
    pub async fn process_receipt(&self, receipt: &MessageReceipt) {
        self.update_status(&receipt.message_id, receipt.status.clone()).await;
    }
}

impl Default for MessageStatusTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Message editor for handling message edits and deletions
pub struct MessageEditor;

impl MessageEditor {
    /// Create an edit message
    pub fn create_edit_message(original_key: MessageKey, _new_text: String) -> SendableMessage {
        let protocol_msg = ProtocolMessage {
            key: Some(original_key),
            message_type: ProtocolMessageType::MessageEdit,
            ephemeral_expiration: None,
            ephemeral_setting_timestamp: None,
            history_sync_notification: None,
            app_state_sync_key_share: None,
            initial_security_notification_setting_sync: None,
            app_state_sync_key_request: None,
        };
        
        SendableMessage::Protocol(protocol_msg)
    }
    
    /// Create a delete message
    pub fn create_delete_message(message_key: MessageKey) -> SendableMessage {
        let protocol_msg = ProtocolMessage {
            key: Some(message_key),
            message_type: ProtocolMessageType::Revoke,
            ephemeral_expiration: None,
            ephemeral_setting_timestamp: None,
            history_sync_notification: None,
            app_state_sync_key_share: None,
            initial_security_notification_setting_sync: None,
            app_state_sync_key_request: None,
        };
        
        SendableMessage::Protocol(protocol_msg)
    }
}

/// Message thread manager for handling conversation threading
pub struct MessageThreadManager {
    threads: HashMap<String, Vec<MessageInfo>>,
}

impl MessageThreadManager {
    /// Create a new thread manager
    pub fn new() -> Self {
        Self {
            threads: HashMap::new(),
        }
    }
    
    /// Add message to thread
    pub fn add_to_thread(&mut self, chat_id: &str, message: MessageInfo) {
        self.threads
            .entry(chat_id.to_string())
            .or_insert_with(Vec::new)
            .push(message);
    }
    
    /// Get thread messages
    pub fn get_thread(&self, chat_id: &str) -> Option<&Vec<MessageInfo>> {
        self.threads.get(chat_id)
    }
    
    /// Get recent messages from thread
    pub fn get_recent_messages(&self, chat_id: &str, count: usize) -> Vec<&MessageInfo> {
        if let Some(messages) = self.threads.get(chat_id) {
            messages.iter().rev().take(count).collect()
        } else {
            Vec::new()
        }
    }
}

impl Default for MessageThreadManager {
    fn default() -> Self {
        Self::new()
    }
}