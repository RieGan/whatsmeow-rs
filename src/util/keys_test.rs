#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ec_keypair_generation() {
        let keypair1 = ECKeyPair::generate();
        let keypair2 = ECKeyPair::generate();
        
        // Keys should be 32 bytes
        assert_eq!(keypair1.private_bytes().len(), 32);
        assert_eq!(keypair1.public_bytes().len(), 32);
        
        // Different generations should produce different keys
        assert_ne!(keypair1.private_bytes(), keypair2.private_bytes());
        assert_ne!(keypair1.public_bytes(), keypair2.public_bytes());
    }
    
    #[test]
    fn test_ec_keypair_from_private_bytes() {
        let private_key = [42u8; 32];
        let keypair = ECKeyPair::from_private_bytes(&private_key).unwrap();
        
        // The private key should be clamped for X25519
        let clamped_private = keypair.private_bytes();
        assert_eq!(clamped_private[0] & 7, 0); // Lower 3 bits cleared
        assert_eq!(clamped_private[31] & 128, 0); // Bit 255 cleared
        assert_eq!(clamped_private[31] & 64, 64); // Bit 254 set
    }
    
    #[test]
    fn test_ecdh_consistency() {
        let alice = ECKeyPair::generate();
        let bob = ECKeyPair::generate();
        
        // Perform ECDH from both sides
        let shared_alice = alice.ecdh(&bob.public_bytes());
        let shared_bob = bob.ecdh(&alice.public_bytes());
        
        // Shared secrets should be identical
        assert_eq!(shared_alice, shared_bob);
    }
    
    #[test]
    fn test_ecdh_different_keys() {
        let alice = ECKeyPair::generate();
        let bob = ECKeyPair::generate();
        let charlie = ECKeyPair::generate();
        
        let shared_alice_bob = alice.ecdh(&bob.public_bytes());
        let shared_alice_charlie = alice.ecdh(&charlie.public_bytes());
        
        // Different key exchanges should produce different results
        assert_ne!(shared_alice_bob, shared_alice_charlie);
    }
    
    #[test]
    fn test_signing_keypair_generation() {
        let keypair1 = SigningKeyPair::generate();
        let keypair2 = SigningKeyPair::generate();
        
        // Keys should be 32 bytes
        assert_eq!(keypair1.private_bytes().len(), 32);
        assert_eq!(keypair1.public_bytes().len(), 32);
        
        // Different generations should produce different keys
        assert_ne!(keypair1.private_bytes(), keypair2.private_bytes());
        assert_ne!(keypair1.public_bytes(), keypair2.public_bytes());
    }
    
    #[test]
    fn test_signing_keypair_from_private_bytes() {
        let private_key = [1u8; 32];
        let keypair = SigningKeyPair::from_private_bytes(&private_key).unwrap();
        
        assert_eq!(keypair.private_bytes(), private_key);
        assert_eq!(keypair.public_bytes().len(), 32);
    }
    
    #[test]
    fn test_invalid_private_key_length() {
        let invalid_key = [1u8; 16]; // Wrong length
        assert!(ECKeyPair::from_private_bytes(&invalid_key).is_err());
        assert!(SigningKeyPair::from_private_bytes(&invalid_key).is_err());
    }
}