use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
    
    #[error("Protobuf decode error: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),
    
    #[error("Cryptographic error: {0}")]
    Crypto(String),
    
    #[error("Authentication failed: {0}")]
    Auth(String),
    
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Not logged in")]
    NotLoggedIn,
    
    #[error("Disconnected: {0}")]
    Disconnected(String),
    
    #[error("Element missing: {0}")]
    ElementMissing(String),
    
    #[error("IQ error - code: {code}, text: {text}")]
    IQ { code: u16, text: String },
}