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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    Text,
    Image,
    Video,
    Audio,
    Voice,
    Document,
    Sticker,
    Location,
    LiveLocation,
    Contact,
    ContactsArray,
    Quote,
    Reaction,
    Poll,
    PollUpdate,
    GroupInvite,
    Payment,
    Order,
    Product,
    ListMessage,
    ButtonMessage,
    TemplateMessage,
    HighlyStructuredMessage,
    InteractiveMessage,
    Call,
    ProtocolMessage,
    AppState,
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
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub page_count: Option<u32>,
    pub seconds: Option<u32>,
    pub ptt: Option<bool>, // Push to talk (voice note)
    pub gif_playback: Option<bool>,
    pub jpeg_thumbnail: Option<Vec<u8>>,
    pub context_info: Option<ContextInfo>,
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

/// Message receipts and status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageStatus {
    Pending,
    Sent,
    Delivered,
    Read,
    Played,
    Failed,
}

/// Message receipt information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReceipt {
    pub message_id: String,
    pub status: MessageStatus,
    pub timestamp: SystemTime,
    pub participant: Option<JID>, // For group messages
}

/// Context information for messages (replies, forwards, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextInfo {
    pub quoted_message: Option<Box<QuotedMessage>>,
    pub mentioned_jids: Vec<JID>,
    pub forwarded: Option<bool>,
    pub forwarding_score: Option<u32>,
    pub is_forwarded: Option<bool>,
    pub ephemeral_setting: Option<u32>,
    pub ephemeral_shared_secret: Option<Vec<u8>>,
    pub external_ad_reply: Option<ExternalAdReply>,
}

/// Quoted message information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotedMessage {
    pub id: String,
    pub remote_jid: JID,
    pub participant: Option<JID>,
    pub message_type: MessageType,
    pub text: Option<String>,
    pub media_type: Option<String>,
}

/// External ad reply information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAdReply {
    pub title: Option<String>,
    pub body: Option<String>,
    pub media_type: Option<String>,
    pub thumbnail_url: Option<String>,
    pub media_url: Option<String>,
    pub source_url: Option<String>,
}

/// Reaction message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionMessage {
    pub key: MessageKey,
    pub text: String, // Emoji
    pub sender_timestamp: Option<SystemTime>,
}

/// Message key for referencing messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageKey {
    pub remote_jid: JID,
    pub from_me: bool,
    pub id: String,
    pub participant: Option<JID>,
}

/// Poll message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollMessage {
    pub name: String,
    pub options: Vec<PollOption>,
    pub selectable_options_count: u32,
    pub context_info: Option<ContextInfo>,
}

/// Poll option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollOption {
    pub name: String,
}

/// Poll update (vote)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollUpdateMessage {
    pub poll_creation_message_key: MessageKey,
    pub vote: PollVote,
    pub sender_timestamp: Option<SystemTime>,
}

/// Poll vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollVote {
    pub selected_options: Vec<String>,
}

/// Group invite message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInviteMessage {
    pub group_jid: JID,
    pub invite_code: String,
    pub invite_expiration: Option<SystemTime>,
    pub group_name: Option<String>,
    pub group_type: Option<String>,
    pub jpeg_thumbnail: Option<Vec<u8>>,
    pub caption: Option<String>,
    pub context_info: Option<ContextInfo>,
}

/// Enhanced text message with formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedTextMessage {
    pub text: String,
    pub matched_text: Option<String>,
    pub canonical_url: Option<String>,
    pub description: Option<String>,
    pub title: Option<String>,
    pub text_arg_b: Option<String>,
    pub thumbnail: Option<Vec<u8>>,
    pub jpeg_thumbnail: Option<Vec<u8>>,
    pub context_info: Option<ContextInfo>,
    pub font: Option<u32>,
    pub preview_type: Option<u32>,
}

/// Protocol message for system messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub key: Option<MessageKey>,
    pub message_type: ProtocolMessageType,
    pub ephemeral_expiration: Option<u32>,
    pub ephemeral_setting_timestamp: Option<SystemTime>,
    pub history_sync_notification: Option<HistorySyncNotification>,
    pub app_state_sync_key_share: Option<AppStateSyncKeyShare>,
    pub initial_security_notification_setting_sync: Option<InitialSecurityNotificationSettingSync>,
    pub app_state_sync_key_request: Option<AppStateSyncKeyRequest>,
}

/// Protocol message types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProtocolMessageType {
    Revoke,
    EphemeralSetting,
    EphemeralSyncResponse,
    HistorySyncNotification,
    AppStateSyncKeyShare,
    AppStateSyncKeyRequest,
    MessageEdit,
    PeerDataOperationRequestMessage,
    PeerDataOperationRequestResponseMessage,
    BotFeedbackMessage,
    InvoiceMessage,
    RequestPhoneNumber,
    Unknown,
}

/// History sync notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorySyncNotification {
    pub file_sha256: Vec<u8>,
    pub file_length: u64,
    pub media_key: Vec<u8>,
    pub file_enc_sha256: Vec<u8>,
    pub direct_path: String,
    pub sync_type: HistorySyncType,
    pub chunk_order: u32,
    pub original_message_id: String,
}

/// History sync types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HistorySyncType {
    InitialBootstrap,
    InitialStatusV3,
    Full,
    Recent,
    PushName,
    NonBlockingData,
    OnDemandSync,
}

/// App state sync key share
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSyncKeyShare {
    pub keys: Vec<AppStateSyncKey>,
}

/// App state sync key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSyncKey {
    pub key_id: Vec<u8>,
    pub key_data: AppStateSyncKeyData,
}

/// App state sync key data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSyncKeyData {
    pub key_data: Vec<u8>,
    pub timestamp: SystemTime,
    pub fingerprint: Vec<u8>,
}

/// Initial security notification setting sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitialSecurityNotificationSettingSync {
    pub security_notification_enabled: bool,
}

/// App state sync key request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSyncKeyRequest {
    pub key_ids: Vec<Vec<u8>>,
}

/// Represents a message that can be sent
#[derive(Debug, Clone)]
pub enum SendableMessage {
    Text(TextMessage),
    ExtendedText(ExtendedTextMessage),
    Image(MediaMessage),
    Video(MediaMessage),
    Audio(MediaMessage),
    Voice(MediaMessage),
    Document(MediaMessage),
    Sticker(MediaMessage),
    Location(LocationMessage),
    Contact(ContactMessage),
    Quote(QuotedMessage),
    Reaction(ReactionMessage),
    Poll(PollMessage),
    PollUpdate(PollUpdateMessage),
    GroupInvite(GroupInviteMessage),
    Protocol(ProtocolMessage),
}