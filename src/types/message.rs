use crate::types::JID;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInfo {
    pub id: String,
    pub chat: JID,
    pub sender: JID,
    pub timestamp: SystemTime,
    pub message_type: MessageType,
    pub from_me: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Text,
    Image,
    Video,
    Audio,
    Document,
    Location,
    Contact,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMessage {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMessage {
    pub url: Option<String>,
    pub direct_path: Option<String>,
    pub media_key: Option<Vec<u8>>,
    pub file_sha256: Option<Vec<u8>>,
    pub file_length: Option<u64>,
    pub mime_type: Option<String>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationMessage {
    pub latitude: f64,
    pub longitude: f64,
    pub name: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactMessage {
    pub display_name: String,
    pub vcard: String,
}

/// Represents a message that can be sent
#[derive(Debug, Clone)]
pub enum SendableMessage {
    Text(TextMessage),
    Image(MediaMessage),
    Video(MediaMessage),
    Audio(MediaMessage),
    Document(MediaMessage),
    Location(LocationMessage),
    Contact(ContactMessage),
}