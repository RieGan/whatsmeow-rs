use crate::error::{Error, Result};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, tungstenite::http::HeaderValue};
use futures_util::{SinkExt, StreamExt};
use tracing::{debug, info, warn};
use std::collections::HashMap;
use url::Url;

pub mod noise;

/// WhatsApp WebSocket endpoints
pub const WHATSAPP_WS_URL: &str = "wss://web.whatsapp.com/ws/chat";
pub const WHATSAPP_WS_URL_2: &str = "wss://web.whatsapp.com/ws";

/// Noise protocol socket for WhatsApp communication
pub struct NoiseSocket {
    ws_stream: Option<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
    noise_handshake: Option<crate::socket::noise::NoiseHandshake>,
    connected: bool,
}

impl NoiseSocket {
    /// Create a new noise socket
    pub async fn new() -> Result<Self> {
        info!("Creating new noise socket");
        
        Ok(Self {
            ws_stream: None,
            noise_handshake: None,
            connected: false,
        })
    }
    
    /// Connect to WhatsApp WebSocket with proper headers and handshake
    pub async fn connect(&mut self) -> Result<()> {
        self.connect_with_url(WHATSAPP_WS_URL).await
    }
    
    /// Connect to WhatsApp WebSocket with custom URL
    pub async fn connect_with_url(&mut self, url: &str) -> Result<()> {
        info!("Connecting to WhatsApp WebSocket at {}", url);
        
        // Parse URL and add required headers
        let mut parsed_url = Url::parse(url)?;
        
        // Add WhatsApp-specific query parameters
        parsed_url.query_pairs_mut()
            .append_pair("ed", "25519")  // Ed25519 support
            .append_pair("agent", "web"); // Web agent
        
        debug!("Establishing WebSocket connection to: {}", parsed_url.as_str());
        let (ws_stream, response) = connect_async(parsed_url.as_str()).await?;
        
        info!("WebSocket connected, status: {}", response.status());
        debug!("Response headers: {:?}", response.headers());
        
        self.ws_stream = Some(ws_stream);
        self.connected = true;
        
        // Initialize Noise handshake
        self.noise_handshake = Some(crate::socket::noise::NoiseHandshake::new());
        
        info!("WhatsApp WebSocket connection established");
        Ok(())
    }
    
    /// Build required headers for WhatsApp WebSocket
    fn build_headers(&self) -> HashMap<String, HeaderValue> {
        let mut headers = HashMap::new();
        
        // Add standard WebSocket headers that WhatsApp expects
        if let Ok(val) = HeaderValue::from_str("https://web.whatsapp.com") {
            headers.insert("Origin".to_string(), val);
        }
        if let Ok(val) = HeaderValue::from_str("chat, binary") {
            headers.insert("Sec-WebSocket-Protocol".to_string(), val);
        }
        if let Ok(val) = HeaderValue::from_str("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36") {
            headers.insert("User-Agent".to_string(), val);
        }
        
        headers
    }
    
    /// Send a message through the socket (with encryption if handshake complete)
    pub async fn send(&mut self, data: Vec<u8>) -> Result<()> {
        if !self.connected {
            return Err(Error::Connection("Socket not connected".to_string()));
        }
        
        if let Some(ref mut stream) = self.ws_stream {
            // If we have a completed noise handshake, encrypt the data
            let encrypted_data = if let Some(ref mut handshake) = self.noise_handshake {
                if handshake.is_completed() {
                    debug!("Encrypting message of {} bytes", data.len());
                    handshake.encrypt(&data)?
                } else {
                    debug!("Sending unencrypted handshake data");
                    data
                }
            } else {
                data
            };
            
            stream.send(Message::Binary(encrypted_data)).await?;
            debug!("Message sent successfully");
            Ok(())
        } else {
            Err(Error::Connection("Socket not connected".to_string()))
        }
    }
    
    /// Receive a message from the socket (with decryption if handshake complete)
    pub async fn receive(&mut self) -> Result<Option<Vec<u8>>> {
        if !self.connected {
            return Err(Error::Connection("Socket not connected".to_string()));
        }
        
        if let Some(ref mut stream) = self.ws_stream {
            if let Some(msg) = stream.next().await {
                match msg? {
                    Message::Binary(encrypted_data) => {
                        debug!("Received binary message of {} bytes", encrypted_data.len());
                        
                        // If we have a completed noise handshake, decrypt the data
                        let decrypted_data = if let Some(ref mut handshake) = self.noise_handshake {
                            if handshake.is_completed() {
                                debug!("Decrypting received message");
                                handshake.decrypt(&encrypted_data)?
                            } else {
                                debug!("Processing handshake data");
                                encrypted_data
                            }
                        } else {
                            encrypted_data
                        };
                        
                        Ok(Some(decrypted_data))
                    },
                    Message::Text(text) => {
                        debug!("Received text message: {}", text);
                        Ok(Some(text.into_bytes()))
                    },
                    Message::Close(frame) => {
                        warn!("WebSocket connection closed: {:?}", frame);
                        self.connected = false;
                        Err(Error::Disconnected("Connection closed by remote".to_string()))
                    },
                    Message::Ping(data) => {
                        debug!("Received ping, sending pong");
                        stream.send(Message::Pong(data)).await?;
                        Ok(None)
                    },
                    Message::Pong(_) => {
                        debug!("Received pong");
                        Ok(None)
                    },
                    _ => Ok(None),
                }
            } else {
                debug!("WebSocket stream ended");
                self.connected = false;
                Ok(None)
            }
        } else {
            Err(Error::Connection("Socket not connected".to_string()))
        }
    }
    
    /// Perform noise handshake with WhatsApp
    pub async fn perform_handshake(&mut self) -> Result<()> {
        if !self.connected {
            return Err(Error::Connection("Socket not connected".to_string()));
        }
        
        if self.noise_handshake.is_some() {
            info!("Starting Noise handshake with WhatsApp");
            
            // Create init message
            let init_message = {
                let handshake = self.noise_handshake.as_mut().unwrap();
                handshake.create_client_init()?
            };
            self.send(init_message).await?;
            
            // Wait for server response and process it
            if let Some(server_response) = self.receive().await? {
                let finish_message = {
                    let handshake = self.noise_handshake.as_mut().unwrap();
                    handshake.process_server_response(&server_response)?;
                    handshake.create_client_finish()?
                };
                self.send(finish_message).await?;
                
                info!("Noise handshake completed successfully");
                Ok(())
            } else {
                Err(Error::Auth("No handshake response from server".to_string()))
            }
        } else {
            Err(Error::Connection("No handshake instance available".to_string()))
        }
    }
    
    /// Check if the socket is connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }
    
    /// Check if the Noise handshake is completed
    pub fn is_handshake_completed(&self) -> bool {
        self.noise_handshake
            .as_ref()
            .map(|h| h.is_completed())
            .unwrap_or(false)
    }
    
    /// Send a ping frame
    pub async fn ping(&mut self) -> Result<()> {
        if let Some(ref mut stream) = self.ws_stream {
            stream.send(Message::Ping(vec![])).await?;
            Ok(())
        } else {
            Err(Error::Connection("Socket not connected".to_string()))
        }
    }
    
    /// Close the socket connection
    pub async fn close(mut self) -> Result<()> {
        if let Some(mut stream) = self.ws_stream.take() {
            info!("Closing WebSocket connection");
            stream.close(None).await?;
        }
        self.connected = false;
        Ok(())
    }
}