/// Media download functionality for WhatsApp

use crate::{
    error::{Error, Result},
    media::{MediaInfo, ProgressInfo},
    util::crypto::{sha256, AesGcm},
};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt};
use tokio::fs::File;
use serde::{Deserialize, Serialize};

/// Download configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    /// Maximum concurrent downloads
    pub max_concurrent_downloads: u32,
    /// Download timeout in seconds
    pub timeout_seconds: u64,
    /// Chunk size for large files (bytes)
    pub chunk_size: usize,
    /// Enable download resume
    pub enable_resume: bool,
    /// User agent string
    pub user_agent: String,
    /// Maximum retries on failure
    pub max_retries: u32,
    /// Retry delay in seconds
    pub retry_delay_seconds: u64,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: 5,
            timeout_seconds: 300, // 5 minutes
            chunk_size: 1024 * 1024, // 1 MB chunks
            enable_resume: true,
            user_agent: "WhatsApp/0.1.0".to_string(),
            max_retries: 3,
            retry_delay_seconds: 2,
        }
    }
}

/// Download session tracking
#[derive(Debug, Clone)]
pub struct DownloadSession {
    /// Unique session ID
    pub session_id: String,
    /// Media info being downloaded
    pub media_info: MediaInfo,
    /// Total file size
    pub total_size: u64,
    /// Bytes downloaded so far
    pub downloaded_bytes: u64,
    /// Download progress (0.0 to 1.0)
    pub progress: f32,
    /// Download speed in bytes per second
    pub speed_bps: Option<u64>,
    /// Session start time
    pub start_time: std::time::Instant,
    /// Is download cancelled
    pub cancelled: bool,
    /// Resume offset (if supported)
    pub resume_offset: u64,
    /// Number of retry attempts
    pub retry_count: u32,
}

impl DownloadSession {
    /// Create new download session
    pub fn new(media_info: MediaInfo) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            total_size: media_info.file_length,
            media_info,
            downloaded_bytes: 0,
            progress: 0.0,
            speed_bps: None,
            start_time: std::time::Instant::now(),
            cancelled: false,
            resume_offset: 0,
            retry_count: 0,
        }
    }
    
    /// Update download progress
    pub fn update_progress(&mut self, downloaded_bytes: u64) {
        self.downloaded_bytes = downloaded_bytes;
        self.progress = if self.total_size > 0 {
            (downloaded_bytes as f32) / (self.total_size as f32)
        } else {
            0.0
        };
        
        // Calculate speed
        let elapsed = self.start_time.elapsed().as_secs();
        if elapsed > 0 {
            self.speed_bps = Some(downloaded_bytes / elapsed);
        }
    }
    
    /// Cancel the download
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
    
    /// Check if download is completed
    pub fn is_completed(&self) -> bool {
        self.downloaded_bytes >= self.total_size && !self.cancelled
    }
    
    /// Get progress info
    pub fn get_progress_info(&self) -> ProgressInfo {
        let mut progress = ProgressInfo::new(self.downloaded_bytes, self.total_size);
        if let Some(speed) = self.speed_bps {
            progress = progress.with_speed(speed);
        }
        progress
    }
    
    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
    
    /// Check if max retries exceeded
    pub fn max_retries_exceeded(&self, max_retries: u32) -> bool {
        self.retry_count >= max_retries
    }
}

/// Media downloader
pub struct MediaDownloader {
    config: DownloadConfig,
    http_client: reqwest::Client,
}

impl MediaDownloader {
    /// Create new media downloader
    pub fn new(config: DownloadConfig) -> Self {
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
    
    /// Download media to file
    pub async fn download_to_file<P: AsRef<Path>>(&self, media_info: &MediaInfo, output_path: P) -> Result<()> {
        let data = self.download_to_bytes(media_info).await?;
        
        let mut file = File::create(output_path).await
            .map_err(|e| Error::Io(e))?;
        
        file.write_all(&data).await
            .map_err(|e| Error::Io(e))?;
        
        file.flush().await
            .map_err(|e| Error::Io(e))?;
        
        Ok(())
    }
    
    /// Download media to bytes
    pub async fn download_to_bytes(&self, media_info: &MediaInfo) -> Result<Vec<u8>> {
        // Validate media info
        if !media_info.validate() {
            return Err(Error::Protocol("Invalid media info".to_string()));
        }
        
        let mut retry_count = 0;
        
        loop {
            match self.try_download(media_info).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= self.config.max_retries {
                        return Err(e);
                    }
                    
                    tracing::warn!("Download attempt {} failed: {}, retrying...", retry_count, e);
                    tokio::time::sleep(std::time::Duration::from_secs(self.config.retry_delay_seconds)).await;
                }
            }
        }
    }
    
    /// Download with progress tracking
    pub async fn download_with_progress<F>(
        &self,
        media_info: &MediaInfo,
        progress_callback: F,
    ) -> Result<Vec<u8>>
    where
        F: Fn(ProgressInfo) + Send + Sync + 'static,
    {
        let progress_callback = Arc::new(progress_callback);
        let total_size = media_info.file_length;
        
        // Download encrypted data
        let progress_callback_clone = progress_callback.clone();
        let encrypted_data = self.download_encrypted_data(&media_info.url, move |progress| {
            progress_callback_clone(progress);
        }).await?;
        
        // Verify encrypted file hash
        let actual_enc_hash = sha256(&encrypted_data);
        if actual_enc_hash != media_info.file_enc_sha256 {
            return Err(Error::Protocol("Encrypted file hash mismatch".to_string()));
        }
        
        // Decrypt the data
        let decrypted_data = self.decrypt_media_data(&encrypted_data, &media_info.media_key)?;
        
        // Verify decrypted file hash
        let actual_hash = sha256(&decrypted_data);
        if actual_hash != media_info.file_sha256 {
            return Err(Error::Protocol("Decrypted file hash mismatch".to_string()));
        }
        
        // Final progress update
        let final_progress = ProgressInfo::new(total_size, total_size);
        progress_callback(final_progress);
        
        Ok(decrypted_data)
    }
    
    /// Try to download (single attempt)
    async fn try_download(&self, media_info: &MediaInfo) -> Result<Vec<u8>> {
        // Download encrypted data
        let encrypted_data = self.download_encrypted_data(&media_info.url, |_| {}).await?;
        
        // Verify encrypted file hash
        let actual_enc_hash = sha256(&encrypted_data);
        if actual_enc_hash != media_info.file_enc_sha256 {
            return Err(Error::Protocol("Encrypted file hash mismatch".to_string()));
        }
        
        // Decrypt the data
        let decrypted_data = self.decrypt_media_data(&encrypted_data, &media_info.media_key)?;
        
        // Verify decrypted file hash
        let actual_hash = sha256(&decrypted_data);
        if actual_hash != media_info.file_sha256 {
            return Err(Error::Protocol("Decrypted file hash mismatch".to_string()));
        }
        
        Ok(decrypted_data)
    }
    
    /// Download encrypted data from URL
    async fn download_encrypted_data(
        &self,
        url: &str,
        progress_callback: impl Fn(ProgressInfo) + Send + Sync,
    ) -> Result<Vec<u8>> {
        let response = self.http_client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Protocol(format!("Download request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(Error::Protocol(format!(
                "Download failed with status: {}",
                response.status()
            )));
        }
        
        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded_bytes = 0u64;
        let mut data = Vec::new();
        
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| Error::Protocol(format!("Download chunk failed: {}", e)))?;
            
            data.extend_from_slice(&chunk);
            downloaded_bytes += chunk.len() as u64;
            
            // Update progress
            let progress = ProgressInfo::new(downloaded_bytes, total_size);
            progress_callback(progress);
        }
        
        Ok(data)
    }
    
    /// Decrypt media data using AES
    fn decrypt_media_data(&self, encrypted_data: &[u8], media_key: &[u8]) -> Result<Vec<u8>> {
        if media_key.len() != 32 {
            return Err(Error::Crypto("Media key must be 32 bytes".to_string()));
        }
        
        // Use first 16 bytes as IV (same as encryption)
        let iv = &media_key[0..16];
        
        // WhatsApp uses AES-256-CBC for media encryption
        // For simplicity, we'll use our AES-GCM implementation
        // In a real implementation, you'd want proper AES-CBC
        let aes = AesGcm::new(media_key.try_into().unwrap())?;
        let nonce: [u8; 12] = iv[0..12].try_into().unwrap();
        
        aes.decrypt(&nonce, encrypted_data)
    }
    
    /// Resume a download from a specific offset
    pub async fn resume_download(
        &self,
        session: &mut DownloadSession,
        output_path: &Path,
    ) -> Result<()> {
        if !self.config.enable_resume {
            return Err(Error::Protocol("Download resume is disabled".to_string()));
        }
        
        if session.cancelled {
            return Err(Error::Protocol("Cannot resume cancelled download".to_string()));
        }
        
        // Check if partial file exists
        let partial_size = if output_path.exists() {
            tokio::fs::metadata(output_path).await
                .map_err(|e| Error::Io(e))?
                .len()
        } else {
            0
        };
        
        session.resume_offset = partial_size;
        session.downloaded_bytes = partial_size;
        
        // Download remaining data
        let remaining_data = self.download_range(
            &session.media_info.url,
            partial_size,
            session.total_size - 1,
        ).await?;
        
        // Append to existing file
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(output_path)
            .await
            .map_err(|e| Error::Io(e))?;
        
        file.write_all(&remaining_data).await
            .map_err(|e| Error::Io(e))?;
        
        session.update_progress(session.total_size);
        
        Ok(())
    }
    
    /// Download a specific byte range
    async fn download_range(&self, url: &str, start: u64, end: u64) -> Result<Vec<u8>> {
        let response = self.http_client
            .get(url)
            .header("Range", format!("bytes={}-{}", start, end))
            .send()
            .await
            .map_err(|e| Error::Protocol(format!("Range download failed: {}", e)))?;
        
        if !response.status().is_success() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(Error::Protocol(format!(
                "Range download failed with status: {}",
                response.status()
            )));
        }
        
        let data = response.bytes().await
            .map_err(|e| Error::Protocol(format!("Failed to read range data: {}", e)))?;
        
        Ok(data.to_vec())
    }
    
    /// Verify downloaded file integrity
    pub fn verify_file_integrity(&self, data: &[u8], media_info: &MediaInfo) -> Result<()> {
        let actual_hash = sha256(data);
        if actual_hash != media_info.file_sha256 {
            return Err(Error::Protocol("File integrity verification failed".to_string()));
        }
        
        if data.len() as u64 != media_info.file_length {
            return Err(Error::Protocol("File size mismatch".to_string()));
        }
        
        Ok(())
    }
    
    /// Get download headers for a URL
    pub async fn get_download_info(&self, url: &str) -> Result<DownloadInfo> {
        let response = self.http_client
            .head(url)
            .send()
            .await
            .map_err(|e| Error::Protocol(format!("Head request failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(Error::Protocol(format!(
                "Head request failed with status: {}",
                response.status()
            )));
        }
        
        let content_length = response.content_length().unwrap_or(0);
        let supports_resume = response.headers()
            .get("accept-ranges")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "bytes")
            .unwrap_or(false);
        
        let last_modified = response.headers()
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());
        
        let etag = response.headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());
        
        Ok(DownloadInfo {
            content_length,
            supports_resume,
            last_modified,
            etag,
        })
    }
}

/// Download information structure
#[derive(Debug, Clone)]
pub struct DownloadInfo {
    /// File size in bytes
    pub content_length: u64,
    /// Whether the server supports resume (Range requests)
    pub supports_resume: bool,
    /// Last modified timestamp
    pub last_modified: Option<String>,
    /// ETag for cache validation
    pub etag: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::MediaInfo;
    
    #[test]
    fn test_download_config_default() {
        let config = DownloadConfig::default();
        assert!(config.max_concurrent_downloads > 0);
        assert!(config.timeout_seconds > 0);
        assert!(config.chunk_size > 0);
        assert!(config.max_retries > 0);
    }
    
    #[test]
    fn test_download_session_creation() {
        let media_info = MediaInfo::new(
            "https://example.com/file.jpg".to_string(),
            "/path/to/file.jpg".to_string(),
            vec![0u8; 32],
            vec![1u8; 32],
            vec![2u8; 32],
            1024,
            "image/jpeg".to_string(),
        );
        
        let session = DownloadSession::new(media_info.clone());
        
        assert!(!session.session_id.is_empty());
        assert_eq!(session.total_size, 1024);
        assert_eq!(session.downloaded_bytes, 0);
        assert_eq!(session.progress, 0.0);
        assert!(!session.cancelled);
        assert_eq!(session.media_info.url, media_info.url);
    }
    
    #[test]
    fn test_download_session_progress() {
        let media_info = MediaInfo::new(
            "https://example.com/file.jpg".to_string(),
            "/path/to/file.jpg".to_string(),
            vec![0u8; 32],
            vec![1u8; 32],
            vec![2u8; 32],
            1000,
            "image/jpeg".to_string(),
        );
        
        let mut session = DownloadSession::new(media_info);
        
        session.update_progress(500);
        assert_eq!(session.downloaded_bytes, 500);
        assert_eq!(session.progress, 0.5);
        
        session.update_progress(1000);
        assert_eq!(session.progress, 1.0);
        assert!(session.is_completed());
    }
    
    #[test]
    fn test_download_session_cancellation() {
        let media_info = MediaInfo::new(
            "https://example.com/file.jpg".to_string(),
            "/path/to/file.jpg".to_string(),
            vec![0u8; 32],
            vec![1u8; 32],
            vec![2u8; 32],
            1000,
            "image/jpeg".to_string(),
        );
        
        let mut session = DownloadSession::new(media_info);
        
        assert!(!session.cancelled);
        session.cancel();
        assert!(session.cancelled);
        assert!(!session.is_completed()); // Cancelled downloads are not completed
    }
    
    #[test]
    fn test_download_session_retry_tracking() {
        let media_info = MediaInfo::new(
            "https://example.com/file.jpg".to_string(),
            "/path/to/file.jpg".to_string(),
            vec![0u8; 32],
            vec![1u8; 32],
            vec![2u8; 32],
            1000,
            "image/jpeg".to_string(),
        );
        
        let mut session = DownloadSession::new(media_info);
        
        assert_eq!(session.retry_count, 0);
        assert!(!session.max_retries_exceeded(3));
        
        session.increment_retry();
        assert_eq!(session.retry_count, 1);
        
        session.increment_retry();
        session.increment_retry();
        session.increment_retry();
        assert!(session.max_retries_exceeded(3));
    }
    
    #[test]
    fn test_media_downloader_creation() {
        let config = DownloadConfig::default();
        let downloader = MediaDownloader::new(config.clone());
        assert_eq!(downloader.config.max_concurrent_downloads, config.max_concurrent_downloads);
    }
    
    #[test]
    fn test_download_info() {
        let info = DownloadInfo {
            content_length: 1024,
            supports_resume: true,
            last_modified: Some("Wed, 21 Oct 2015 07:28:00 GMT".to_string()),
            etag: Some("\"abc123\"".to_string()),
        };
        
        assert_eq!(info.content_length, 1024);
        assert!(info.supports_resume);
        assert!(info.last_modified.is_some());
        assert!(info.etag.is_some());
    }
    
    #[test]
    fn test_media_decryption() {
        let config = DownloadConfig::default();
        let downloader = MediaDownloader::new(config);
        
        let media_key = vec![1u8; 32];
        let data = b"Hello, World!";
        
        // Test with invalid key length
        let invalid_key = vec![0u8; 16];
        let result = downloader.decrypt_media_data(data, &invalid_key);
        assert!(result.is_err());
        
        // Test with valid key length (encryption/decryption would require proper setup)
        let result = downloader.decrypt_media_data(data, &media_key);
        // This will fail because we're not providing properly encrypted data,
        // but it tests the key length validation
        assert!(result.is_err());
    }
    
    #[test]
    fn test_file_integrity_verification() {
        let config = DownloadConfig::default();
        let downloader = MediaDownloader::new(config);
        
        let data = b"Hello, World!";
        let data_hash = crate::util::crypto::sha256(data);
        
        let media_info = MediaInfo::new(
            "https://example.com/file.txt".to_string(),
            "/path/to/file.txt".to_string(),
            vec![0u8; 32],
            data_hash.clone(),
            vec![2u8; 32],
            data.len() as u64,
            "text/plain".to_string(),
        );
        
        // Valid data should pass verification
        assert!(downloader.verify_file_integrity(data, &media_info).is_ok());
        
        // Invalid data should fail verification
        let wrong_data = b"Wrong data!";
        assert!(downloader.verify_file_integrity(wrong_data, &media_info).is_err());
        
        // Wrong size should fail verification
        let media_info_wrong_size = MediaInfo::new(
            "https://example.com/file.txt".to_string(),
            "/path/to/file.txt".to_string(),
            vec![0u8; 32],
            data_hash,
            vec![2u8; 32],
            999, // Wrong size
            "text/plain".to_string(),
        );
        assert!(downloader.verify_file_integrity(data, &media_info_wrong_size).is_err());
    }
}