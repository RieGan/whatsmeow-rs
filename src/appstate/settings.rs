/// Settings synchronization system for WhatsApp App State
/// 
/// Handles synchronization of user preferences and settings including:
/// - User profile settings
/// - Privacy settings
/// - Notification preferences
/// - Media settings
/// - Chat settings
/// - Security settings

use crate::{
    appstate::{
        AppStateSync, AppStateEvent, AppStateOperation, AppStateDataType, 
        AppStateKey, SyncContext, SyncStatus, SyncConflict, AppStateVersion
    },
    error::{Error, Result},
    types::JID,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::SystemTime,
};
use tokio::sync::RwLock;

/// User settings synchronized with WhatsApp
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserSettings {
    /// Settings identifier
    pub settings_id: String,
    /// Profile settings
    pub profile: ProfileSettings,
    /// Privacy settings
    pub privacy: PrivacySettings,
    /// Notification settings
    pub notifications: NotificationSettings,
    /// Media settings
    pub media: MediaSettings,
    /// Chat settings
    pub chat: ChatSettings,
    /// Security settings
    pub security: SecuritySettings,
    /// Appearance settings
    pub appearance: AppearanceSettings,
    /// Storage settings
    pub storage: StorageSettings,
    /// Language and locale settings
    pub locale: LocaleSettings,
    /// Last time settings were updated
    pub last_updated: SystemTime,
    /// Sync version for conflict resolution
    pub version: AppStateVersion,
}

/// Profile settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileSettings {
    /// Display name
    pub display_name: Option<String>,
    /// Status message
    pub status_message: Option<String>,
    /// Profile photo URL
    pub profile_photo_url: Option<String>,
    /// Profile photo data
    pub profile_photo_data: Option<Vec<u8>>,
    /// Show profile photo to
    pub show_profile_photo_to: ProfilePhotoVisibility,
    /// Show status to
    pub show_status_to: StatusVisibility,
    /// Business profile settings
    pub business_profile: Option<BusinessProfileSettings>,
}

/// Privacy settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrivacySettings {
    /// Last seen visibility
    pub last_seen: LastSeenVisibility,
    /// Profile photo visibility
    pub profile_photo: ProfilePhotoVisibility,
    /// Status visibility
    pub status: StatusVisibility,
    /// Online status visibility
    pub online_status: OnlineStatusVisibility,
    /// Read receipts enabled
    pub read_receipts: bool,
    /// Typing indicators enabled
    pub typing_indicators: bool,
    /// Groups: who can add me
    pub groups_add_me: GroupsAddMePermission,
    /// Calls: who can call me
    pub calls_permission: CallsPermission,
    /// Blocked contacts
    pub blocked_contacts: Vec<JID>,
    /// Two-step verification enabled
    pub two_step_verification: bool,
    /// Disappearing messages default timer
    pub default_disappearing_timer: Option<u32>,
}

/// Notification settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotificationSettings {
    /// Notifications enabled
    pub enabled: bool,
    /// Message notifications
    pub message_notifications: MessageNotificationSettings,
    /// Group notifications
    pub group_notifications: GroupNotificationSettings,
    /// Call notifications
    pub call_notifications: CallNotificationSettings,
    /// Security notifications
    pub security_notifications: SecurityNotificationSettings,
    /// Do not disturb settings
    pub do_not_disturb: DoNotDisturbSettings,
}

/// Media settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaSettings {
    /// Auto-download media on mobile data
    pub auto_download_mobile: MediaAutoDownloadSettings,
    /// Auto-download media on Wi-Fi
    pub auto_download_wifi: MediaAutoDownloadSettings,
    /// Auto-download media when roaming
    pub auto_download_roaming: MediaAutoDownloadSettings,
    /// Media quality settings
    pub media_quality: MediaQualitySettings,
    /// Storage management
    pub storage_management: StorageManagementSettings,
}

/// Chat settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatSettings {
    /// Default theme
    pub default_theme: String,
    /// Default wallpaper
    pub default_wallpaper: Option<String>,
    /// Font size
    pub font_size: FontSize,
    /// Chat backup settings
    pub backup: BackupSettings,
    /// Archive all chats enabled
    pub archive_all_chats: bool,
    /// Keep chats archived
    pub keep_chats_archived: bool,
    /// Enter is send enabled
    pub enter_is_send: bool,
    /// Media visibility in gallery
    pub media_visibility_in_gallery: bool,
}

/// Security settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SecuritySettings {
    /// Show security notifications
    pub show_security_notifications: bool,
    /// Fingerprint lock enabled
    pub fingerprint_lock: bool,
    /// Fingerprint lock timeout
    pub fingerprint_lock_timeout: Option<u32>,
    /// Screen lock enabled
    pub screen_lock: bool,
    /// Screen lock timeout
    pub screen_lock_timeout: Option<u32>,
    /// App lock enabled
    pub app_lock: bool,
    /// App lock timeout
    pub app_lock_timeout: Option<u32>,
}

/// Appearance settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppearanceSettings {
    /// Theme mode
    pub theme_mode: ThemeMode,
    /// Dark mode enabled
    pub dark_mode: bool,
    /// System theme enabled (follow system)
    pub system_theme: bool,
    /// Chat wallpaper
    pub chat_wallpaper: Option<String>,
    /// Interface language
    pub interface_language: Option<String>,
}

/// Storage settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageSettings {
    /// Storage usage limit
    pub storage_limit: Option<u64>,
    /// Auto-delete old media
    pub auto_delete_media: bool,
    /// Auto-delete media after days
    pub auto_delete_after_days: Option<u32>,
    /// Keep recent media count
    pub keep_recent_media_count: Option<u32>,
}

/// Locale settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocaleSettings {
    /// Language code
    pub language: String,
    /// Country code
    pub country: String,
    /// Timezone
    pub timezone: String,
    /// Date format
    pub date_format: DateFormat,
    /// Time format (12/24 hour)
    pub time_format: TimeFormat,
}

// Enums for various setting options

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProfilePhotoVisibility {
    Everyone,
    Contacts,
    Nobody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StatusVisibility {
    Everyone,
    Contacts,
    ContactsExcept(Vec<JID>),
    Nobody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LastSeenVisibility {
    Everyone,
    Contacts,
    Nobody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OnlineStatusVisibility {
    Everyone,
    SameAsLastSeen,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GroupsAddMePermission {
    Everyone,
    Contacts,
    ContactsExcept(Vec<JID>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CallsPermission {
    Everyone,
    Contacts,
    Nobody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FontSize {
    Small,
    Medium,
    Large,
    ExtraLarge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DateFormat {
    DMY, // DD/MM/YYYY
    MDY, // MM/DD/YYYY
    YMD, // YYYY/MM/DD
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TimeFormat {
    TwentyFourHour,
    TwelveHour,
}

// Nested settings structures

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BusinessProfileSettings {
    pub business_name: String,
    pub business_category: String,
    pub business_description: Option<String>,
    pub business_website: Option<String>,
    pub business_email: Option<String>,
    pub business_address: Option<String>,
    pub business_hours: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageNotificationSettings {
    pub enabled: bool,
    pub sound: Option<String>,
    pub vibration: bool,
    pub light: bool,
    pub popup: bool,
    pub high_priority: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupNotificationSettings {
    pub enabled: bool,
    pub sound: Option<String>,
    pub vibration: bool,
    pub light: bool,
    pub popup: bool,
    pub high_priority: bool,
    pub use_custom_sound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallNotificationSettings {
    pub enabled: bool,
    pub ringtone: Option<String>,
    pub vibration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SecurityNotificationSettings {
    pub enabled: bool,
    pub sound: Option<String>,
    pub vibration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DoNotDisturbSettings {
    pub enabled: bool,
    pub start_time: Option<String>, // HH:MM format
    pub end_time: Option<String>,   // HH:MM format
    pub days: Vec<u8>, // 0-6 for Sunday-Saturday
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaAutoDownloadSettings {
    pub photos: bool,
    pub audio: bool,
    pub videos: bool,
    pub documents: bool,
    pub video_size_limit: Option<u64>, // in bytes
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaQualitySettings {
    pub photo_quality: PhotoQuality,
    pub video_quality: VideoQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PhotoQuality {
    Auto,
    BestQuality,
    DataSaver,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VideoQuality {
    Auto,
    BestQuality,
    DataSaver,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageManagementSettings {
    pub auto_cleanup_enabled: bool,
    pub cleanup_frequency_days: u32,
    pub keep_media_days: u32,
    pub cleanup_large_files: bool,
    pub large_file_threshold: u64, // in bytes
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackupSettings {
    pub enabled: bool,
    pub frequency: BackupFrequency,
    pub include_videos: bool,
    pub wifi_only: bool,
    pub backup_account: Option<String>,
    pub last_backup: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackupFrequency {
    Never,
    Daily,
    Weekly,
    Monthly,
}

/// Settings synchronization manager
pub struct SettingsSync {
    /// Settings storage
    settings: Arc<RwLock<HashMap<String, UserSettings>>>,
    /// Settings cache for quick access
    settings_cache: Arc<RwLock<Option<CachedSettings>>>,
}

/// Cached settings for performance
#[derive(Debug, Clone)]
pub struct CachedSettings {
    pub settings: UserSettings,
    pub cached_at: SystemTime,
    pub expires_at: SystemTime,
}

impl UserSettings {
    /// Create default user settings
    pub fn default_settings() -> Self {
        Self {
            settings_id: "default".to_string(),
            profile: ProfileSettings::default(),
            privacy: PrivacySettings::default(),
            notifications: NotificationSettings::default(),
            media: MediaSettings::default(),
            chat: ChatSettings::default(),
            security: SecuritySettings::default(),
            appearance: AppearanceSettings::default(),
            storage: StorageSettings::default(),
            locale: LocaleSettings::default(),
            last_updated: SystemTime::now(),
            version: AppStateVersion {
                timestamp: SystemTime::now(),
                hash: String::new(),
                device_id: "local".to_string(),
            },
        }
    }

    /// Update a specific setting category
    pub fn update_profile(&mut self, profile: ProfileSettings) {
        self.profile = profile;
        self.mark_updated();
    }

    pub fn update_privacy(&mut self, privacy: PrivacySettings) {
        self.privacy = privacy;
        self.mark_updated();
    }

    pub fn update_notifications(&mut self, notifications: NotificationSettings) {
        self.notifications = notifications;
        self.mark_updated();
    }

    pub fn update_media(&mut self, media: MediaSettings) {
        self.media = media;
        self.mark_updated();
    }

    pub fn update_chat(&mut self, chat: ChatSettings) {
        self.chat = chat;
        self.mark_updated();
    }

    pub fn update_security(&mut self, security: SecuritySettings) {
        self.security = security;
        self.mark_updated();
    }

    pub fn update_appearance(&mut self, appearance: AppearanceSettings) {
        self.appearance = appearance;
        self.mark_updated();
    }

    pub fn update_storage(&mut self, storage: StorageSettings) {
        self.storage = storage;
        self.mark_updated();
    }

    pub fn update_locale(&mut self, locale: LocaleSettings) {
        self.locale = locale;
        self.mark_updated();
    }

    /// Mark settings as updated
    fn mark_updated(&mut self) {
        self.last_updated = SystemTime::now();
        self.version.timestamp = SystemTime::now();
    }
}

// Default implementations for settings structures

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            display_name: None,
            status_message: Some("Hey there! I am using WhatsApp.".to_string()),
            profile_photo_url: None,
            profile_photo_data: None,
            show_profile_photo_to: ProfilePhotoVisibility::Everyone,
            show_status_to: StatusVisibility::Contacts,
            business_profile: None,
        }
    }
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            last_seen: LastSeenVisibility::Everyone,
            profile_photo: ProfilePhotoVisibility::Everyone,
            status: StatusVisibility::Contacts,
            online_status: OnlineStatusVisibility::Everyone,
            read_receipts: true,
            typing_indicators: true,
            groups_add_me: GroupsAddMePermission::Everyone,
            calls_permission: CallsPermission::Everyone,
            blocked_contacts: Vec::new(),
            two_step_verification: false,
            default_disappearing_timer: None,
        }
    }
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            message_notifications: MessageNotificationSettings::default(),
            group_notifications: GroupNotificationSettings::default(),
            call_notifications: CallNotificationSettings::default(),
            security_notifications: SecurityNotificationSettings::default(),
            do_not_disturb: DoNotDisturbSettings::default(),
        }
    }
}

impl Default for MediaSettings {
    fn default() -> Self {
        Self {
            auto_download_mobile: MediaAutoDownloadSettings {
                photos: true,
                audio: true,
                videos: false,
                documents: false,
                video_size_limit: Some(16 * 1024 * 1024), // 16MB
            },
            auto_download_wifi: MediaAutoDownloadSettings {
                photos: true,
                audio: true,
                videos: true,
                documents: true,
                video_size_limit: Some(50 * 1024 * 1024), // 50MB
            },
            auto_download_roaming: MediaAutoDownloadSettings {
                photos: false,
                audio: false,
                videos: false,
                documents: false,
                video_size_limit: None,
            },
            media_quality: MediaQualitySettings {
                photo_quality: PhotoQuality::Auto,
                video_quality: VideoQuality::Auto,
            },
            storage_management: StorageManagementSettings {
                auto_cleanup_enabled: false,
                cleanup_frequency_days: 30,
                keep_media_days: 90,
                cleanup_large_files: false,
                large_file_threshold: 100 * 1024 * 1024, // 100MB
            },
        }
    }
}

impl Default for ChatSettings {
    fn default() -> Self {
        Self {
            default_theme: "default".to_string(),
            default_wallpaper: None,
            font_size: FontSize::Medium,
            backup: BackupSettings {
                enabled: false,
                frequency: BackupFrequency::Never,
                include_videos: false,
                wifi_only: true,
                backup_account: None,
                last_backup: None,
            },
            archive_all_chats: false,
            keep_chats_archived: true,
            enter_is_send: false,
            media_visibility_in_gallery: true,
        }
    }
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            show_security_notifications: true,
            fingerprint_lock: false,
            fingerprint_lock_timeout: Some(300), // 5 minutes
            screen_lock: false,
            screen_lock_timeout: Some(300),
            app_lock: false,
            app_lock_timeout: Some(300),
        }
    }
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            dark_mode: false,
            system_theme: true,
            chat_wallpaper: None,
            interface_language: None,
        }
    }
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            storage_limit: None,
            auto_delete_media: false,
            auto_delete_after_days: Some(30),
            keep_recent_media_count: Some(100),
        }
    }
}

impl Default for LocaleSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            country: "US".to_string(),
            timezone: "UTC".to_string(),
            date_format: DateFormat::MDY,
            time_format: TimeFormat::TwelveHour,
        }
    }
}

impl Default for MessageNotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sound: Some("default".to_string()),
            vibration: true,
            light: true,
            popup: true,
            high_priority: false,
        }
    }
}

impl Default for GroupNotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sound: Some("default".to_string()),
            vibration: true,
            light: true,
            popup: true,
            high_priority: false,
            use_custom_sound: false,
        }
    }
}

impl Default for CallNotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            ringtone: Some("default".to_string()),
            vibration: true,
        }
    }
}

impl Default for SecurityNotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sound: Some("default".to_string()),
            vibration: true,
        }
    }
}

impl Default for DoNotDisturbSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            start_time: None,
            end_time: None,
            days: Vec::new(),
        }
    }
}

impl SettingsSync {
    /// Create a new settings sync manager
    pub fn new() -> Self {
        Self {
            settings: Arc::new(RwLock::new(HashMap::new())),
            settings_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Get user settings
    pub async fn get_settings(&self, settings_id: &str) -> Option<UserSettings> {
        // Try cache first
        if let Some(cached) = self.get_from_cache().await {
            if cached.settings.settings_id == settings_id {
                return Some(cached.settings);
            }
        }

        // Fall back to storage
        let storage = self.settings.read().await;
        let settings = storage.get(settings_id).cloned();

        // Update cache if found
        if let Some(ref settings) = settings {
            self.update_cache(settings.clone()).await;
        }

        settings
    }

    /// Update user settings
    pub async fn update_settings(&self, settings: UserSettings) -> Result<()> {
        let settings_id = settings.settings_id.clone();

        // Update storage
        {
            let mut storage = self.settings.write().await;
            storage.insert(settings_id, settings.clone());
        }

        // Update cache
        self.update_cache(settings).await;

        Ok(())
    }

    /// Get or create default settings
    pub async fn get_or_create_settings(&self, settings_id: &str) -> UserSettings {
        if let Some(settings) = self.get_settings(settings_id).await {
            settings
        } else {
            let mut default_settings = UserSettings::default_settings();
            default_settings.settings_id = settings_id.to_string();
            
            if let Err(e) = self.update_settings(default_settings.clone()).await {
                tracing::warn!("Failed to save default settings: {}", e);
            }
            
            default_settings
        }
    }

    /// Update specific setting category
    pub async fn update_profile_settings(&self, settings_id: &str, profile: ProfileSettings) -> Result<()> {
        let mut settings = self.get_or_create_settings(settings_id).await;
        settings.update_profile(profile);
        self.update_settings(settings).await
    }

    pub async fn update_privacy_settings(&self, settings_id: &str, privacy: PrivacySettings) -> Result<()> {
        let mut settings = self.get_or_create_settings(settings_id).await;
        settings.update_privacy(privacy);
        self.update_settings(settings).await
    }

    pub async fn update_notification_settings(&self, settings_id: &str, notifications: NotificationSettings) -> Result<()> {
        let mut settings = self.get_or_create_settings(settings_id).await;
        settings.update_notifications(notifications);
        self.update_settings(settings).await
    }

    /// Get cached settings if valid
    async fn get_from_cache(&self) -> Option<CachedSettings> {
        let cache = self.settings_cache.read().await;
        if let Some(cached) = cache.as_ref() {
            if cached.expires_at > SystemTime::now() {
                return Some(cached.clone());
            }
        }
        None
    }

    /// Update cache with settings
    async fn update_cache(&self, settings: UserSettings) {
        let cache_entry = CachedSettings {
            settings,
            cached_at: SystemTime::now(),
            expires_at: SystemTime::now() + std::time::Duration::from_secs(600), // 10 minutes
        };

        let mut cache = self.settings_cache.write().await;
        *cache = Some(cache_entry);
    }

    /// Calculate hash for settings version
    fn calculate_settings_hash(&self, settings: &UserSettings) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        settings.settings_id.hash(&mut hasher);
        // Hash key settings that frequently change
        settings.privacy.read_receipts.hash(&mut hasher);
        settings.notifications.enabled.hash(&mut hasher);
        settings.appearance.theme_mode.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }

    /// Merge settings for conflict resolution
    pub fn merge_settings(&self, local: &UserSettings, remote: &UserSettings) -> UserSettings {
        let mut merged = local.clone();

        // Use the most recent version for most fields
        if remote.version.timestamp > local.version.timestamp {
            merged.profile = remote.profile.clone();
            merged.privacy = remote.privacy.clone();
            merged.notifications = remote.notifications.clone();
            merged.media = remote.media.clone();
            merged.chat = remote.chat.clone();
            merged.security = remote.security.clone();
            merged.appearance = remote.appearance.clone();
            merged.storage = remote.storage.clone();
            merged.locale = remote.locale.clone();
            merged.version = remote.version.clone();
        }

        // Merge blocked contacts (union)
        for blocked_jid in &remote.privacy.blocked_contacts {
            if !merged.privacy.blocked_contacts.contains(blocked_jid) {
                merged.privacy.blocked_contacts.push(blocked_jid.clone());
            }
        }

        // Update timestamp
        merged.last_updated = SystemTime::now();

        merged
    }
}

#[async_trait::async_trait]
impl AppStateSync for SettingsSync {
    fn data_type(&self) -> AppStateDataType {
        AppStateDataType::Settings
    }

    async fn sync_from_remote(&self, ctx: &SyncContext, events: Vec<AppStateEvent>) -> Result<()> {
        for event in events {
            match event.operation {
                AppStateOperation::Update => {
                    if let Some(data) = event.data {
                        let settings: UserSettings = serde_json::from_slice(&data)
                            .map_err(|e| Error::Protocol(format!("Failed to deserialize settings: {}", e)))?;

                        let key = AppStateKey::settings(&settings.settings_id);

                        // Check for conflicts
                        if let Some(existing) = self.get_settings(&settings.settings_id).await {
                            if existing.version.timestamp > settings.version.timestamp {
                                // Local version is newer, create conflict
                                let conflict = SyncConflict {
                                    key: key.clone(),
                                    local_version: existing.version,
                                    remote_version: settings.version,
                                    local_data: Some(serde_json::to_vec(&existing).unwrap()),
                                    remote_data: Some(data),
                                    detected_at: SystemTime::now(),
                                };
                                ctx.add_conflict(conflict).await;
                                ctx.update_sync_status(key, SyncStatus::Conflict).await;
                                continue;
                            }
                        }

                        self.update_settings(settings).await?;
                        ctx.update_sync_status(key, SyncStatus::Synced).await;
                    }
                }
                AppStateOperation::Delete => {
                    let settings_id = &event.key;
                    let mut storage = self.settings.write().await;
                    storage.remove(settings_id);

                    let key = AppStateKey::settings(settings_id);
                    ctx.update_sync_status(key, SyncStatus::Synced).await;
                }
                _ => {
                    // Handle other operations as needed
                }
            }
        }

        ctx.update_last_sync(AppStateDataType::Settings).await;
        Ok(())
    }

    async fn sync_to_remote(&self, ctx: &SyncContext) -> Result<Vec<AppStateEvent>> {
        let mut events = Vec::new();
        let storage = self.settings.read().await;

        for (settings_id, settings) in storage.iter() {
            let key = AppStateKey::settings(settings_id);
            let status = ctx.get_sync_status(&key).await;

            if status == SyncStatus::NotSynced {
                let data = serde_json::to_vec(settings)
                    .map_err(|e| Error::Protocol(format!("Failed to serialize settings: {}", e)))?;

                events.push(AppStateEvent {
                    data_type: AppStateDataType::Settings,
                    operation: AppStateOperation::Update,
                    timestamp: settings.last_updated,
                    key: settings_id.clone(),
                    data: Some(data),
                });

                ctx.update_sync_status(key, SyncStatus::Syncing).await;
            }
        }

        Ok(events)
    }

    async fn incremental_sync(&self, ctx: &SyncContext, since: SystemTime) -> Result<Vec<AppStateEvent>> {
        let mut events = Vec::new();
        let storage = self.settings.read().await;

        for (settings_id, settings) in storage.iter() {
            if settings.last_updated > since {
                let data = serde_json::to_vec(settings)
                    .map_err(|e| Error::Protocol(format!("Failed to serialize settings: {}", e)))?;

                events.push(AppStateEvent {
                    data_type: AppStateDataType::Settings,
                    operation: AppStateOperation::Update,
                    timestamp: settings.last_updated,
                    key: settings_id.clone(),
                    data: Some(data),
                });
            }
        }

        Ok(events)
    }

    async fn full_sync(&self, ctx: &SyncContext) -> Result<Vec<AppStateEvent>> {
        self.sync_to_remote(ctx).await
    }

    async fn resolve_conflicts(&self, ctx: &SyncContext, conflicts: Vec<SyncConflict>) -> Result<()> {
        for conflict in conflicts {
            if let (Some(local_data), Some(remote_data)) = (&conflict.local_data, &conflict.remote_data) {
                let local_settings: UserSettings = serde_json::from_slice(local_data)
                    .map_err(|e| Error::Protocol(format!("Failed to deserialize local settings: {}", e)))?;
                let remote_settings: UserSettings = serde_json::from_slice(remote_data)
                    .map_err(|e| Error::Protocol(format!("Failed to deserialize remote settings: {}", e)))?;

                // Merge settings
                let merged = self.merge_settings(&local_settings, &remote_settings);
                self.update_settings(merged).await?;

                ctx.update_sync_status(conflict.key, SyncStatus::Synced).await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_settings_basic_operations() {
        let sync = SettingsSync::new();

        let settings_id = "user123";
        let settings = sync.get_or_create_settings(settings_id).await;
        
        assert_eq!(settings.settings_id, settings_id);
        assert!(settings.notifications.enabled);
        assert_eq!(settings.appearance.theme_mode, ThemeMode::System);
    }

    #[tokio::test]
    async fn test_privacy_settings_update() {
        let sync = SettingsSync::new();

        let settings_id = "user123";
        let mut privacy = PrivacySettings::default();
        privacy.read_receipts = false;
        privacy.last_seen = LastSeenVisibility::Nobody;

        sync.update_privacy_settings(settings_id, privacy).await.unwrap();

        let updated_settings = sync.get_settings(settings_id).await.unwrap();
        assert!(!updated_settings.privacy.read_receipts);
        assert_eq!(updated_settings.privacy.last_seen, LastSeenVisibility::Nobody);
    }

    #[tokio::test]
    async fn test_blocked_contacts() {
        let sync = SettingsSync::new();

        let settings_id = "user123";
        let mut privacy = PrivacySettings::default();
        privacy.blocked_contacts.push(JID::new("blocked1".to_string(), "s.whatsapp.net".to_string()));
        privacy.blocked_contacts.push(JID::new("blocked2".to_string(), "s.whatsapp.net".to_string()));

        sync.update_privacy_settings(settings_id, privacy).await.unwrap();

        let settings = sync.get_settings(settings_id).await.unwrap();
        assert_eq!(settings.privacy.blocked_contacts.len(), 2);
    }

    #[tokio::test]
    async fn test_notification_settings() {
        let sync = SettingsSync::new();

        let settings_id = "user123";
        let mut notifications = NotificationSettings::default();
        notifications.enabled = false;
        notifications.message_notifications.sound = Some("custom_sound.mp3".to_string());

        sync.update_notification_settings(settings_id, notifications).await.unwrap();

        let settings = sync.get_settings(settings_id).await.unwrap();
        assert!(!settings.notifications.enabled);
        assert_eq!(settings.notifications.message_notifications.sound, Some("custom_sound.mp3".to_string()));
    }
}