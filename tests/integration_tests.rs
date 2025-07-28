use whatsmeow::{
    Client,
    auth::AuthState,
    store::{MemoryStore, DeviceStore},
    types::{Event, SendableMessage, TextMessage, JID},
    binary::{Node, NodeContent},
    util::keys::{ECKeyPair, SigningKeyPair},
    proto::ProtoUtils,
};
use std::sync::Arc;

#[tokio::test]
async fn test_client_creation() {
    let store: Arc<dyn DeviceStore> = Arc::new(MemoryStore::new());
    let client = Client::new(store);
    
    // Client should start in logged out state
    assert!(!client.is_logged_in());
    
    // Should be able to check auth state
    let auth_state = client.auth_state().await;
    assert!(matches!(auth_state, AuthState::Unauthenticated));
}

#[tokio::test]
async fn test_qr_generation() {
    let store: Arc<dyn DeviceStore> = Arc::new(MemoryStore::new());
    let client = Client::new(store);
    
    // Should be able to generate QR code
    let qr_result = client.generate_qr().await;
    assert!(qr_result.is_ok());
    
    let qr_code = qr_result.unwrap();
    assert!(!qr_code.is_empty());
}

#[tokio::test]
async fn test_event_handler() {
    let store: Arc<dyn DeviceStore> = Arc::new(MemoryStore::new());
    let client = Client::new(store);
    
    let mut event_received = false;
    
    // Add event handler
    client.add_event_handler(Box::new(|event| {
        match event {
            Event::QRCode { .. } => {
                // Event handler should be called
                true
            },
            _ => true,
        }
    })).await;
    
    // Generate QR should trigger event
    let _ = client.generate_qr().await;
}

#[test]
fn test_jid_parsing() {
    let jid: JID = "1234567890@s.whatsapp.net".parse().unwrap();
    
    assert_eq!(jid.user, "1234567890");
    assert_eq!(jid.server, "s.whatsapp.net");
    assert!(jid.is_user());
    assert!(!jid.is_group());
}

#[test]
fn test_message_creation() {
    let text_msg = TextMessage {
        text: "Hello, World!".to_string(),
    };
    
    let sendable = SendableMessage::Text(text_msg);
    
    match sendable {
        SendableMessage::Text(msg) => {
            assert_eq!(msg.text, "Hello, World!");
        },
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_binary_node_creation() {
    let mut attrs = std::collections::HashMap::new();
    attrs.insert("id".to_string(), "test123".to_string());
    
    let node = Node {
        tag: "message".to_string(),
        attrs,
        content: NodeContent::Text("Hello".to_string()),
    };
    
    assert_eq!(node.tag, "message");
    assert_eq!(node.attrs.get("id"), Some(&"test123".to_string()));
    
    match node.content {
        NodeContent::Text(text) => assert_eq!(text, "Hello"),
        _ => panic!("Wrong content type"),
    }
}

#[test]
fn test_key_generation() {
    let ec_keypair = ECKeyPair::generate();
    let signing_keypair = SigningKeyPair::generate();
    
    // Keys should be proper length
    assert_eq!(ec_keypair.private_bytes().len(), 32);
    assert_eq!(ec_keypair.public_bytes().len(), 32);
    assert_eq!(signing_keypair.private_bytes().len(), 32);
    assert_eq!(signing_keypair.public_bytes().len(), 32);
    
    // Different generations should produce different keys
    let ec_keypair2 = ECKeyPair::generate();
    assert_ne!(ec_keypair.private_bytes(), ec_keypair2.private_bytes());
}

#[test]
fn test_ecdh() {
    let alice = ECKeyPair::generate();
    let bob = ECKeyPair::generate();
    
    let shared_alice = alice.ecdh(&bob.public_bytes());
    let shared_bob = bob.ecdh(&alice.public_bytes());
    
    // ECDH should produce the same shared secret from both sides
    assert_eq!(shared_alice, shared_bob);
}

#[test]
fn test_protobuf_utils() {
    let text_msg = ProtoUtils::create_text_message("Test message");
    assert_eq!(text_msg.text, Some("Test message".to_string()));
    
    let msg_key = ProtoUtils::create_message_key("test@example.com", "msg123", true);
    assert_eq!(msg_key.remote_jid, Some("test@example.com".to_string()));
    assert_eq!(msg_key.id, Some("msg123".to_string()));
    assert_eq!(msg_key.from_me, Some(true));
}

#[tokio::test]
async fn test_memory_store() {
    let store = MemoryStore::new();
    
    // Initially should have no device data
    assert!(!store.is_registered().await.unwrap());
    assert!(store.load_device().await.unwrap().is_none());
}