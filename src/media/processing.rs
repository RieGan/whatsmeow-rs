/// Media processing utilities for thumbnails, metadata extraction, and format conversion

use crate::{
    error::{Error, Result},
    media::MediaType,
};
use std::path::Path;
use serde::{Deserialize, Serialize};

/// Processed media information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessedMedia {
    /// Media type
    pub media_type: MediaType,
    /// File size in bytes
    pub file_size: u64,
    /// MIME type
    pub mime_type: String,
    /// Original filename
    pub filename: Option<String>,
    /// Thumbnail data (JPEG encoded)
    pub thumbnail: Option<Vec<u8>>,
    /// Duration in seconds (for audio/video)
    pub duration: Option<u32>,
    /// Width in pixels (for images/video)
    pub width: Option<u32>,
    /// Height in pixels (for images/video)
    pub height: Option<u32>,
    /// Bitrate (for audio/video)
    pub bitrate: Option<u32>,
    /// Frame rate (for video)
    pub fps: Option<f32>,
    /// Audio sample rate (for audio)
    pub sample_rate: Option<u32>,
    /// Number of audio channels
    pub channels: Option<u32>,
    /// Whether the media has transparency (for images)
    pub has_transparency: Option<bool>,
    /// Color depth in bits per pixel
    pub color_depth: Option<u32>,
}

impl ProcessedMedia {
    /// Create new processed media info
    pub fn new(media_type: MediaType, file_size: u64, mime_type: String) -> Self {
        Self {
            media_type,
            file_size,
            mime_type,
            filename: None,
            thumbnail: None,
            duration: None,
            width: None,
            height: None,
            bitrate: None,
            fps: None,
            sample_rate: None,
            channels: None,
            has_transparency: None,
            color_depth: None,
        }
    }
    
    /// Set filename
    pub fn with_filename(mut self, filename: String) -> Self {
        self.filename = Some(filename);
        self
    }
    
    /// Set dimensions
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }
    
    /// Set duration
    pub fn with_duration(mut self, duration: u32) -> Self {
        self.duration = Some(duration);
        self
    }
    
    /// Set thumbnail
    pub fn with_thumbnail(mut self, thumbnail: Vec<u8>) -> Self {
        self.thumbnail = Some(thumbnail);
        self
    }
    
    /// Set audio properties
    pub fn with_audio_properties(mut self, sample_rate: u32, channels: u32, bitrate: Option<u32>) -> Self {
        self.sample_rate = Some(sample_rate);
        self.channels = Some(channels);
        self.bitrate = bitrate;
        self
    }
    
    /// Set video properties
    pub fn with_video_properties(mut self, fps: f32, bitrate: Option<u32>) -> Self {
        self.fps = Some(fps);
        self.bitrate = bitrate;
        self
    }
    
    /// Set transparency information
    pub fn with_transparency(mut self, has_transparency: bool) -> Self {
        self.has_transparency = Some(has_transparency);
        self
    }
    
    /// Set color depth
    pub fn with_color_depth(mut self, color_depth: u32) -> Self {
        self.color_depth = Some(color_depth);
        self
    }
}

/// Media processor for handling different media types
pub struct MediaProcessor {
    /// Maximum thumbnail size (width x height)
    pub max_thumbnail_size: (u32, u32),
    /// Thumbnail quality (0-100)
    pub thumbnail_quality: u8,
    /// Enable advanced processing
    pub enable_advanced_processing: bool,
}

impl MediaProcessor {
    /// Create new media processor
    pub fn new() -> Self {
        Self {
            max_thumbnail_size: (320, 320),
            thumbnail_quality: 85,
            enable_advanced_processing: true,
        }
    }
    
    /// Create processor with custom settings
    pub fn with_settings(max_thumbnail_size: (u32, u32), thumbnail_quality: u8) -> Self {
        Self {
            max_thumbnail_size,
            thumbnail_quality,
            enable_advanced_processing: true,
        }
    }
    
    /// Process image file
    pub async fn process_image<P: AsRef<Path>>(&self, file_path: P) -> Result<ProcessedMedia> {
        let path = file_path.as_ref();
        let metadata = tokio::fs::metadata(path).await
            .map_err(|e| Error::from(e))?;
        
        let file_size = metadata.len();
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string());
        
        // Read file data for analysis
        let data = tokio::fs::read(path).await
            .map_err(|e| Error::from(e))?;
        
        // Detect image format and get basic info
        let (mime_type, width, height, has_transparency) = self.analyze_image_data(&data)?;
        
        // Generate thumbnail
        let thumbnail = if self.enable_advanced_processing {
            Some(self.generate_image_thumbnail(&data, mime_type.as_str()).await?)
        } else {
            None
        };
        
        let mut processed = ProcessedMedia::new(MediaType::Image, file_size, mime_type)
            .with_dimensions(width, height);
        
        if let Some(filename) = filename {
            processed = processed.with_filename(filename);
        }
        
        if let Some(thumbnail) = thumbnail {
            processed = processed.with_thumbnail(thumbnail);
        }
        
        processed = processed.with_transparency(has_transparency);
        
        Ok(processed)
    }
    
    /// Process video file
    pub async fn process_video<P: AsRef<Path>>(&self, file_path: P) -> Result<ProcessedMedia> {
        let path = file_path.as_ref();
        let metadata = tokio::fs::metadata(path).await
            .map_err(|e| Error::from(e))?;
        
        let file_size = metadata.len();
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string());
        
        // Read initial bytes for format detection
        let mut file = tokio::fs::File::open(path).await
            .map_err(|e| Error::from(e))?;
        
        let mut header = vec![0u8; 1024];
        use tokio::io::AsyncReadExt;
        let bytes_read = file.read(&mut header).await
            .map_err(|e| Error::from(e))?;
        header.truncate(bytes_read);
        
        // Detect video format
        let mime_type = self.detect_video_format(&header, path);
        
        // Extract video metadata (simplified implementation)
        let (width, height, duration, fps) = self.extract_video_metadata(path).await?;
        
        // Generate thumbnail from first frame
        let thumbnail = if self.enable_advanced_processing {
            self.generate_video_thumbnail(path).await.ok()
        } else {
            None
        };
        
        let mut processed = ProcessedMedia::new(MediaType::Video, file_size, mime_type)
            .with_dimensions(width, height)
            .with_duration(duration);
        
        if let Some(filename) = filename {
            processed = processed.with_filename(filename);
        }
        
        if let Some(thumbnail) = thumbnail {
            processed = processed.with_thumbnail(thumbnail);
        }
        
        if let Some(fps) = fps {
            processed = processed.with_video_properties(fps, None);
        }
        
        Ok(processed)
    }
    
    /// Process audio file
    pub async fn process_audio<P: AsRef<Path>>(&self, file_path: P) -> Result<ProcessedMedia> {
        let path = file_path.as_ref();
        let metadata = tokio::fs::metadata(path).await
            .map_err(|e| Error::from(e))?;
        
        let file_size = metadata.len();
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string());
        
        // Read initial bytes for format detection
        let mut file = tokio::fs::File::open(path).await
            .map_err(|e| Error::from(e))?;
        
        let mut header = vec![0u8; 512];
        use tokio::io::AsyncReadExt;
        let bytes_read = file.read(&mut header).await
            .map_err(|e| Error::from(e))?;
        header.truncate(bytes_read);
        
        // Detect audio format
        let mime_type = self.detect_audio_format(&header, path);
        
        // Extract audio metadata
        let (duration, sample_rate, channels, bitrate) = self.extract_audio_metadata(path).await?;
        
        let mut processed = ProcessedMedia::new(MediaType::Audio, file_size, mime_type)
            .with_duration(duration)
            .with_audio_properties(sample_rate, channels, bitrate);
        
        if let Some(filename) = filename {
            processed = processed.with_filename(filename);
        }
        
        Ok(processed)
    }
    
    /// Process document file
    pub async fn process_document<P: AsRef<Path>>(&self, file_path: P) -> Result<ProcessedMedia> {
        let path = file_path.as_ref();
        let metadata = tokio::fs::metadata(path).await
            .map_err(|e| Error::from(e))?;
        
        let file_size = metadata.len();
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string());
        
        // Detect document format
        let mime_type = self.detect_document_format(path);
        
        // Generate thumbnail for supported document types
        let thumbnail = if self.enable_advanced_processing {
            self.generate_document_thumbnail(path).await.ok()
        } else {
            None
        };
        
        let mut processed = ProcessedMedia::new(MediaType::Document, file_size, mime_type);
        
        if let Some(filename) = filename {
            processed = processed.with_filename(filename);
        }
        
        if let Some(thumbnail) = thumbnail {
            processed = processed.with_thumbnail(thumbnail);
        }
        
        Ok(processed)
    }
    
    /// Process static sticker
    pub async fn process_static_sticker<P: AsRef<Path>>(&self, file_path: P) -> Result<ProcessedMedia> {
        let mut processed = self.process_image(file_path).await?;
        processed.media_type = MediaType::Sticker;
        
        // Validate sticker requirements
        if let (Some(width), Some(height)) = (processed.width, processed.height) {
            if width != height {
                return Err(Error::Protocol("Sticker must be square".to_string()));
            }
            if width > 512 || height > 512 {
                return Err(Error::Protocol("Sticker size must not exceed 512x512".to_string()));
            }
        }
        
        Ok(processed)
    }
    
    /// Process animated sticker
    pub async fn process_animated_sticker<P: AsRef<Path>>(&self, file_path: P) -> Result<ProcessedMedia> {
        // For animated stickers, try video processing first
        let processed = if self.is_video_file(file_path.as_ref()) {
            let mut processed = self.process_video(file_path).await?;
            processed.media_type = MediaType::AnimatedSticker;
            processed
        } else {
            let mut processed = self.process_image(file_path).await?;
            processed.media_type = MediaType::AnimatedSticker;
            processed
        };
        
        // Validate animated sticker requirements
        if let (Some(width), Some(height)) = (processed.width, processed.height) {
            if width != height {
                return Err(Error::Protocol("Animated sticker must be square".to_string()));
            }
            if width > 512 || height > 512 {
                return Err(Error::Protocol("Animated sticker size must not exceed 512x512".to_string()));
            }
        }
        
        Ok(processed)
    }
    
    /// Analyze image data to extract basic information
    fn analyze_image_data(&self, data: &[u8]) -> Result<(String, u32, u32, bool)> {
        if data.is_empty() {
            return Err(Error::Protocol("Empty image data".to_string()));
        }
        
        // JPEG detection
        if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
            let (width, height) = self.extract_jpeg_dimensions(data)?;
            return Ok(("image/jpeg".to_string(), width, height, false));
        }
        
        // PNG detection
        if data.len() >= 8 && &data[0..8] == &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
            let (width, height, has_alpha) = self.extract_png_dimensions(data)?;
            return Ok(("image/png".to_string(), width, height, has_alpha));
        }
        
        // WebP detection
        if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
            let (width, height) = self.extract_webp_dimensions(data)?;
            return Ok(("image/webp".to_string(), width, height, false));
        }
        
        // GIF detection
        if data.len() >= 6 && (&data[0..6] == b"GIF87a" || &data[0..6] == b"GIF89a") {
            let (width, height) = self.extract_gif_dimensions(data)?;
            return Ok(("image/gif".to_string(), width, height, true));
        }
        
        Err(Error::Protocol("Unsupported image format".to_string()))
    }
    
    /// Extract JPEG dimensions
    fn extract_jpeg_dimensions(&self, data: &[u8]) -> Result<(u32, u32)> {
        // Simplified JPEG dimension extraction
        // Look for SOF (Start of Frame) markers
        let mut i = 2; // Skip SOI marker
        
        while i + 8 < data.len() {
            if data[i] == 0xFF {
                let marker = data[i + 1];
                // SOF markers (0xC0-0xCF, except 0xC4, 0xC8, 0xCC)
                if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC {
                    if i + 7 < data.len() {
                        let height = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                        let width = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                        return Ok((width, height));
                    }
                }
                
                // Skip to next marker
                if i + 2 < data.len() {
                    let length = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
                    i += 2 + length;
                } else {
                    break;
                }
            } else {
                i += 1;
            }
        }
        
        Err(Error::Protocol("Could not extract JPEG dimensions".to_string()))
    }
    
    /// Extract PNG dimensions
    fn extract_png_dimensions(&self, data: &[u8]) -> Result<(u32, u32, bool)> {
        if data.len() < 24 {
            return Err(Error::Protocol("PNG data too short".to_string()));
        }
        
        // IHDR chunk should be at bytes 8-20
        if &data[12..16] != b"IHDR" {
            return Err(Error::Protocol("Invalid PNG IHDR chunk".to_string()));
        }
        
        let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        
        // Check color type for transparency
        let color_type = data[25];
        let has_alpha = color_type == 4 || color_type == 6; // Gray+Alpha or RGBA
        
        Ok((width, height, has_alpha))
    }
    
    /// Extract WebP dimensions
    fn extract_webp_dimensions(&self, data: &[u8]) -> Result<(u32, u32)> {
        if data.len() < 30 {
            return Err(Error::Protocol("WebP data too short".to_string()));
        }
        
        // Check WebP variant
        if &data[12..16] == b"VP8 " {
            // Simple WebP (lossy)
            if data.len() < 30 {
                return Err(Error::Protocol("VP8 data too short".to_string()));
            }
            let width = (u16::from_le_bytes([data[26], data[27]]) & 0x3FFF) as u32;
            let height = (u16::from_le_bytes([data[28], data[29]]) & 0x3FFF) as u32;
            Ok((width, height))
        } else if &data[12..16] == b"VP8L" {
            // Lossless WebP
            if data.len() < 25 {
                return Err(Error::Protocol("VP8L data too short".to_string()));
            }
            let bits = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);
            let width = (bits & 0x3FFF) + 1;
            let height = ((bits >> 14) & 0x3FFF) + 1;
            Ok((width, height))
        } else {
            Err(Error::Protocol("Unsupported WebP format".to_string()))
        }
    }
    
    /// Extract GIF dimensions
    fn extract_gif_dimensions(&self, data: &[u8]) -> Result<(u32, u32)> {
        if data.len() < 10 {
            return Err(Error::Protocol("GIF data too short".to_string()));
        }
        
        let width = u16::from_le_bytes([data[6], data[7]]) as u32;
        let height = u16::from_le_bytes([data[8], data[9]]) as u32;
        
        Ok((width, height))
    }
    
    /// Generate thumbnail from image data
    async fn generate_image_thumbnail(&self, _data: &[u8], _mime_type: &str) -> Result<Vec<u8>> {
        // Simplified thumbnail generation
        // In a real implementation, you would use an image processing library
        // like image-rs to resize and convert to JPEG
        
        // For now, return a minimal JPEG thumbnail placeholder
        Ok(self.create_placeholder_thumbnail())
    }
    
    /// Generate thumbnail from video
    async fn generate_video_thumbnail(&self, _file_path: &Path) -> Result<Vec<u8>> {
        // Simplified video thumbnail generation
        // In a real implementation, you would use ffmpeg or similar
        // to extract a frame from the video
        
        Ok(self.create_placeholder_thumbnail())
    }
    
    /// Generate thumbnail from document
    async fn generate_document_thumbnail(&self, _file_path: &Path) -> Result<Vec<u8>> {
        // Simplified document thumbnail generation
        // In a real implementation, you would render the first page
        // of PDFs or office documents
        
        Ok(self.create_placeholder_thumbnail())
    }
    
    /// Create a placeholder thumbnail
    fn create_placeholder_thumbnail(&self) -> Vec<u8> {
        // Minimal JPEG header for a 1x1 pixel image
        vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
            0x01, 0x01, 0x00, 0x48, 0x00, 0x48, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43,
            0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09,
            0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12,
            0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
            0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29,
            0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32,
            0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x01,
            0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01,
            0xFF, 0xC4, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0xFF, 0xC4,
            0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xDA, 0x00, 0x0C,
            0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3F, 0x00, 0x8A, 0x00,
            0xFF, 0xD9,
        ]
    }
    
    /// Detect video format from header bytes
    fn detect_video_format(&self, header: &[u8], path: &Path) -> String {
        // Check file extension first
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "mp4" => return "video/mp4".to_string(),
                "3gp" => return "video/3gpp".to_string(),
                "mov" => return "video/quicktime".to_string(),
                "avi" => return "video/avi".to_string(),
                "mkv" => return "video/mkv".to_string(),
                _ => {}
            }
        }
        
        // Check magic bytes
        if header.len() >= 8 {
            // MP4/MOV (ftyp box)
            if &header[4..8] == b"ftyp" {
                return "video/mp4".to_string();
            }
            // AVI (RIFF...AVI )
            if &header[0..4] == b"RIFF" && header.len() >= 12 && &header[8..12] == b"AVI " {
                return "video/avi".to_string();
            }
        }
        
        "video/mp4".to_string() // Default
    }
    
    /// Detect audio format from header bytes
    fn detect_audio_format(&self, header: &[u8], path: &Path) -> String {
        // Check file extension first
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "mp3" => return "audio/mpeg".to_string(),
                "aac" => return "audio/aac".to_string(),
                "ogg" => return "audio/ogg".to_string(),
                "wav" => return "audio/wav".to_string(),
                "m4a" => return "audio/mp4".to_string(),
                _ => {}
            }
        }
        
        // Check magic bytes
        if header.len() >= 4 {
            // MP3 (ID3 or sync frame)
            if &header[0..3] == b"ID3" || (header[0] == 0xFF && (header[1] & 0xE0) == 0xE0) {
                return "audio/mpeg".to_string();
            }
            // OGG
            if &header[0..4] == b"OggS" {
                return "audio/ogg".to_string();
            }
            // WAV (RIFF...WAVE)
            if &header[0..4] == b"RIFF" && header.len() >= 12 && &header[8..12] == b"WAVE" {
                return "audio/wav".to_string();
            }
        }
        
        "audio/mpeg".to_string() // Default
    }
    
    /// Detect document format
    fn detect_document_format(&self, path: &Path) -> String {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "pdf" => "application/pdf".to_string(),
                "doc" => "application/msword".to_string(),
                "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
                "xls" => "application/vnd.ms-excel".to_string(),
                "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
                "txt" => "text/plain".to_string(),
                "zip" => "application/zip".to_string(),
                "json" => "application/json".to_string(),
                _ => "application/octet-stream".to_string(),
            }
        } else {
            "application/octet-stream".to_string()
        }
    }
    
    /// Check if file is a video file
    fn is_video_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            matches!(ext.to_lowercase().as_str(), "mp4" | "3gp" | "mov" | "avi" | "mkv" | "webm")
        } else {
            false
        }
    }
    
    /// Extract video metadata (simplified)
    async fn extract_video_metadata(&self, _path: &Path) -> Result<(u32, u32, u32, Option<f32>)> {
        // Simplified metadata extraction
        // In a real implementation, you would use ffprobe or similar
        Ok((1920, 1080, 60, Some(30.0))) // Default values
    }
    
    /// Extract audio metadata (simplified)
    async fn extract_audio_metadata(&self, _path: &Path) -> Result<(u32, u32, u32, Option<u32>)> {
        // Simplified metadata extraction
        // In a real implementation, you would use ffprobe or similar
        Ok((180, 44100, 2, Some(128))) // duration, sample_rate, channels, bitrate
    }
}

impl Default for MediaProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;
    
    #[test]
    fn test_processed_media_creation() {
        let processed = ProcessedMedia::new(MediaType::Image, 1024, "image/jpeg".to_string())
            .with_filename("test.jpg".to_string())
            .with_dimensions(800, 600)
            .with_transparency(false);
        
        assert_eq!(processed.media_type, MediaType::Image);
        assert_eq!(processed.file_size, 1024);
        assert_eq!(processed.mime_type, "image/jpeg");
        assert_eq!(processed.filename, Some("test.jpg".to_string()));
        assert_eq!(processed.width, Some(800));
        assert_eq!(processed.height, Some(600));
        assert_eq!(processed.has_transparency, Some(false));
    }
    
    #[test]
    fn test_media_processor_creation() {
        let processor = MediaProcessor::new();
        assert_eq!(processor.max_thumbnail_size, (320, 320));
        assert_eq!(processor.thumbnail_quality, 85);
        assert!(processor.enable_advanced_processing);
        
        let custom_processor = MediaProcessor::with_settings((256, 256), 90);
        assert_eq!(custom_processor.max_thumbnail_size, (256, 256));
        assert_eq!(custom_processor.thumbnail_quality, 90);
    }
    
    #[test]
    fn test_jpeg_dimension_extraction() {
        let processor = MediaProcessor::new();
        
        // Minimal JPEG with SOF0 marker
        let jpeg_data = vec![
            0xFF, 0xD8, // SOI
            0xFF, 0xC0, 0x00, 0x11, 0x08, // SOF0 marker + length + precision
            0x01, 0x00, // Height: 256
            0x01, 0x00, // Width: 256
            0x03, // Components
            0x01, 0x11, 0x00, // Component 1
            0x02, 0x11, 0x01, // Component 2
            0x03, 0x11, 0x01, // Component 3
        ];
        
        let result = processor.extract_jpeg_dimensions(&jpeg_data);
        assert!(result.is_ok());
        let (width, height) = result.unwrap();
        assert_eq!(width, 256);
        assert_eq!(height, 256);
    }
    
    #[test]
    fn test_png_dimension_extraction() {
        let processor = MediaProcessor::new();
        
        // PNG signature + IHDR chunk
        let png_data = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
            0x49, 0x48, 0x44, 0x52, // "IHDR"
            0x00, 0x00, 0x01, 0x00, // Width: 256
            0x00, 0x00, 0x01, 0x00, // Height: 256
            0x08, // Bit depth
            0x06, // Color type (RGBA)
            0x00, 0x00, 0x00, // Compression, filter, interlace
        ];
        
        let result = processor.extract_png_dimensions(&png_data);
        assert!(result.is_ok());
        let (width, height, has_alpha) = result.unwrap();
        assert_eq!(width, 256);
        assert_eq!(height, 256);
        assert!(has_alpha); // Color type 6 (RGBA) has alpha
    }
    
    #[test]
    fn test_image_format_detection() {
        let processor = MediaProcessor::new();
        
        // JPEG
        let jpeg_data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        assert!(processor.analyze_image_data(&jpeg_data).is_err()); // Too short for full analysis
        
        // PNG signature
        let png_data = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x08, 0x02, 0x00, 0x00, 0x00,
        ];
        let result = processor.analyze_image_data(&png_data);
        assert!(result.is_ok());
        let (mime_type, width, height, _) = result.unwrap();
        assert_eq!(mime_type, "image/png");
        assert_eq!(width, 256);
        assert_eq!(height, 256);
        
        // Unsupported format
        let unknown_data = vec![0x00, 0x01, 0x02, 0x03];
        assert!(processor.analyze_image_data(&unknown_data).is_err());
    }
    
    #[test]
    fn test_video_format_detection() {
        let processor = MediaProcessor::new();
        
        // MP4/MOV (ftyp box)
        let mp4_header = vec![0x00, 0x00, 0x00, 0x20, b'f', b't', b'y', b'p'];
        let mime_type = processor.detect_video_format(&mp4_header, Path::new("test.mp4"));
        assert_eq!(mime_type, "video/mp4");
        
        // AVI
        let avi_header = vec![
            b'R', b'I', b'F', b'F', 0x00, 0x00, 0x00, 0x00,
            b'A', b'V', b'I', b' ',
        ];
        let mime_type = processor.detect_video_format(&avi_header, Path::new("test.avi"));
        assert_eq!(mime_type, "video/avi");
        
        // Unknown format defaults to MP4
        let unknown_header = vec![0x00, 0x01, 0x02, 0x03];
        let mime_type = processor.detect_video_format(&unknown_header, Path::new("test.unknown"));
        assert_eq!(mime_type, "video/mp4");
    }
    
    #[test]
    fn test_audio_format_detection() {
        let processor = MediaProcessor::new();
        
        // MP3 (ID3 tag)
        let mp3_header = vec![b'I', b'D', b'3', 0x03, 0x00];
        let mime_type = processor.detect_audio_format(&mp3_header, Path::new("test.mp3"));
        assert_eq!(mime_type, "audio/mpeg");
        
        // OGG
        let ogg_header = vec![b'O', b'g', b'g', b'S'];
        let mime_type = processor.detect_audio_format(&ogg_header, Path::new("test.ogg"));
        assert_eq!(mime_type, "audio/ogg");
        
        // WAV
        let wav_header = vec![
            b'R', b'I', b'F', b'F', 0x00, 0x00, 0x00, 0x00,
            b'W', b'A', b'V', b'E',
        ];
        let mime_type = processor.detect_audio_format(&wav_header, Path::new("test.wav"));
        assert_eq!(mime_type, "audio/wav");
    }
    
    #[test]
    fn test_document_format_detection() {
        let processor = MediaProcessor::new();
        
        assert_eq!(processor.detect_document_format(Path::new("test.pdf")), "application/pdf");
        assert_eq!(processor.detect_document_format(Path::new("test.doc")), "application/msword");
        assert_eq!(processor.detect_document_format(Path::new("test.txt")), "text/plain");
        assert_eq!(processor.detect_document_format(Path::new("test.unknown")), "application/octet-stream");
    }
    
    #[test]
    fn test_placeholder_thumbnail() {
        let processor = MediaProcessor::new();
        let thumbnail = processor.create_placeholder_thumbnail();
        
        assert!(!thumbnail.is_empty());
        assert_eq!(thumbnail[0], 0xFF); // JPEG SOI marker
        assert_eq!(thumbnail[1], 0xD8);
    }
    
    #[tokio::test]
    async fn test_process_image_file() {
        let processor = MediaProcessor::new();
        
        // Create a temporary JPEG file with minimal header
        let mut temp_file = NamedTempFile::new().unwrap();
        let jpeg_data = vec![
            0xFF, 0xD8, // SOI
            0xFF, 0xC0, 0x00, 0x11, 0x08, // SOF0
            0x00, 0x64, 0x00, 0x64, // 100x100
            0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01,
            0xFF, 0xD9, // EOI
        ];
        temp_file.write_all(&jpeg_data).unwrap();
        
        let result = processor.process_image(temp_file.path()).await;
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        assert_eq!(processed.media_type, MediaType::Image);
        assert_eq!(processed.mime_type, "image/jpeg");
        assert!(processed.filename.is_some());
    }
    
    #[test]
    fn test_is_video_file() {
        let processor = MediaProcessor::new();
        
        assert!(processor.is_video_file(Path::new("test.mp4")));
        assert!(processor.is_video_file(Path::new("test.avi")));
        assert!(processor.is_video_file(Path::new("test.mov")));
        assert!(!processor.is_video_file(Path::new("test.jpg")));
        assert!(!processor.is_video_file(Path::new("test.txt")));
    }
}