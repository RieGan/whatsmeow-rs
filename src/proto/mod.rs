// WhatsApp Protocol Buffer Definitions
//
// This module contains the generated protobuf structs from WhatsApp's .proto files

// Include generated protobuf modules if compilation succeeded  
// The generated files will be placed in src/proto/generated/ by build.rs
pub mod generated {
    #![allow(warnings, unused)] // Generated code may have warnings
    
    // Conditionally include generated protobuf files
    // These will only be available if protoc compilation succeeded
    macro_rules! include_proto {
        ($path:expr) => {
            #[cfg(all(feature = "default", not(any(target_os = "windows"))))]
            include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/proto/generated/", $path, ".rs"));
        };
    }
    
    include_proto!("wa_common");
    include_proto!("wa_web");
    include_proto!("wa_e2e");
    include_proto!("wa_msg_transport");
}

// Fallback structures when protobuf compilation is not available
pub mod fallback {
    use serde::{Deserialize, Serialize};
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageProto {
        pub key: String,
        pub content: Vec<u8>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HandshakeMessage {
        pub client_hello: Vec<u8>,
        pub server_hello: Vec<u8>,
    }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageKey {
        pub remote_jid: Option<String>,
        pub from_me: Option<bool>,
        pub id: Option<String>,
        pub participant: Option<String>,
    }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MessageText {
        pub text: Option<String>,
        pub mentioned_jid: Vec<String>,
    }
    
    /// Placeholder message for testing
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PlaceholderMessage {
        pub content: String,
    }
}

// Protobuf utility functions
pub mod utils;

// Try to use generated protobuf, fall back to manual definitions
// This allows the library to work even without protoc installed
pub use fallback::*;

// Also re-export generated types when available (for advanced users)
pub use generated as proto_generated;

// Re-export utilities
pub use utils::ProtoUtils;