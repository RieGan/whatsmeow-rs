#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    
    #[test]
    fn test_user_jid_parsing() {
        let jid = JID::from_str("1234567890@s.whatsapp.net").unwrap();
        
        assert_eq!(jid.user, "1234567890");
        assert_eq!(jid.server, "s.whatsapp.net");
        assert_eq!(jid.device, 0);
        assert_eq!(jid.agent, 0);
        assert!(jid.is_user());
        assert!(!jid.is_group());
    }
    
    #[test]
    fn test_group_jid_parsing() {
        let jid = JID::from_str("1234567890-5678901234@g.us").unwrap();
        
        assert_eq!(jid.user, "1234567890-5678901234");
        assert_eq!(jid.server, "g.us");
        assert!(!jid.is_user());
        assert!(jid.is_group());
    }
    
    #[test]
    fn test_broadcast_jid_parsing() {
        let jid = JID::from_str("status@broadcast").unwrap();
        
        assert_eq!(jid.user, "status");
        assert_eq!(jid.server, "broadcast");
        assert!(jid.is_broadcast());
    }
    
    #[test]
    fn test_device_jid_parsing() {
        let jid = JID::from_str("1234567890.123:456@s.whatsapp.net").unwrap();
        
        assert_eq!(jid.user, "1234567890");
        assert_eq!(jid.device, 123);
        assert_eq!(jid.agent, 456);
        assert_eq!(jid.server, "s.whatsapp.net");
    }
    
    #[test]
    fn test_jid_to_string() {
        let jid = JID {
            user: "1234567890".to_string(),
            server: "s.whatsapp.net".to_string(),
            device: 0,
            agent: 0,
        };
        
        assert_eq!(jid.to_string(), "1234567890@s.whatsapp.net");
    }
    
    #[test]
    fn test_device_jid_to_string() {
        let jid = JID {
            user: "1234567890".to_string(),
            server: "s.whatsapp.net".to_string(),
            device: 123,
            agent: 456,
        };
        
        assert_eq!(jid.to_string(), "1234567890.123:456@s.whatsapp.net");
    }
    
    #[test]
    fn test_invalid_jid_parsing() {
        // Missing @
        assert!(JID::from_str("1234567890").is_err());
        
        // Empty user
        assert!(JID::from_str("@s.whatsapp.net").is_err());
        
        // Empty server
        assert!(JID::from_str("1234567890@").is_err());
        
        // Invalid device format
        assert!(JID::from_str("1234567890.abc@s.whatsapp.net").is_err());
    }
    
    #[test]
    fn test_jid_equality() {
        let jid1 = JID::from_str("1234567890@s.whatsapp.net").unwrap();
        let jid2 = JID::from_str("1234567890@s.whatsapp.net").unwrap();
        let jid3 = JID::from_str("0987654321@s.whatsapp.net").unwrap();
        
        assert_eq!(jid1, jid2);
        assert_ne!(jid1, jid3);
    }
    
    #[test]
    fn test_jid_to_user_jid() {
        let device_jid = JID::from_str("1234567890.123:456@s.whatsapp.net").unwrap();
        let user_jid = device_jid.to_user_jid();
        
        assert_eq!(user_jid.user, "1234567890");
        assert_eq!(user_jid.server, "s.whatsapp.net");
        assert_eq!(user_jid.device, 0);
        assert_eq!(user_jid.agent, 0);
    }
}