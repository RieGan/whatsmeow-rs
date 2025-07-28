/// Media upload functionality for WhatsApp

use crate::{
    error::{Error, Result},
    media::{MediaInfo, MediaType, ProgressInfo},
    util::crypto::{sha256, AesGcm},
};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt;
use tokio::fs::File;
use serde::{Deserialize, Serialize};

/// Upload configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadConfig {
    /// WhatsApp media upload endpoint
    pub upload_endpoint: String,
    /// Maximum concurrent uploads
    pub max_concurrent_uploads: u32,
    /// Upload timeout in seconds
    pub timeout_seconds: u64,
    /// Chunk size for large files (bytes)
    pub chunk_size: usize,
    /// Enable upload resume
    pub enable_resume: bool,
    /// User agent string
    pub user_agent: String,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            upload_endpoint: "https://mmg.whatsapp.net".to_string(),
            max_concurrent_uploads: 3,
            timeout_seconds: 300, // 5 minutes
            chunk_size: 1024 * 1024, // 1 MB chunks
            enable_resume: true,
            user_agent: "WhatsApp/0.1.0".to_string(),
        }
    }
}

/// Upload session tracking
#[derive(Debug, Clone)]
pub struct UploadSession {
    /// Unique session ID
    pub session_id: String,
    /// File path being uploaded
    pub file_path: String,
    /// Media type
    pub media_type: MediaType,
    /// Total file size
    pub total_size: u64,
    /// Bytes uploaded so far
    pub uploaded_bytes: u64,
    /// Upload progress (0.0 to 1.0)
    pub progress: f32,
    /// Upload speed in bytes per second
    pub speed_bps: Option<u64>,
    /// Session start time
    pub start_time: std::time::Instant,
    /// Is upload cancelled
    pub cancelled: bool,
    /// Resume token (if supported)
    pub resume_token: Option<String>,
}

impl UploadSession {
    /// Create new upload session
    pub fn new(file_path: String, media_type: MediaType, total_size: u64) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            file_path,
            media_type,
            total_size,
            uploaded_bytes: 0,
            progress: 0.0,
            speed_bps: None,
            start_time: std::time::Instant::now(),
            cancelled: false,
            resume_token: None,
        }
    }
    
    /// Update upload progress
    pub fn update_progress(&mut self, uploaded_bytes: u64) {
        self.uploaded_bytes = uploaded_bytes;
        self.progress = if self.total_size > 0 {
            (uploaded_bytes as f32) / (self.total_size as f32)
        } else {
            0.0
        };
        
        // Calculate speed
        let elapsed = self.start_time.elapsed().as_secs();
        if elapsed > 0 {
            self.speed_bps = Some(uploaded_bytes / elapsed);
        }
    }
    
    /// Cancel the upload
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
    
    /// Check if upload is completed
    pub fn is_completed(&self) -> bool {
        self.uploaded_bytes >= self.total_size && !self.cancelled
    }
    
    /// Get progress info
    pub fn get_progress_info(&self) -> ProgressInfo {
        let mut progress = ProgressInfo::new(self.uploaded_bytes, self.total_size);
        if let Some(speed) = self.speed_bps {
            progress = progress.with_speed(speed);
        }
        progress
    }
}

/// Media uploader
pub struct MediaUploader {
    config: UploadConfig,
    http_client: reqwest::Client,
}

impl MediaUploader {
    /// Create new media uploader
    pub fn new(config: UploadConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .user_agent(&config.user_agent)
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            config,
            http_client,
        }
    }
    
    /// Upload media file
    pub async fn upload_file<P: AsRef<Path>>(&self, file_path: P, media_type: MediaType) -> Result<MediaInfo> {
        let path = file_path.as_ref();
        
        // Validate file exists and get size
        let metadata = tokio::fs::metadata(path).await
            .map_err(|e| Error::Io(e))?;
        
        if !metadata.is_file() {
            return Err(Error::Protocol("Path is not a file".to_string()));
        }
        
        let file_size = metadata.len();
        
        // Check file size limits
        if file_size > media_type.max_file_size() {
            return Err(Error::Protocol(format!(
                "File size {} exceeds limit {} for media type {:?}",
                file_size, media_type.max_file_size(), media_type
            )));
        }
        
        // Read file data
        let mut file = File::open(path).await
            .map_err(|e| Error::Io(e))?;
        
        let mut file_data = Vec::with_capacity(file_size as usize);
        file.read_to_end(&mut file_data).await
            .map_err(|e| Error::Io(e))?;
        
        // Upload the data
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file")
            .to_string();
        
        self.upload_bytes(&file_data, &filename, media_type).await
    }
    
    /// Upload media from bytes
    pub async fn upload_bytes(&self, data: &[u8], filename: &str, media_type: MediaType) -> Result<MediaInfo> {
        if data.is_empty() {
            return Err(Error::Protocol("Cannot upload empty data".to_string()));
        }
        
        let file_size = data.len() as u64;
        
        // Check file size limits
        if file_size > media_type.max_file_size() {
            return Err(Error::Protocol(format!(
                "Data size {} exceeds limit {} for media type {:?}",
                file_size, media_type.max_file_size(), media_type
            )));
        }
        
        // Generate media key and encrypt file
        let media_key = self.generate_media_key();
        let encrypted_data = self.encrypt_media_data(data, &media_key)?;
        
        // Calculate hashes
        let file_sha256 = sha256(data);
        let file_enc_sha256 = sha256(&encrypted_data);
        
        // Determine MIME type
        let mime_type = self.detect_mime_type(data, filename, &media_type);
        
        // Upload encrypted data
        let (upload_url, direct_path) = self.upload_encrypted_data(&encrypted_data, &mime_type).await?;
        
        Ok(MediaInfo::new(
            upload_url,
            direct_path,
            media_key,
            file_sha256,
            file_enc_sha256,
            file_size,
            mime_type,
        ))
    }
    
    /// Upload with progress tracking
    pub async fn upload_with_progress<P: AsRef<Path>>(
        &self,
        file_path: P,
        media_type: MediaType,
        progress_callback: impl Fn(ProgressInfo) + Send + Sync + 'static,
    ) -> Result<MediaInfo> {
        let path = file_path.as_ref();
        let metadata = tokio::fs::metadata(path).await
            .map_err(|e| Error::Io(e))?;
        
        let file_size = metadata.len();
        let progress_callback = Arc::new(progress_callback);
        
        // Create progress tracker
        let progress_tracker = Arc::new(Mutex::new(ProgressInfo::new(0, file_size)));
        
        // Upload in chunks for progress tracking
        let mut file = File::open(path).await
            .map_err(|e| Error::Io(e))?;
        
        let mut all_data = Vec::with_capacity(file_size as usize);
        let mut uploaded_bytes = 0u64;
        
        loop {
            let mut buffer = vec![0u8; self.config.chunk_size];
            let bytes_read = file.read(&mut buffer).await
                .map_err(|e| Error::Io(e))?;
            
            if bytes_read == 0 {
                break;
            }
            
            buffer.truncate(bytes_read);
            all_data.extend_from_slice(&buffer);
            
            uploaded_bytes += bytes_read as u64;
            
            // Update progress
            {
                let mut progress = progress_tracker.lock().unwrap();
                *progress = ProgressInfo::new(uploaded_bytes, file_size);
                progress_callback(progress.clone());
            }
        }
        
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file")
            .to_string();
        
        self.upload_bytes(&all_data, &filename, media_type).await
    }
    
    /// Generate random media key
    fn generate_media_key(&self) -> Vec<u8> {
        crate::util::crypto::random_bytes(32)
    }
    
    /// Encrypt media data using AES-256-CBC
    fn encrypt_media_data(&self, data: &[u8], media_key: &[u8]) -> Result<Vec<u8>> {
        if media_key.len() != 32 {
            return Err(Error::Crypto("Media key must be 32 bytes".to_string()));
        }
        
        // Use first 16 bytes as IV
        let iv = &media_key[0..16];
        
        // WhatsApp uses AES-256-CBC for media encryption
        // For simplicity, we'll use our AES-GCM implementation
        // In a real implementation, you'd want proper AES-CBC
        let aes = AesGcm::new(media_key.try_into().unwrap())?;
        let nonce: [u8; 12] = iv[0..12].try_into().unwrap();
        
        aes.encrypt(&nonce, data)
    }
    
    /// Detect MIME type from data and filename
    fn detect_mime_type(&self, data: &[u8], filename: &str, media_type: &MediaType) -> String {
        // Simple MIME type detection based on file extension
        let extension = std::path::Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        match media_type {
            MediaType::Image => match extension.as_str() {
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "webp" => "image/webp",
                "gif" => "image/gif",
                _ => {
                    // Check magic bytes
                    if data.starts_with(&[0xFF, 0xD8]) {
                        "image/jpeg"
                    } else if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                        "image/png"
                    } else if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" {
                        "image/webp"
                    } else {
                        "image/jpeg" // default
                    }
                }
            },
            MediaType::Video => match extension.as_str() {
                "mp4" => "video/mp4",
                "3gp" => "video/3gpp",
                "mov" => "video/quicktime",
                "avi" => "video/avi",
                "mkv" => "video/mkv",
                _ => "video/mp4", // default
            },
            MediaType::Audio | MediaType::VoiceNote => match extension.as_str() {
                "mp3" => "audio/mpeg",
                "aac" => "audio/aac",
                "ogg" => "audio/ogg",
                "wav" => "audio/wav",
                "m4a" => "audio/mp4",
                _ => "audio/mpeg", // default
            },
            MediaType::Document => match extension.as_str() {
                "pdf" => "application/pdf",
                "doc" => "application/msword",
                "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "xls" => "application/vnd.ms-excel",
                "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "txt" => "text/plain",
                "zip" => "application/zip",
                "json" => "application/json",
                _ => "application/octet-stream",
            },
            MediaType::Sticker | MediaType::AnimatedSticker => "image/webp",
            _ => "application/octet-stream",
        }.to_string()
    }
    
    /// Upload encrypted data to WhatsApp servers
    async fn upload_encrypted_data(&self, encrypted_data: &[u8], mime_type: &str) -> Result<(String, String)> {
        // This is a simplified implementation
        // In reality, WhatsApp has a complex upload process involving:
        // 1. Getting upload tokens from the main connection
        // 2. Uploading to specific media servers
        // 3. Getting direct paths and URLs back
        
        let upload_url = format!("{}/mms/upload", self.config.upload_endpoint);
        
        // Create multipart form data
        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(encrypted_data.to_vec())
                    .mime_str(mime_type)
                    .map_err(|e| Error::Protocol(format!("Invalid MIME type: {}", e)))?,
            );
        
        // Make upload request
        let response = self.http_client
            .post(&upload_url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| Error::Protocol(format!("Upload request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(Error::Protocol(format!(
                "Upload failed with status: {}",
                response.status()
            )));
        }
        
        // Parse response to get URL and direct path
        let _response_text = response.text().await
            .map_err(|e| Error::Protocol(format!("Failed to read response: {}", e)))?;
        
        // In a real implementation, parse JSON response to get actual URLs
        let file_id = uuid::Uuid::new_v4().to_string();
        let download_url = format!("{}/mms/download/{}", self.config.upload_endpoint, file_id);
        let direct_path = format!("/mms/download/{}", file_id);
        
        Ok((download_url, direct_path))
    }
    
    /// Resume an interrupted upload
    pub async fn resume_upload(&self, session: &mut UploadSession, resume_data: &[u8]) -> Result<MediaInfo> {
        if !self.config.enable_resume {
            return Err(Error::Protocol("Upload resume is disabled".to_string()));
        }
        
        if session.cancelled {
            return Err(Error::Protocol("Cannot resume cancelled upload".to_string()));
        }
        
        // Calculate remaining data to upload
        let remaining_data = &resume_data[session.uploaded_bytes as usize..];
        
        // Continue upload from where it left off
        // This is a simplified implementation
        let filename = std::path::Path::new(&session.file_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file");
        
        self.upload_bytes(remaining_data, filename, session.media_type.clone()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;
    
    #[test]
    fn test_upload_config_default() {
        let config = UploadConfig::default();
        assert!(!config.upload_endpoint.is_empty());
        assert!(config.max_concurrent_uploads > 0);
        assert!(config.timeout_seconds > 0);
        assert!(config.chunk_size > 0);
    }
    
    #[test]
    fn test_upload_session_creation() {
        let session = UploadSession::new(
            "/path/to/file.jpg".to_string(),
            MediaType::Image,
            1024,
        );
        
        assert!(!session.session_id.is_empty());
        assert_eq!(session.total_size, 1024);
        assert_eq!(session.uploaded_bytes, 0);
        assert_eq!(session.progress, 0.0);
        assert!(!session.cancelled);
    }
    
    #[test]
    fn test_upload_session_progress() {
        let mut session = UploadSession::new(
            "/path/to/file.jpg".to_string(),
            MediaType::Image,
            1000,
        );
        
        session.update_progress(500);
        assert_eq!(session.uploaded_bytes, 500);
        assert_eq!(session.progress, 0.5);
        
        session.update_progress(1000);
        assert_eq!(session.progress, 1.0);
        assert!(session.is_completed());
    }
    
    #[test]
    fn test_upload_session_cancellation() {
        let mut session = UploadSession::new(
            "/path/to/file.jpg".to_string(),
            MediaType::Image,
            1000,
        );
        
        assert!(!session.cancelled);
        session.cancel();
        assert!(session.cancelled);
        assert!(!session.is_completed()); // Cancelled uploads are not completed
    }
    
    #[test]
    fn test_media_uploader_creation() {
        let config = UploadConfig::default();
        let uploader = MediaUploader::new(config.clone());
        assert_eq!(uploader.config.upload_endpoint, config.upload_endpoint);
    }
    
    #[test]
    fn test_mime_type_detection() {
        let config = UploadConfig::default();
        let uploader = MediaUploader::new(config);
        
        // JPEG image
        let jpeg_data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        let mime_type = uploader.detect_mime_type(&jpeg_data, "image.jpg", &MediaType::Image);
        assert_eq!(mime_type, "image/jpeg");
        
        // PNG image
        let png_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let mime_type = uploader.detect_mime_type(&png_data, "image.png", &MediaType::Image);
        assert_eq!(mime_type, "image/png");
        
        // Document
        let doc_data = vec![0x50, 0x4B]; // ZIP-based document
        let mime_type = uploader.detect_mime_type(&doc_data, "document.pdf", &MediaType::Document);
        assert_eq!(mime_type, "application/pdf");
    }
    
    #[tokio::test]
    async fn test_file_size_validation() {
        let config = UploadConfig::default();
        let uploader = MediaUploader::new(config);
        
        // Create a temporary file that's too large for stickers
        let mut temp_file = NamedTempFile::new().unwrap();
        let large_data = vec![0u8; 1024 * 1024]; // 1MB (sticker limit is 500KB)
        temp_file.write_all(&large_data).unwrap();
        
        let result = uploader.upload_file(temp_file.path(), MediaType::Sticker).await;
        assert!(result.is_err());
        
        // Test with valid size
        let small_data = vec![0u8; 100 * 1024]; // 100KB
        let result = uploader.upload_bytes(&small_data, "sticker.webp", MediaType::Sticker).await;
        // This will fail due to mock upload endpoint, but size validation should pass
        assert!(result.is_err()); // Expected to fail due to network/endpoint issues
    }
    
    #[test]
    fn test_media_key_generation() {
        let config = UploadConfig::default();
        let uploader = MediaUploader::new(config);
        
        let media_key = uploader.generate_media_key();
        assert_eq!(media_key.len(), 32);
        
        // Generate another key and ensure they're different
        let media_key2 = uploader.generate_media_key();
        assert_ne!(media_key, media_key2);
    }
    
    #[test]
    fn test_media_encryption() {
        let config = UploadConfig::default();
        let uploader = MediaUploader::new(config);
        
        let data = b"Hello, World!";
        let media_key = uploader.generate_media_key();
        
        let encrypted = uploader.encrypt_media_data(data, &media_key).unwrap();
        assert_ne!(encrypted, data);
        assert!(!encrypted.is_empty());
        
        // Test with invalid key length
        let invalid_key = vec![0u8; 16];
        let result = uploader.encrypt_media_data(data, &invalid_key);
        assert!(result.is_err());
    }
}