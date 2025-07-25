use crate::{
    binary::{Node, NodeContent},
    error::{Error, Result},
    types::{JID, SendableMessage, TextMessage, MessageInfo, MessageType},
};
use std::collections::HashMap;
use std::time::SystemTime;

/// Message builder for creating WhatsApp messages
pub struct MessageBuilder {
    to: JID,
    message_type: MessageType,
    content: Option<SendableMessage>,
}

impl MessageBuilder {
    /// Create a new message builder
    pub fn new(to: JID) -> Self {
        Self {
            to,
            message_type: MessageType::Text,
            content: None,
        }
    }
    
    /// Set text message content
    pub fn text(mut self, text: String) -> Self {
        self.message_type = MessageType::Text;
        self.content = Some(SendableMessage::Text(TextMessage { text }));
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
            _ => Err(Error::Protocol("Unsupported message type".to_string())),
        }
    }
    
    /// Build a text message node
    fn build_text_message(&self, message_id: String, from_jid: JID, text: &str) -> Result<Node> {
        let mut attrs = HashMap::new();
        attrs.insert("id".to_string(), message_id);
        attrs.insert("type".to_string(), "text".to_string());
        attrs.insert("to".to_string(), self.to.to_string());
        attrs.insert("from".to_string(), from_jid.to_string());
        
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        attrs.insert("t".to_string(), timestamp.to_string());
        
        // Create the message content node
        let text_node = Node {
            tag: "body".to_string(),
            attrs: HashMap::new(),
            content: NodeContent::Text(text.to_string()),
        };
        
        Ok(Node {
            tag: "message".to_string(),
            attrs,
            content: NodeContent::Children(vec![text_node]),
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
            Some("document") => MessageType::Document,
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

/// Message queue for managing outgoing messages
pub struct MessageQueue {
    pending: Vec<PendingMessage>,
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