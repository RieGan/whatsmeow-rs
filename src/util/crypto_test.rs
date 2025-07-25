#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sha256() {
        let input = b"hello world";
        let hash = sha256(input);
        
        // SHA256 of "hello world" should be consistent
        assert_eq!(hash.len(), 32);
        assert_ne!(hash, [0u8; 32]); // Should not be all zeros
    }
    
    #[test]
    fn test_hkdf_expand() {
        let ikm = b"input key material";
        let salt = b"salt";
        let length = 32;
        
        let result = hkdf_expand(ikm, salt, length).unwrap();
        
        assert_eq!(result.len(), length);
        assert_ne!(result, vec![0u8; length]); // Should not be all zeros
    }
    
    #[test]
    fn test_aes_gcm_encrypt_decrypt() {
        let key = [1u8; 32];
        let nonce = [2u8; 12];
        let plaintext = b"secret message";
        
        let aes = AesGcm::new(&key).unwrap();
        
        // Encrypt
        let ciphertext = aes.encrypt(&nonce, plaintext).unwrap();
        assert_ne!(ciphertext, plaintext);
        assert!(ciphertext.len() > plaintext.len()); // Should include auth tag
        
        // Decrypt
        let decrypted = aes.decrypt(&nonce, &ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_aes_gcm_wrong_key() {
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];
        let nonce = [3u8; 12];
        let plaintext = b"secret message";
        
        let aes1 = AesGcm::new(&key1).unwrap();
        let aes2 = AesGcm::new(&key2).unwrap();
        
        let ciphertext = aes1.encrypt(&nonce, plaintext).unwrap();
        
        // Decryption with wrong key should fail
        assert!(aes2.decrypt(&nonce, &ciphertext).is_err());
    }
    
    #[test]
    fn test_generate_key() {
        let key1 = generate_key();
        let key2 = generate_key();
        
        assert_eq!(key1.len(), 32);
        assert_eq!(key2.len(), 32);
        assert_ne!(key1, key2); // Should be different random keys
    }
}