use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    
    #[error("JSON serialization error: {0}")]
    Json(String),
    
    #[error("IO error: {0}")]
    Io(String),
    
    #[error("URL parse error: {0}")]
    UrlParse(String),
    
    #[error("Protobuf decode error: {0}")]
    ProtobufDecode(String),
    
    #[error("Cryptographic error: {0}")]
    Crypto(String),
    
    #[error("Authentication failed: {0}")]
    Auth(String),
    
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Invalid JID: {0}")]
    InvalidJID(String),
    
    #[error("Not logged in")]
    NotLoggedIn,
    
    #[error("Disconnected: {0}")]
    Disconnected(String),
    
    #[error("Element missing: {0}")]
    ElementMissing(String),
    
    #[error("IQ error - code: {code}, text: {text}")]
    IQ { code: u16, text: String },
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Error::WebSocket(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err.to_string())
    }
}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Error::UrlParse(err.to_string())
    }
}

impl From<prost::DecodeError> for Error {
    fn from(err: prost::DecodeError) -> Self {
        Error::ProtobufDecode(err.to_string())
    }
}