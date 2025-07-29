use crate::types::{JID, MessageInfo, MessageReceipt};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Event handler function type
pub type EventHandler = Box<dyn Fn(Event) -> bool + Send + Sync>;

/// All possible events that can be emitted by the WhatsApp client
#[derive(Debug, Clone)]
pub enum Event {
    /// Connection state changed
    Connected,
    Disconnected { reason: String },
    
    /// Authentication events
    LoggedIn,
    LoggedOut,
    QRCode { code: String },
    
    /// Message events
    Message(MessageInfo),
    MessageReceipt { receipt: MessageReceipt },
    MessageRevoke(MessageRevokeEvent),
    MessageAck(MessageAckEvent),
    
    /// Presence events
    Presence(PresenceEvent),
    
    /// Group events
    GroupInfo(GroupInfoEvent),
    GroupParticipants(GroupParticipantsEvent),
    
    /// Other events
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEvent {
    pub info: MessageInfo,
    pub message: Vec<u8>, // Raw message data for now
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRevokeEvent {
    pub chat: JID,
    pub sender: JID,
    pub id: String,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAckEvent {
    pub chat: JID,
    pub sender: JID,
    pub ids: Vec<String>,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceEvent {
    pub from: JID,
    pub unavailable: bool,
    pub last_seen: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfoEvent {
    pub jid: JID,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub participants: Vec<JID>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupParticipantsEvent {
    pub jid: JID,
    pub participants: Vec<JID>,
    pub action: GroupParticipantAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GroupParticipantAction {
    Add,
    Remove,
    Promote,
    Demote,
}