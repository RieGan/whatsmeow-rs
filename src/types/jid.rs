use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_jid_parsing() {
        let jid_str = "1234567890@s.whatsapp.net";
        let jid = JID::parse(jid_str).unwrap();
        assert_eq!(jid.user, "1234567890");
        assert_eq!(jid.server, "s.whatsapp.net");
        assert_eq!(jid.to_string(), jid_str);
    }
    
    #[test]
    fn test_group_jid() {
        let group_jid = JID::new_group("groupid123");
        assert_eq!(group_jid.server, "g.us");
        assert!(group_jid.is_group());
    }
}

/// JID (Jabber ID) represents a WhatsApp user or group identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct JID {
    pub user: String,
    pub agent: u8,
    pub device: u8,
    pub server: String,
    pub ad: bool,
}

impl JID {
    /// Create a new JID
    pub fn new(user: String, server: String) -> Self {
        Self {
            user,
            agent: 0,
            device: 0,
            server,
            ad: false,
        }
    }
    
    /// Parse a JID from string format "user@server"
    pub fn parse(jid_str: &str) -> Result<Self, crate::error::Error> {
        let parts: Vec<&str> = jid_str.split('@').collect();
        if parts.len() != 2 {
            return Err(crate::error::Error::InvalidJID(format!("Invalid JID format: {}", jid_str)));
        }
        
        Ok(Self::new(parts[0].to_string(), parts[1].to_string()))
    }
    
    /// Create a new group JID
    pub fn new_group(group_id: &str) -> Self {
        Self::new(group_id.to_string(), "g.us".to_string())
    }
    
    /// Check if this is a user JID
    pub fn is_user(&self) -> bool {
        self.server == "s.whatsapp.net"
    }
    
    /// Check if this is a group JID
    pub fn is_group(&self) -> bool {
        self.server == "g.us"
    }
    
    /// Check if this is a server JID
    pub fn is_server(&self) -> bool {
        self.server == "server"
    }
    
    /// Get the string representation without device info
    pub fn to_non_ad(&self) -> String {
        format!("{}@{}", self.user, self.server)
    }
}

impl fmt::Display for JID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.agent != 0 || self.device != 0 {
            write!(f, "{}.{}:{}@{}", self.user, self.agent, self.device, self.server)
        } else {
            write!(f, "{}@{}", self.user, self.server)
        }
    }
}

impl std::str::FromStr for JID {
    type Err = crate::Error;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('@').collect();
        if parts.len() != 2 {
            return Err(crate::Error::Protocol(format!("Invalid JID format: {}", s)));
        }
        
        let user_part = parts[0];
        let server = parts[1].to_string();
        
        if user_part.contains('.') && user_part.contains(':') {
            let agent_device: Vec<&str> = user_part.split('.').collect();
            let user = agent_device[0].to_string();
            let device_parts: Vec<&str> = agent_device[1].split(':').collect();
            let agent = device_parts[0].parse().map_err(|_| {
                crate::Error::Protocol(format!("Invalid agent in JID: {}", s))
            })?;
            let device = device_parts[1].parse().map_err(|_| {
                crate::Error::Protocol(format!("Invalid device in JID: {}", s))
            })?;
            
            Ok(JID {
                user,
                agent,
                device,
                server,
                ad: true,
            })
        } else {
            Ok(JID {
                user: user_part.to_string(),
                agent: 0,
                device: 0,
                server,
                ad: false,
            })
        }
    }
}