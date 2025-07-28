/// Media handling for WhatsApp messages - images, videos, audio, documents

pub mod types;
pub mod upload;
pub mod download;
pub mod processing;
pub mod encryption;

use crate::{
    error::{Error, Result},
};
use std::path::Path;
use std::collections::HashMap;

pub use types::*;
pub use upload::*;
pub use download::*;
pub use processing::*;
pub use encryption::*;

/// Media manager for handling all media operations
pub struct MediaManager {
    /// Upload configuration
    upload_config: UploadConfig,
    /// Download configuration  
    download_config: DownloadConfig,
    /// Active upload sessions
    active_uploads: HashMap<String, UploadSession>,
    /// Active download sessions
    active_downloads: HashMap<String, DownloadSession>,
    /// Media cache directory
    cache_directory: Option<String>,
}

impl MediaManager {
    /// Create a new media manager
    pub fn new() -> Self {
        Self {
            upload_config: UploadConfig::default(),
            download_config: DownloadConfig::default(),
            active_uploads: HashMap::new(),
            active_downloads: HashMap::new(),
            cache_directory: None,
        }
    }
    
    /// Create media manager with custom cache directory
    pub fn with_cache_dir<P: AsRef<Path>>(cache_dir: P) -> Self {
        Self {
            upload_config: UploadConfig::default(),
            download_config: DownloadConfig::default(),
            active_uploads: HashMap::new(),
            active_downloads: HashMap::new(),
            cache_directory: Some(cache_dir.as_ref().to_string_lossy().to_string()),
        }
    }
    
    /// Set upload configuration
    pub fn set_upload_config(&mut self, config: UploadConfig) {
        self.upload_config = config;
    }
    
    /// Set download configuration
    pub fn set_download_config(&mut self, config: DownloadConfig) {
        self.download_config = config;
    }
    
    /// Upload media file and get media info for message
    pub async fn upload_media<P: AsRef<Path>>(&mut self, file_path: P, media_type: MediaType) -> Result<MediaInfo> {
        let uploader = MediaUploader::new(self.upload_config.clone());
        let media_info = uploader.upload_file(file_path, media_type).await?;
        Ok(media_info)
    }
    
    /// Upload media from bytes
    pub async fn upload_media_bytes(&mut self, data: &[u8], filename: &str, media_type: MediaType) -> Result<MediaInfo> {
        let uploader = MediaUploader::new(self.upload_config.clone());
        let media_info = uploader.upload_bytes(data, filename, media_type).await?;
        Ok(media_info)
    }
    
    /// Download media to file
    pub async fn download_media<P: AsRef<Path>>(&mut self, media_info: &MediaInfo, output_path: P) -> Result<()> {
        let downloader = MediaDownloader::new(self.download_config.clone());
        downloader.download_to_file(media_info, output_path).await?;
        Ok(())
    }
    
    /// Download media to bytes
    pub async fn download_media_bytes(&mut self, media_info: &MediaInfo) -> Result<Vec<u8>> {
        let downloader = MediaDownloader::new(self.download_config.clone());
        let data = downloader.download_to_bytes(media_info).await?;
        Ok(data)
    }
    
    /// Create image message
    pub async fn create_image_message<P: AsRef<Path>>(&mut self, file_path: P, caption: Option<String>) -> Result<MediaMessage> {
        // Process image to generate thumbnail
        let processor = MediaProcessor::new();
        let processed = processor.process_image(file_path.as_ref()).await?;
        
        // Upload original image
        let media_info = self.upload_media(file_path, MediaType::Image).await?;
        
        Ok(MediaMessage {
            media_type: MediaType::Image,
            media_info,
            caption,
            thumbnail: processed.thumbnail,
            duration: None,
            width: processed.width,
            height: processed.height,
            file_size: processed.file_size,
            mime_type: processed.mime_type,
            filename: processed.filename,
        })
    }
    
    /// Create video message
    pub async fn create_video_message<P: AsRef<Path>>(&mut self, file_path: P, caption: Option<String>) -> Result<MediaMessage> {
        let processor = MediaProcessor::new();
        let processed = processor.process_video(file_path.as_ref()).await?;
        
        let media_info = self.upload_media(file_path, MediaType::Video).await?;
        
        Ok(MediaMessage {
            media_type: MediaType::Video,
            media_info,
            caption,
            thumbnail: processed.thumbnail,
            duration: processed.duration,
            width: processed.width,
            height: processed.height,
            file_size: processed.file_size,
            mime_type: processed.mime_type,
            filename: processed.filename,
        })
    }
    
    /// Create audio message
    pub async fn create_audio_message<P: AsRef<Path>>(&mut self, file_path: P, is_voice_note: bool) -> Result<MediaMessage> {
        let processor = MediaProcessor::new();
        let processed = processor.process_audio(file_path.as_ref()).await?;
        
        let media_type = if is_voice_note { 
            MediaType::VoiceNote 
        } else { 
            MediaType::Audio 
        };
        
        let media_info = self.upload_media(file_path, media_type.clone()).await?;
        
        Ok(MediaMessage {
            media_type,
            media_info,
            caption: None,
            thumbnail: None,
            duration: processed.duration,
            width: None,
            height: None,
            file_size: processed.file_size,
            mime_type: processed.mime_type,
            filename: processed.filename,
        })
    }
    
    /// Create document message
    pub async fn create_document_message<P: AsRef<Path>>(&mut self, file_path: P, title: Option<String>) -> Result<MediaMessage> {
        let processor = MediaProcessor::new();
        let processed = processor.process_document(file_path.as_ref()).await?;
        
        let media_info = self.upload_media(file_path, MediaType::Document).await?;
        
        Ok(MediaMessage {
            media_type: MediaType::Document,
            media_info,
            caption: title,
            thumbnail: processed.thumbnail,
            duration: None,
            width: None,
            height: None,
            file_size: processed.file_size,
            mime_type: processed.mime_type,
            filename: processed.filename,
        })
    }
    
    /// Create sticker message
    pub async fn create_sticker_message<P: AsRef<Path>>(&mut self, file_path: P, animated: bool) -> Result<MediaMessage> {
        let processor = MediaProcessor::new();
        let processed = if animated {
            processor.process_animated_sticker(file_path.as_ref()).await?
        } else {
            processor.process_static_sticker(file_path.as_ref()).await?
        };
        
        let media_type = if animated { 
            MediaType::AnimatedSticker 
        } else { 
            MediaType::Sticker 
        };
        
        let media_info = self.upload_media(file_path, media_type.clone()).await?;
        
        Ok(MediaMessage {
            media_type,
            media_info,
            caption: None,
            thumbnail: None,
            duration: if animated { processed.duration } else { None },
            width: processed.width,
            height: processed.height,
            file_size: processed.file_size,
            mime_type: processed.mime_type,
            filename: processed.filename,
        })
    }
    
    /// Get upload progress for a session
    pub fn get_upload_progress(&self, session_id: &str) -> Option<f32> {
        self.active_uploads.get(session_id).map(|session| session.progress)
    }
    
    /// Get download progress for a session
    pub fn get_download_progress(&self, session_id: &str) -> Option<f32> {
        self.active_downloads.get(session_id).map(|session| session.progress)
    }
    
    /// Cancel upload session
    pub fn cancel_upload(&mut self, session_id: &str) -> Result<()> {
        if let Some(mut session) = self.active_uploads.remove(session_id) {
            session.cancel();
            Ok(())
        } else {
            Err(Error::Protocol("Upload session not found".to_string()))
        }
    }
    
    /// Cancel download session
    pub fn cancel_download(&mut self, session_id: &str) -> Result<()> {
        if let Some(mut session) = self.active_downloads.remove(session_id) {
            session.cancel();
            Ok(())
        } else {
            Err(Error::Protocol("Download session not found".to_string()))
        }
    }
    
    /// Clear cache directory
    pub async fn clear_cache(&self) -> Result<()> {
        if let Some(cache_dir) = &self.cache_directory {
            tokio::fs::remove_dir_all(cache_dir).await
                .map_err(|e| Error::Io(e))?;
            tokio::fs::create_dir_all(cache_dir).await
                .map_err(|e| Error::Io(e))?;
        }
        Ok(())
    }
    
    /// Get cache size in bytes
    pub async fn get_cache_size(&self) -> Result<u64> {
        if let Some(cache_dir) = &self.cache_directory {
            calculate_directory_size(cache_dir).await
        } else {
            Ok(0)
        }
    }
}

impl Default for MediaManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate directory size recursively
fn calculate_directory_size(dir_path: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64>> + Send + '_>> {
    Box::pin(async move {
        let mut total_size = 0u64;
        
        let mut entries = tokio::fs::read_dir(dir_path).await
            .map_err(|e| Error::Io(e))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| Error::Io(e))? {
            
            let metadata = entry.metadata().await
                .map_err(|e| Error::Io(e))?;
            
            if metadata.is_file() {
                total_size += metadata.len();
            } else if metadata.is_dir() {
                let sub_path = entry.path().to_string_lossy().to_string();
                total_size += calculate_directory_size(&sub_path).await?;
            }
        }
        
        Ok(total_size)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_media_manager_creation() {
        let manager = MediaManager::new();
        assert!(manager.cache_directory.is_none());
        assert!(manager.active_uploads.is_empty());
        assert!(manager.active_downloads.is_empty());
    }
    
    #[tokio::test]
    async fn test_media_manager_with_cache() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path();
        
        let manager = MediaManager::with_cache_dir(cache_path);
        assert!(manager.cache_directory.is_some());
        assert!(manager.cache_directory.unwrap().contains(&cache_path.to_string_lossy().to_string()));
    }
    
    #[tokio::test]
    async fn test_cache_operations() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path();
        
        let manager = MediaManager::with_cache_dir(cache_path);
        
        // Initially cache should be empty
        let size = manager.get_cache_size().await.unwrap();
        assert_eq!(size, 0);
        
        // Clear cache should work even when empty
        manager.clear_cache().await.unwrap();
    }
}