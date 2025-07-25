use std::collections::HashMap;
use once_cell::sync::Lazy;

#[cfg(test)]
mod tests;

/// Dictionary version sent to the server
pub const DICT_VERSION: u8 = 3;

/// Token type constants used in binary XML representation
pub const LIST_EMPTY: u8 = 0;
pub const DICTIONARY_0: u8 = 236;
pub const DICTIONARY_1: u8 = 237;
pub const DICTIONARY_2: u8 = 238;
pub const DICTIONARY_3: u8 = 239;
pub const INTEROP_JID: u8 = 245;
pub const FB_JID: u8 = 246;
pub const AD_JID: u8 = 247;
pub const LIST_8: u8 = 248;
pub const LIST_16: u8 = 249;
pub const JID_PAIR: u8 = 250;
pub const HEX_8: u8 = 251;
pub const BINARY_8: u8 = 252;
pub const BINARY_20: u8 = 253;
pub const BINARY_32: u8 = 254;
pub const NIBBLE_8: u8 = 255;

/// Other constants
pub const PACKED_MAX: u8 = 127;
pub const SINGLE_BYTE_MAX: u16 = 256;

/// Single-byte tokens used in WhatsApp binary protocol
pub const SINGLE_BYTE_TOKENS: &[&str] = &[
    "",
    "xmlstreamstart",
    "xmlstreamend", 
    "s.whatsapp.net",
    "type",
    "participant",
    "from",
    "receipt",
    "id",
    "notification",
    "disappearing_mode",
    "status",
    "jid",
    "broadcast",
    "user",
    "devices",
    "device_hash",
    "to",
    "offline",
    "message",
    "result",
    "class",
    "xmlns",
    "duration",
    "notify",
    "iq",
    "t",
    "ack",
    "g.us",
    "enc",
    "urn:xmpp:whatsapp:push",
    "presence",
    "config_value",
    "picture",
    "verified_name",
    "config_code",
    "key-index-list",
    "contact",
    "mediatype",
    "routing_info",
    "edge_routing",
    "get",
    "read",
    "urn:xmpp:ping",
    "fallback_hostname",
    "0",
    "chatstate",
    "business_hours_config",
    "unavailable",
    "download_buckets",
    "skmsg",
    "verified_level",
    "composing",
    "handshake",
    "device-list",
    "media",
    "text",
    "fallback_ip4",
    "media_conn",
    "device",
    "creation",
    "location",
    "config",
    "item",
    "fallback_ip6",
    "count",
    "w:profile:picture",
    "image",
    "business",
    "2",
    "hostname",
    "call-creator",
    "display_name",
    "relaylatency",
    "platform",
    "abprops",
    "success",
    "msg",
    "offline_preview",
    "prop",
    "key-index",
    "v",
    "day_of_week",
    "pkmsg",
    "version",
    "1",
    "ping",
    "w:p",
    "download",
    "video",
    "set",
    "specific_hours",
    "props",
    "primary",
    "unknown",
    "hash",
    "commerce_experience",
    "last",
    "subscribe",
    "max_buckets",
    "call",
    "profile",
    "member_since_text",
    "close_time",
    "call-id",
    "sticker",
    "mode",
    "participants",
    "value",
    "query",
    "profile_options",
    "open_time",
    "code",
    "list",
    "host",
    "ts",
    "contacts",
    "upload",
    "lid",
    "preview",
    "update",
    "usync",
    "w:stats",
    "delivery",
    "auth_ttl",
    "context",
    "fail",
    "cart_enabled",
    "appdata",
    "category",
    "atn",
    "direct_connection",
    "decrypt-fail",
    "relay_id",
    "mmg-fallback.whatsapp.net",
    "target",
    "available",
    "name",
    "last_id",
    "mmg.whatsapp.net",
    "categories",
    "401",
    "is_new",
    "index",
    "tctoken",
    "ip4",
    "token_id",
    "latency",
    "recipient",
    "edit",
    "ip6",
    "add",
    "thumbnail-document",
    "26",
    "paused",
    "true",
    "identity",
    "stream:error",
    "key",
    "sidelist",
    "background",
    "audio",
    "3",
    "thumbnail-image",
    "biz-cover-photo",
    "cat",
    "gcm",
    "thumbnail-video",
    "error",
    "auth",
    "deny",
    "serial",
    "in",
    "registration",
    "thumbnail-link",
    "remove",
    "00",
    "gif",
    "thumbnail-gif",
    "tag",
    "capability",
    "multicast",
    "item-not-found",
    "description",
    "business_hours",
    "config_expo_key",
    "md-app-state",
    "expiration",
    "fallback",
    "ttl",
    "300",
    "md-msg-hist",
    "device_orientation",
    "out",
    "w:m",
    "open_24h",
    "side_list",
    "token",
    "inactive",
    "01",
    "document",
    "te2",
    "played",
    "encrypt",
    "msgr",
    "hide",
    "direct_path",
    "12",
    "state",
    "not-authorized",
    "url",
    "terminate",
    "signature",
    "status-revoke-delay",
    "02",
    "te",
    "linked_accounts",
    "trusted_contact",
    "timezone",
    "ptt",
    "kyc-id",
    "privacy_token",
    "readreceipts",
    "appointment_only",
    "address",
    "expected_ts",
    "privacy",
    "7",
    "android",
    "interactive",
    "device-identity",
    "enabled",
    "attribute_padding",
    "1080",
    "03",
    "screen_height",
];

/// Double-byte tokens grouped by dictionary
pub const DOUBLE_BYTE_TOKENS: &[&[&str]] = &[
    // Dictionary 0
    &[
        "read-self", "active", "fbns", "protocol", "reaction", "screen_width", "heartbeat", 
        "deviceid", "2:47DEQpj8", "uploadfieldstat", "voip_settings", "retry", "priority", 
        "longitude", "conflict", "false", "ig_professional", "replaced", "preaccept", 
        "cover_photo", "uncompressed", "encopt", "ppic", "04", "passive", "status-revoke-drop",
        // ... truncated for brevity, full list would continue
    ],
    // Dictionary 1
    &[
        "reject", "dirty", "announcement", "020", "13", "9", "status_video_max_bitrate",
        // ... truncated for brevity
    ],
    // Dictionary 2 
    &[
        "64", "ptt_playback_speed_enabled", "web_product_list_message_enabled",
        // ... truncated for brevity
    ],
    // Dictionary 3
    &[
        "1724", "profile_picture", "1071", "1314", "1605", "407", "990", "1710",
        // ... truncated for brevity
    ],
];

/// Index for fast single-byte token lookup
static SINGLE_BYTE_TOKEN_INDEX: Lazy<HashMap<&'static str, u8>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for (index, &token) in SINGLE_BYTE_TOKENS.iter().enumerate() {
        if !token.is_empty() {
            map.insert(token, index as u8);
        }
    }
    map
});

/// Index for fast double-byte token lookup
static DOUBLE_BYTE_TOKEN_INDEX: Lazy<HashMap<&'static str, (u8, u8)>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for (dict_index, &tokens) in DOUBLE_BYTE_TOKENS.iter().enumerate() {
        for (token_index, &token) in tokens.iter().enumerate() {
            map.insert(token, (dict_index as u8, token_index as u8));
        }
    }
    map
});

/// Get the string value of a double-byte token
pub fn get_double_token(dict: u8, index: u8) -> Option<&'static str> {
    DOUBLE_BYTE_TOKENS
        .get(dict as usize)
        .and_then(|tokens| tokens.get(index as usize))
        .copied()
}

/// Get the index of a single-byte token
pub fn index_of_single_token(token: &str) -> Option<u8> {
    SINGLE_BYTE_TOKEN_INDEX.get(token).copied()
}

/// Get the index of a double-byte token
pub fn index_of_double_token(token: &str) -> Option<(u8, u8)> {
    DOUBLE_BYTE_TOKEN_INDEX.get(token).copied()
}

/// Get single-byte token by index
pub fn get_single_token(index: u8) -> Option<&'static str> {
    SINGLE_BYTE_TOKENS.get(index as usize).copied()
}