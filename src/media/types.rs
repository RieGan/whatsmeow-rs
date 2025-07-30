/// Media types and structures for WhatsApp media messages

use serde::{Deserialize, Serialize};

/// Type of media content
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    /// Auto-detect from file extension/MIME type
    Auto,
    /// Static image (JPEG, PNG, WebP)
    Image,
    /// Video file (MP4, AVI, etc.)
    Video,
    /// Audio file (MP3, AAC, etc.)
    Audio,
    /// Voice note (PTT - Push to Talk)
    VoiceNote,
    /// Document/file
    Document,
    /// Static sticker
    Sticker,
    /// Animated sticker
    AnimatedSticker,
    /// Location message
    Location,
    /// Contact card
    Contact,
}

impl MediaType {
    /// Get expected MIME types for this media type
    pub fn expected_mime_types(&self) -> Vec<&'static str> {
        match self {
            MediaType::Auto => vec![], // Auto detection doesn't restrict MIME types
            MediaType::Image => vec![
                "image/jpeg", "image/png", "image/webp", "image/gif"
            ],
            MediaType::Video => vec![
                "video/mp4", "video/3gpp", "video/quicktime", "video/avi", "video/mkv"
            ],
            MediaType::Audio => vec![
                "audio/mpeg", "audio/mp4", "audio/aac", "audio/ogg", "audio/wav"
            ],
            MediaType::VoiceNote => vec![
                "audio/ogg", "audio/mpeg", "audio/mp4", "audio/aac"
            ],
            MediaType::Document => vec![
                "application/pdf", "application/msword", "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "application/vnd.ms-excel", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "text/plain", "application/zip", "application/json"
            ],
            MediaType::Sticker => vec![
                "image/webp", "image/png"
            ],
            MediaType::AnimatedSticker => vec![
                "image/webp", "video/mp4"
            ],
            MediaType::Location => vec![],
            MediaType::Contact => vec![],
        }
    }
    
    /// Get maximum file size for this media type (in bytes)  
    pub fn max_file_size(&self) -> u64 {
        match self {
            MediaType::Auto => 100 * 1024 * 1024, // 100MB for auto detection
            MediaType::Image => 16 * 1024 * 1024,        // 16 MB
            MediaType::Video => 64 * 1024 * 1024,        // 64 MB
            MediaType::Audio => 16 * 1024 * 1024,        // 16 MB
            MediaType::VoiceNote => 16 * 1024 * 1024,    // 16 MB
            MediaType::Document => 100 * 1024 * 1024,    // 100 MB
            MediaType::Sticker => 500 * 1024,            // 500 KB
            MediaType::AnimatedSticker => 500 * 1024,    // 500 KB
            MediaType::Location => 0,
            MediaType::Contact => 0,
        }
    }
    
    /// Check if this media type requires thumbnail generation
    pub fn requires_thumbnail(&self) -> bool {
        matches!(self, MediaType::Image | MediaType::Video | MediaType::Document)
    }
    
    /// Check if this media type supports captions
    pub fn supports_caption(&self) -> bool {
        matches!(self, MediaType::Image | MediaType::Video | MediaType::Document)
    }
}

/// Media information for uploaded content
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaInfo {
    /// Media URL for download
    pub url: String,
    /// Direct path for WhatsApp servers
    pub direct_path: Option<String>,
    /// Media key for decryption
    pub media_key: Vec<u8>,
    /// File SHA256 hash
    pub file_sha256: Vec<u8>,
    /// Encrypted file SHA256 hash
    pub file_enc_sha256: Vec<u8>,
    /// File size in bytes
    pub file_length: u64,
    /// MIME type
    pub mime_type: String,
    /// Upload timestamp
    pub upload_timestamp: u64,
    /// Media type detected
    pub media_type: MediaType,
    /// Image/video width
    pub width: Option<u32>,
    /// Image/video height
    pub height: Option<u32>,
    /// Audio/video duration in seconds
    pub duration: Option<u32>,
    /// Thumbnail data
    pub thumbnail: Option<Vec<u8>>,
}

impl MediaInfo {
    /// Create new media info
    pub fn new(
        url: String,
        direct_path: Option<String>,
        media_key: Vec<u8>,
        file_sha256: Vec<u8>,
        file_enc_sha256: Vec<u8>,
        file_length: u64,
        mime_type: String,
        media_type: MediaType,
    ) -> Self {
        Self {
            url,
            direct_path,
            media_key,
            file_sha256,
            file_enc_sha256,
            file_length,
            mime_type,
            upload_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            media_type,
            width: None,
            height: None,
            duration: None,
            thumbnail: None,
        }
    }
    
    /// Validate media info completeness
    pub fn validate(&self) -> bool {
        !self.url.is_empty() &&
        self.direct_path.as_ref().map_or(true, |p| !p.is_empty()) &&
        !self.media_key.is_empty() &&
        self.media_key.len() == 32 &&
        !self.file_sha256.is_empty() &&
        self.file_sha256.len() == 32 &&
        !self.file_enc_sha256.is_empty() &&
        self.file_enc_sha256.len() == 32 &&
        self.file_length > 0 &&
        !self.mime_type.is_empty()
    }
}

/// Complete media message structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaMessage {
    /// Type of media
    pub media_type: MediaType,
    /// Media information for download
    pub media_info: MediaInfo,
    /// Optional caption text
    pub caption: Option<String>,
    /// Thumbnail data (JPEG encoded)
    pub thumbnail: Option<Vec<u8>>,
    /// Duration in seconds (for video/audio)
    pub duration: Option<u32>,
    /// Width in pixels (for images/video)
    pub width: Option<u32>,
    /// Height in pixels (for images/video)
    pub height: Option<u32>,
    /// File size in bytes
    pub file_size: u64,
    /// MIME type
    pub mime_type: String,
    /// Original filename
    pub filename: Option<String>,
}

impl MediaMessage {
    /// Create a new media message
    pub fn new(media_type: MediaType, media_info: MediaInfo) -> Self {
        Self {
            media_type,
            file_size: media_info.file_length,
            mime_type: media_info.mime_type.clone(),
            media_info,
            caption: None,
            thumbnail: None,
            duration: None,
            width: None,
            height: None,
            filename: None,
        }
    }
    
    /// Set caption (if supported by media type)
    pub fn with_caption(mut self, caption: String) -> Self {
        if self.media_type.supports_caption() {
            self.caption = Some(caption);
        }
        self
    }
    
    /// Set thumbnail
    pub fn with_thumbnail(mut self, thumbnail: Vec<u8>) -> Self {
        self.thumbnail = Some(thumbnail);
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
    
    /// Set filename
    pub fn with_filename(mut self, filename: String) -> Self {
        self.filename = Some(filename);
        self
    }
    
    /// Validate media message
    pub fn validate(&self) -> bool {
        // Basic validation
        if !self.media_info.validate() || self.file_size == 0 {
            return false;
        }
        
        // Check file size limits
        if self.file_size > self.media_type.max_file_size() {
            return false;
        }
        
        // Check MIME type
        let expected_types = self.media_type.expected_mime_types();
        if !expected_types.is_empty() && !expected_types.contains(&self.mime_type.as_str()) {
            return false;
        }
        
        // Type-specific validation
        match self.media_type {
            MediaType::Image | MediaType::Video | MediaType::Sticker | MediaType::AnimatedSticker => {
                self.width.is_some() && self.height.is_some()
            },
            MediaType::Audio | MediaType::VoiceNote => {
                self.duration.is_some()
            },
            _ => true,
        }
    }
    
    /// Get display size in a human-readable format
    pub fn get_display_size(&self) -> String {
        let size = self.file_size as f64;
        if size < 1024.0 {
            format!("{} B", size)
        } else if size < 1024.0 * 1024.0 {
            format!("{:.1} KB", size / 1024.0)
        } else if size < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1} MB", size / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", size / (1024.0 * 1024.0 * 1024.0))
        }
    }
    
    /// Get display duration in a human-readable format
    pub fn get_display_duration(&self) -> Option<String> {
        self.duration.map(|dur| {
            let minutes = dur / 60;
            let seconds = dur % 60;
            if minutes > 0 {
                format!("{}:{:02}", minutes, seconds)
            } else {
                format!("0:{:02}", seconds)
            }
        })
    }
}

/// Location message data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocationMessage {
    /// Latitude coordinate
    pub latitude: f64,
    /// Longitude coordinate
    pub longitude: f64,
    /// Location name/address
    pub name: Option<String>,
    /// Location address
    pub address: Option<String>,
    /// Location URL (Google Maps, etc.)
    pub url: Option<String>,
    /// Thumbnail image of the location
    pub thumbnail: Option<Vec<u8>>,
}

impl LocationMessage {
    /// Create a new location message
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
            name: None,
            address: None,
            url: None,
            thumbnail: None,
        }
    }
    
    /// Set location name
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }
    
    /// Set location address
    pub fn with_address(mut self, address: String) -> Self {
        self.address = Some(address);
        self
    }
    
    /// Set location URL
    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }
    
    /// Set location thumbnail
    pub fn with_thumbnail(mut self, thumbnail: Vec<u8>) -> Self {
        self.thumbnail = Some(thumbnail);
        self
    }
    
    /// Validate location coordinates
    pub fn validate(&self) -> bool {
        self.latitude >= -90.0 && self.latitude <= 90.0 &&
        self.longitude >= -180.0 && self.longitude <= 180.0
    }
    
    /// Generate Google Maps URL
    pub fn generate_maps_url(&self) -> String {
        format!("https://maps.google.com/maps?q={},{}", self.latitude, self.longitude)
    }
}

/// Contact message data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContactMessage {
    /// Contact display name
    pub display_name: String,
    /// vCard data
    pub vcard: String,
    /// Contact phone numbers
    pub phone_numbers: Vec<String>,
    /// Contact email addresses
    pub emails: Vec<String>,
    /// Contact organization
    pub organization: Option<String>,
    /// Contact photo thumbnail
    pub photo: Option<Vec<u8>>,
}

impl ContactMessage {
    /// Create a new contact message
    pub fn new(display_name: String, vcard: String) -> Self {
        Self {
            display_name,
            vcard,
            phone_numbers: Vec::new(),
            emails: Vec::new(),
            organization: None,
            photo: None,
        }
    }
    
    /// Add a phone number
    pub fn add_phone_number(mut self, phone: String) -> Self {
        self.phone_numbers.push(phone);
        self
    }
    
    /// Add an email address
    pub fn add_email(mut self, email: String) -> Self {
        self.emails.push(email);
        self
    }
    
    /// Set organization
    pub fn with_organization(mut self, organization: String) -> Self {
        self.organization = Some(organization);
        self
    }
    
    /// Set contact photo
    pub fn with_photo(mut self, photo: Vec<u8>) -> Self {
        self.photo = Some(photo);
        self
    }
    
    /// Validate contact data
    pub fn validate(&self) -> bool {
        !self.display_name.is_empty() && !self.vcard.is_empty()
    }
}

/// Upload/Download progress information
#[derive(Debug, Clone, PartialEq)]
pub struct ProgressInfo {
    /// Bytes transferred so far
    pub bytes_transferred: u64,
    /// Total bytes to transfer
    pub total_bytes: u64,
    /// Progress as percentage (0.0 to 1.0)
    pub progress: f32,
    /// Transfer speed in bytes per second
    pub speed_bps: Option<u64>,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<u64>,
}

impl ProgressInfo {
    /// Create new progress info
    pub fn new(bytes_transferred: u64, total_bytes: u64) -> Self {
        let progress = if total_bytes > 0 {
            (bytes_transferred as f32) / (total_bytes as f32)
        } else {
            0.0
        };
        
        Self {
            bytes_transferred,
            total_bytes,
            progress,
            speed_bps: None,
            eta_seconds: None,
        }
    }
    
    /// Update with speed information
    pub fn with_speed(mut self, speed_bps: u64) -> Self {
        self.speed_bps = Some(speed_bps);
        
        // Calculate ETA
        if speed_bps > 0 {
            let remaining_bytes = self.total_bytes.saturating_sub(self.bytes_transferred);
            self.eta_seconds = Some(remaining_bytes / speed_bps);
        }
        
        self
    }
    
    /// Get progress as percentage string
    pub fn progress_percentage(&self) -> String {
        format!("{:.1}%", self.progress * 100.0)
    }
    
    /// Get speed in human-readable format
    pub fn speed_display(&self) -> Option<String> {
        self.speed_bps.map(|speed| {
            if speed < 1024 {
                format!("{} B/s", speed)
            } else if speed < 1024 * 1024 {
                format!("{:.1} KB/s", speed as f64 / 1024.0)
            } else {
                format!("{:.1} MB/s", speed as f64 / (1024.0 * 1024.0))
            }
        })
    }
    
    /// Get ETA in human-readable format
    pub fn eta_display(&self) -> Option<String> {
        self.eta_seconds.map(|eta| {
            if eta < 60 {
                format!("{}s", eta)
            } else if eta < 3600 {
                format!("{}m {}s", eta / 60, eta % 60)
            } else {
                format!("{}h {}m", eta / 3600, (eta % 3600) / 60)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_media_type_properties() {
        assert!(MediaType::Image.requires_thumbnail());
        assert!(MediaType::Video.requires_thumbnail());
        assert!(!MediaType::Audio.requires_thumbnail());
        
        assert!(MediaType::Image.supports_caption());
        assert!(MediaType::Video.supports_caption());
        assert!(!MediaType::VoiceNote.supports_caption());
        
        assert!(MediaType::Image.max_file_size() > 0);
        assert!(MediaType::Video.max_file_size() > MediaType::Image.max_file_size());
    }
    
    #[test]
    fn test_media_info_validation() {
        let media_info = MediaInfo::new(
            "https://example.com/file".to_string(),
            "/path/to/file".to_string(),
            vec![0u8; 32],
            vec![1u8; 32],
            vec![2u8; 32],
            1024,
            "image/jpeg".to_string(),
        );
        
        assert!(media_info.validate());
        
        // Invalid media key length
        let invalid_media_info = MediaInfo::new(
            "https://example.com/file".to_string(),
            "/path/to/file".to_string(),
            vec![0u8; 16], // Wrong length
            vec![1u8; 32],
            vec![2u8; 32],
            1024,
            "image/jpeg".to_string(),
        );
        
        assert!(!invalid_media_info.validate());
    }
    
    #[test]
    fn test_media_message_creation() {
        let media_info = MediaInfo::new(
            "https://example.com/image.jpg".to_string(),
            "/path/to/image.jpg".to_string(),
            vec![0u8; 32],
            vec![1u8; 32],
            vec![2u8; 32],
            1024,
            "image/jpeg".to_string(),
        );
        
        let message = MediaMessage::new(MediaType::Image, media_info)
            .with_caption("Test image".to_string())
            .with_dimensions(800, 600)
            .with_filename("image.jpg".to_string());
        
        assert_eq!(message.media_type, MediaType::Image);
        assert_eq!(message.caption, Some("Test image".to_string()));
        assert_eq!(message.width, Some(800));
        assert_eq!(message.height, Some(600));
        assert_eq!(message.filename, Some("image.jpg".to_string()));
    }
    
    #[test]
    fn test_location_message() {
        let location = LocationMessage::new(37.7749, -122.4194)
            .with_name("San Francisco".to_string())
            .with_address("San Francisco, CA, USA".to_string());
        
        assert!(location.validate());
        assert_eq!(location.name, Some("San Francisco".to_string()));
        
        let maps_url = location.generate_maps_url();
        assert!(maps_url.contains("37.7749"));
        assert!(maps_url.contains("-122.4194"));
        
        // Invalid coordinates
        let invalid_location = LocationMessage::new(91.0, 181.0);
        assert!(!invalid_location.validate());
    }
    
    #[test]
    fn test_contact_message() {
        let contact = ContactMessage::new(
            "John Doe".to_string(),
            "BEGIN:VCARD\nVERSION:3.0\nFN:John Doe\nEND:VCARD".to_string(),
        )
        .add_phone_number("+1234567890".to_string())
        .add_email("john@example.com".to_string())
        .with_organization("Example Corp".to_string());
        
        assert!(contact.validate());
        assert_eq!(contact.phone_numbers.len(), 1);
        assert_eq!(contact.emails.len(), 1);
        assert_eq!(contact.organization, Some("Example Corp".to_string()));
    }
    
    #[test]
    fn test_progress_info() {
        let progress = ProgressInfo::new(512, 1024);
        assert_eq!(progress.progress, 0.5);
        assert_eq!(progress.progress_percentage(), "50.0%");
        
        let progress_with_speed = progress.with_speed(1024);
        assert_eq!(progress_with_speed.speed_bps, Some(1024));
        assert_eq!(progress_with_speed.eta_seconds, Some(0)); // 512 remaining / 1024 speed = 0.5 -> 0
        
        let speed_display = progress_with_speed.speed_display().unwrap();
        assert_eq!(speed_display, "1.0 KB/s");
    }
    
    #[test]
    fn test_media_message_display_methods() {
        let media_info = MediaInfo::new(
            "https://example.com/video.mp4".to_string(),
            "/path/to/video.mp4".to_string(),
            vec![0u8; 32],
            vec![1u8; 32],
            vec![2u8; 32],
            5 * 1024 * 1024, // 5 MB
            "video/mp4".to_string(),
        );
        
        let message = MediaMessage::new(MediaType::Video, media_info)
            .with_duration(150); // 2:30
        
        assert_eq!(message.get_display_size(), "5.0 MB");
        assert_eq!(message.get_display_duration(), Some("2:30".to_string()));
    }
}