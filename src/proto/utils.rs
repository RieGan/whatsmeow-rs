use crate::{
    binary::Node,
    error::{Error, Result},
    proto::{MessageKey, MessageText, MessageProto}
};
use prost::Message;

/// Utility functions for working with protobuf messages in WhatsApp protocol
pub struct ProtoUtils;

impl ProtoUtils {
    /// Convert a binary Node to a protobuf message
    pub fn node_to_proto<T: Message + Default>(node: &Node) -> Result<T> {
        match &node.content {
            crate::binary::NodeContent::Binary(data) => {
                T::decode(&data[..]).map_err(|e| Error::ProtobufDecode(e))
            },
            _ => Err(Error::Protocol("Node does not contain binary data".to_string()))
        }
    }
    
    /// Convert a protobuf message to binary data for a Node
    pub fn proto_to_bytes<T: Message>(message: &T) -> Vec<u8> {
        let mut buf = Vec::new();
        message.encode(&mut buf).unwrap();
        buf
    }
    
    /// Create a message node with protobuf content
    pub fn create_message_node<T: Message>(tag: &str, message: &T) -> Node {
        Node {
            tag: tag.to_string(),
            attrs: std::collections::HashMap::new(),
            content: crate::binary::NodeContent::Binary(Self::proto_to_bytes(message)),
        }
    }
    
    /// Extract message text from a protobuf message
    pub fn extract_text_message(data: &[u8]) -> Result<String> {
        let message_text = MessageText::decode(data)?;
        Ok(message_text.text.unwrap_or_default())
    }
    
    /// Create a text message protobuf
    pub fn create_text_message(text: &str) -> MessageText {
        MessageText {
            text: Some(text.to_string()),
            mentioned_jid: vec![],
        }
    }
    
    /// Create a message key protobuf
    pub fn create_message_key(jid: &str, id: &str, from_me: bool) -> MessageKey {
        MessageKey {
            remote_jid: Some(jid.to_string()),
            from_me: Some(from_me),
            id: Some(id.to_string()),
            participant: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_text_message() {
        let msg = ProtoUtils::create_text_message("Hello, World!");
        assert_eq!(msg.text, Some("Hello, World!".to_string()));
        assert!(msg.mentioned_jid.is_empty());
    }
    
    #[test]
    fn test_create_message_key() {
        let key = ProtoUtils::create_message_key("1234567890@s.whatsapp.net", "msg123", true);
        assert_eq!(key.remote_jid, Some("1234567890@s.whatsapp.net".to_string()));
        assert_eq!(key.id, Some("msg123".to_string()));
        assert_eq!(key.from_me, Some(true));
    }
}