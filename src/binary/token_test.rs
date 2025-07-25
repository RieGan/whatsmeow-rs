#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_single_byte_token_lookup() {
        // Test valid token indices
        assert_eq!(get_single_byte_token(0).unwrap(), "");
        assert_eq!(get_single_byte_token(1).unwrap(), "xmlstreamstart");
        assert_eq!(get_single_byte_token(2).unwrap(), "xmlstreamend");
        
        // Test invalid index
        assert!(get_single_byte_token(1000).is_none());
    }
    
    #[test]
    fn test_double_byte_token_lookup() {
        // Test valid token indices
        assert!(get_double_byte_token(0).is_some());
        assert!(get_double_byte_token(100).is_some());
        
        // Test invalid index
        assert!(get_double_byte_token(10000).is_none());
    }
    
    #[test]
    fn test_token_array_lengths() {
        // Ensure token arrays are not empty
        assert!(!SINGLE_BYTE_TOKENS.is_empty());
        assert!(!DOUBLE_BYTE_TOKENS.is_empty());
        
        // Ensure reasonable sizes
        assert!(SINGLE_BYTE_TOKENS.len() > 200);
        assert!(DOUBLE_BYTE_TOKENS.len() > 500);
    }
    
    #[test]
    fn test_whatsapp_specific_tokens() {
        // Test for some known WhatsApp-specific tokens
        assert!(SINGLE_BYTE_TOKENS.contains(&"s.whatsapp.net"));
        assert!(SINGLE_BYTE_TOKENS.contains(&"type"));
        assert!(SINGLE_BYTE_TOKENS.contains(&"message"));
    }
    
    #[test]
    fn test_token_uniqueness() {
        // Single byte tokens should be unique
        let mut single_set = std::collections::HashSet::new();
        for token in SINGLE_BYTE_TOKENS {
            assert!(single_set.insert(*token), "Duplicate single byte token: {}", token);
        }
        
        // Double byte tokens should be unique
        let mut double_set = std::collections::HashSet::new();
        for token in DOUBLE_BYTE_TOKENS {
            assert!(double_set.insert(*token), "Duplicate double byte token: {}", token);
        }
    }
    
    #[test]
    fn test_constants() {
        // Test binary protocol constants
        assert_eq!(LIST_EMPTY, 0);
        assert_eq!(LIST_8, 248);
        assert_eq!(LIST_16, 249);
        assert_eq!(JID_PAIR, 250);
        assert_eq!(HEX_8, 251);
        assert_eq!(BINARY_8, 252);
        assert_eq!(BINARY_20, 253);
        assert_eq!(BINARY_32, 254);
        assert_eq!(NIBBLE_8, 255);
    }
}