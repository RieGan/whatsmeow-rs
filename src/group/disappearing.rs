/// WhatsApp Disappearing Messages implementation for Groups
/// 
/// Disappearing messages automatically delete after a specified time period.
/// This module implements group-specific disappearing message functionality,
/// including configuration, scheduling, and cleanup processes.

use crate::{
    error::{Error, Result},
    types::JID,
    group::{GroupInfo, GroupSettings, DisappearingMessageSettings},
};
use serde::{Deserialize, Serialize};
use std::{
    time::{SystemTime, Duration},
    collections::HashMap,
};

/// Disappearing message timer presets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisappearingTimer {
    /// 24 hours
    OneDay,
    /// 7 days
    OneWeek,
    /// 90 days
    NinetyDays,
    /// Custom duration in seconds
    Custom(u64),
}

impl DisappearingTimer {
    /// Get duration in seconds
    pub fn duration_seconds(&self) -> u64 {
        match self {
            Self::OneDay => 24 * 60 * 60,
            Self::OneWeek => 7 * 24 * 60 * 60,
            Self::NinetyDays => 90 * 24 * 60 * 60,
            Self::Custom(seconds) => *seconds,
        }
    }
    
    /// Create from seconds
    pub fn from_seconds(seconds: u64) -> Self {
        match seconds {
            86400 => Self::OneDay,       // 24 hours
            604800 => Self::OneWeek,     // 7 days
            7776000 => Self::NinetyDays, // 90 days
            _ => Self::Custom(seconds),
        }
    }
    
    /// Validate timer duration
    pub fn validate(&self) -> Result<()> {
        let duration = self.duration_seconds();
        
        // Minimum 1 minute
        if duration < 60 {
            return Err(Error::Protocol(
                "Disappearing timer too short (minimum 1 minute)".to_string()
            ));
        }
        
        // Maximum 1 year
        if duration > 365 * 24 * 60 * 60 {
            return Err(Error::Protocol(
                "Disappearing timer too long (maximum 1 year)".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Get human-readable description
    pub fn description(&self) -> String {
        match self {
            Self::OneDay => "1 day".to_string(),
            Self::OneWeek => "1 week".to_string(),
            Self::NinetyDays => "90 days".to_string(),
            Self::Custom(seconds) => {
                let days = seconds / (24 * 60 * 60);
                let hours = (seconds % (24 * 60 * 60)) / (60 * 60);
                let minutes = (seconds % (60 * 60)) / 60;
                
                if days > 0 {
                    format!("{} days", days)
                } else if hours > 0 {
                    format!("{} hours", hours)
                } else {
                    format!("{} minutes", minutes)
                }
            }
        }
    }
}

impl Default for DisappearingTimer {
    fn default() -> Self {
        Self::OneWeek
    }
}

/// Configuration for disappearing messages in a group
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupDisappearingConfig {
    /// Whether disappearing messages are enabled
    pub enabled: bool,
    /// Timer for new messages
    pub timer: DisappearingTimer,
    /// Whether setting was enabled by admin
    pub enabled_by_admin: bool,
    /// Who enabled the setting
    pub enabled_by: JID,
    /// When the setting was last changed
    pub last_changed: SystemTime,
    /// Whether members can change their own timer
    pub allow_member_timer_change: bool,
    /// Whether to show disappearing message notifications
    pub show_notifications: bool,
    /// Whether messages disappear for everyone or just sender
    pub disappear_for_everyone: bool,
}

impl GroupDisappearingConfig {
    /// Create new disappearing config
    pub fn new(enabled_by: JID, timer: DisappearingTimer) -> Self {
        Self {
            enabled: true,
            timer,
            enabled_by_admin: true,
            enabled_by,
            last_changed: SystemTime::now(),
            allow_member_timer_change: false,
            show_notifications: true,
            disappear_for_everyone: true,
        }
    }
    
    /// Disable disappearing messages
    pub fn disabled(disabled_by: JID) -> Self {
        Self {
            enabled: false,
            timer: DisappearingTimer::default(),
            enabled_by_admin: true,
            enabled_by: disabled_by,
            last_changed: SystemTime::now(),
            allow_member_timer_change: false,
            show_notifications: true,
            disappear_for_everyone: true,
        }
    }
    
    /// Update timer
    pub fn update_timer(&mut self, new_timer: DisappearingTimer, changed_by: JID) -> Result<()> {
        new_timer.validate()?;
        
        self.timer = new_timer;
        self.enabled_by = changed_by;
        self.last_changed = SystemTime::now();
        
        Ok(())
    }
    
    /// Enable/disable disappearing messages
    pub fn set_enabled(&mut self, enabled: bool, changed_by: JID) {
        self.enabled = enabled;
        self.enabled_by = changed_by;
        self.last_changed = SystemTime::now();
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            self.timer.validate()?;
        }
        Ok(())
    }
}

/// A message scheduled for disappearing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisappearingMessage {
    /// Message ID
    pub message_id: String,
    /// Group JID
    pub group_jid: JID,
    /// Sender JID
    pub sender: JID,
    /// When message was sent
    pub sent_at: SystemTime,
    /// When message should disappear
    pub disappear_at: SystemTime,
    /// Timer used for this message
    pub timer: DisappearingTimer,
    /// Whether message has been deleted
    pub deleted: bool,
    /// Message content type (for cleanup tracking)
    pub content_type: MessageContentType,
    /// Media file paths (if any) to clean up
    pub media_files: Vec<String>,
}

impl DisappearingMessage {
    /// Create new disappearing message
    pub fn new(
        message_id: String,
        group_jid: JID,
        sender: JID,
        timer: DisappearingTimer,
        content_type: MessageContentType,
    ) -> Self {
        let sent_at = SystemTime::now();
        let disappear_at = sent_at + Duration::from_secs(timer.duration_seconds());
        
        Self {
            message_id,
            group_jid,
            sender,
            sent_at,
            disappear_at,
            timer,
            deleted: false,
            content_type,
            media_files: Vec::new(),
        }
    }
    
    /// Add media file to cleanup list
    pub fn add_media_file(&mut self, file_path: String) {
        self.media_files.push(file_path);
    }
    
    /// Check if message should disappear now
    pub fn should_disappear(&self) -> bool {
        !self.deleted && SystemTime::now() >= self.disappear_at
    }
    
    /// Get time remaining until disappearing
    pub fn time_remaining(&self) -> Option<Duration> {
        if self.deleted {
            return None;
        }
        
        let now = SystemTime::now();
        if now >= self.disappear_at {
            Some(Duration::from_secs(0))
        } else {
            self.disappear_at.duration_since(now).ok()
        }
    }
    
    /// Mark as deleted
    pub fn mark_deleted(&mut self) {
        self.deleted = true;
    }
    
    /// Get age of message
    pub fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.sent_at)
            .unwrap_or(Duration::from_secs(0))
    }
}

/// Message content type for cleanup purposes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageContentType {
    /// Text message
    Text,
    /// Image message
    Image,
    /// Video message
    Video,
    /// Audio message
    Audio,
    /// Document message
    Document,
    /// Sticker message
    Sticker,
    /// Location message
    Location,
    /// Contact message
    Contact,
}

/// Statistics for disappearing messages in a group
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisappearingStats {
    /// Total messages scheduled to disappear
    pub total_scheduled: u32,
    /// Messages already disappeared
    pub disappeared_count: u32,
    /// Messages pending disappearing
    pub pending_count: u32,
    /// Total media files cleaned up
    pub media_files_cleaned: u32,
    /// Last cleanup time
    pub last_cleanup: Option<SystemTime>,
}

impl DisappearingStats {
    /// Create new stats
    pub fn new() -> Self {
        Self {
            total_scheduled: 0,
            disappeared_count: 0,
            pending_count: 0,
            media_files_cleaned: 0,
            last_cleanup: None,
        }
    }
    
    /// Record a message scheduled for disappearing
    pub fn message_scheduled(&mut self) {
        self.total_scheduled += 1;
        self.pending_count += 1;
    }
    
    /// Record a message disappeared
    pub fn message_disappeared(&mut self, media_file_count: u32) {
        if self.pending_count > 0 {
            self.pending_count -= 1;
        }
        self.disappeared_count += 1;
        self.media_files_cleaned += media_file_count;
    }
    
    /// Update cleanup time
    pub fn cleanup_performed(&mut self) {
        self.last_cleanup = Some(SystemTime::now());
    }
}

impl Default for DisappearingStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Manager for disappearing messages in groups
#[derive(Debug)]
pub struct GroupDisappearingManager {
    /// Group configurations
    group_configs: HashMap<JID, GroupDisappearingConfig>,
    /// Scheduled disappearing messages
    scheduled_messages: HashMap<JID, Vec<DisappearingMessage>>,
    /// Statistics by group
    stats: HashMap<JID, DisappearingStats>,
}

impl GroupDisappearingManager {
    /// Create new disappearing manager
    pub fn new() -> Self {
        Self {
            group_configs: HashMap::new(),
            scheduled_messages: HashMap::new(),
            stats: HashMap::new(),
        }
    }
    
    /// Configure disappearing messages for a group
    pub fn configure_group(
        &mut self,
        group_jid: JID,
        config: GroupDisappearingConfig,
    ) -> Result<()> {
        // Validate configuration
        config.validate()?;
        
        // Store configuration
        self.group_configs.insert(group_jid.clone(), config);
        
        // Initialize message list
        self.scheduled_messages.insert(group_jid.clone(), Vec::new());
        
        // Initialize stats
        self.stats.insert(group_jid.clone(), DisappearingStats::new());
        
        tracing::info!("Configured disappearing messages for group: {}", group_jid);
        
        Ok(())
    }
    
    /// Enable disappearing messages for a group
    pub fn enable_disappearing_messages(
        &mut self,
        group_jid: &JID,
        timer: DisappearingTimer,
        enabled_by: JID,
        group_info: &GroupInfo,
    ) -> Result<()> {
        // Check permissions - only admins can enable/disable
        if !group_info.is_admin(&enabled_by) {
            return Err(Error::Protocol(
                "Only admins can enable disappearing messages".to_string()
            ));
        }
        
        // Validate timer
        timer.validate()?;
        
        // Create or update configuration
        let config = GroupDisappearingConfig::new(enabled_by, timer);
        self.configure_group(group_jid.clone(), config)?;
        
        tracing::info!("Enabled disappearing messages for group: {}", group_jid);
        
        Ok(())
    }
    
    /// Disable disappearing messages for a group
    pub fn disable_disappearing_messages(
        &mut self,
        group_jid: &JID,
        disabled_by: JID,
        group_info: &GroupInfo,
    ) -> Result<()> {
        // Check permissions
        if !group_info.is_admin(&disabled_by) {
            return Err(Error::Protocol(
                "Only admins can disable disappearing messages".to_string()
            ));
        }
        
        // Update configuration
        if let Some(config) = self.group_configs.get_mut(group_jid) {
            config.set_enabled(false, disabled_by);
        } else {
            let config = GroupDisappearingConfig::disabled(disabled_by);
            self.configure_group(group_jid.clone(), config)?;
        }
        
        tracing::info!("Disabled disappearing messages for group: {}", group_jid);
        
        Ok(())
    }
    
    /// Update timer for disappearing messages
    pub fn update_timer(
        &mut self,
        group_jid: &JID,
        new_timer: DisappearingTimer,
        changed_by: JID,
        group_info: &GroupInfo,
    ) -> Result<()> {
        // Check permissions
        let config = self.group_configs.get(group_jid)
            .ok_or_else(|| Error::Protocol("Group not configured for disappearing messages".to_string()))?;
        
        // Check if member can change timer or if user is admin
        if !group_info.is_admin(&changed_by) && !config.allow_member_timer_change {
            return Err(Error::Protocol(
                "Only admins can change disappearing message timer".to_string()
            ));
        }
        
        // Update timer
        let config = self.group_configs.get_mut(group_jid).unwrap();
        config.update_timer(new_timer, changed_by)?;
        
        tracing::info!("Updated disappearing timer for group: {}", group_jid);
        
        Ok(())
    }
    
    /// Schedule a message for disappearing
    pub fn schedule_message(
        &mut self,
        message_id: String,
        group_jid: &JID,
        sender: JID,
        content_type: MessageContentType,
    ) -> Result<()> {
        // Check if disappearing messages are enabled
        let config = self.group_configs.get(group_jid)
            .ok_or_else(|| Error::Protocol("Group not configured for disappearing messages".to_string()))?;
        
        if !config.enabled {
            return Ok(()); // Not enabled, nothing to schedule
        }
        
        // Create disappearing message
        let disappearing_msg = DisappearingMessage::new(
            message_id,
            group_jid.clone(),
            sender,
            config.timer.clone(),
            content_type,
        );
        
        // Add to scheduled messages
        let scheduled = self.scheduled_messages.get_mut(group_jid).unwrap();
        scheduled.push(disappearing_msg);
        
        // Update stats
        let stats = self.stats.get_mut(group_jid).unwrap();
        stats.message_scheduled();
        
        tracing::debug!("Scheduled message {} for disappearing", message_id);
        
        Ok(())
    }
    
    /// Add media file to a scheduled message for cleanup
    pub fn add_media_to_message(
        &mut self,
        group_jid: &JID,
        message_id: &str,
        media_file: String,
    ) -> Result<()> {
        let scheduled = self.scheduled_messages.get_mut(group_jid)
            .ok_or_else(|| Error::Protocol("Group not found".to_string()))?;
        
        let message = scheduled.iter_mut()
            .find(|msg| msg.message_id == message_id)
            .ok_or_else(|| Error::Protocol("Message not found".to_string()))?;
        
        message.add_media_file(media_file);
        
        Ok(())
    }
    
    /// Process disappearing messages (should be called periodically)
    pub async fn process_disappearing_messages(&mut self) -> Result<Vec<(JID, String)>> {
        let mut disappeared_messages = Vec::new();
        
        for (group_jid, scheduled) in &mut self.scheduled_messages {
            let mut messages_to_remove = Vec::new();
            
            for (index, message) in scheduled.iter_mut().enumerate() {
                if message.should_disappear() {
                    // Mark as deleted
                    message.mark_deleted();
                    
                    // Add to cleanup list  
                    disappeared_messages.push((group_jid.clone(), message.message_id.clone()));
                    
                    // Clean up media files
                    for media_file in &message.media_files {
                        if let Err(e) = self.cleanup_media_file(media_file).await {
                            tracing::warn!("Failed to cleanup media file {}: {}", media_file, e);
                        }
                    }
                    
                    messages_to_remove.push(index);
                    
                    // Update stats
                    if let Some(stats) = self.stats.get_mut(group_jid) {
                        stats.message_disappeared(message.media_files.len() as u32);
                    }
                    
                    tracing::info!("Message {} disappeared from group {}", message.message_id, group_jid);
                }
            }
            
            // Remove disappeared messages (in reverse order to maintain indices)
            for index in messages_to_remove.into_iter().rev() {
                scheduled.remove(index);
            }
            
            // Update cleanup time
            if let Some(stats) = self.stats.get_mut(group_jid) {
                stats.cleanup_performed();
            }
        }
        
        Ok(disappeared_messages)
    }
    
    /// Clean up media file
    async fn cleanup_media_file(&self, file_path: &str) -> Result<()> {
        // This would actually delete the file from storage
        // For now, just log the action
        tracing::info!("Cleaning up media file: {}", file_path);
        
        // In a real implementation:
        // tokio::fs::remove_file(file_path).await?;
        
        Ok(())
    }
    
    /// Get configuration for a group
    pub fn get_config(&self, group_jid: &JID) -> Option<&GroupDisappearingConfig> {
        self.group_configs.get(group_jid)
    }
    
    /// Get scheduled messages for a group
    pub fn get_scheduled_messages(&self, group_jid: &JID) -> Option<&Vec<DisappearingMessage>> {
        self.scheduled_messages.get(group_jid)
    }
    
    /// Get statistics for a group
    pub fn get_stats(&self, group_jid: &JID) -> Option<&DisappearingStats> {
        self.stats.get(group_jid)
    }
    
    /// Check if disappearing messages are enabled for a group
    pub fn is_enabled(&self, group_jid: &JID) -> bool {
        self.group_configs.get(group_jid)
            .map(|config| config.enabled)
            .unwrap_or(false)
    }
    
    /// Get pending message count for a group
    pub fn get_pending_count(&self, group_jid: &JID) -> usize {
        self.scheduled_messages.get(group_jid)
            .map(|msgs| msgs.iter().filter(|msg| !msg.deleted).count())
            .unwrap_or(0)
    }
    
    /// Update group settings with disappearing message configuration
    pub fn apply_to_group_settings(
        config: &GroupDisappearingConfig,
        group_settings: &mut GroupSettings,
    ) {
        if config.enabled {
            group_settings.disappearing_messages = Some(DisappearingMessageSettings {
                duration: config.timer.duration_seconds(),
                enabled_by_admin: config.enabled_by_admin,
            });
        } else {
            group_settings.disappearing_messages = None;
        }
    }
}

impl Default for GroupDisappearingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_jid(user: &str) -> JID {
        JID::new(user.to_string(), "s.whatsapp.net".to_string())
    }
    
    fn create_test_group_jid() -> JID {
        JID::new("disappearing_group".to_string(), "g.us".to_string())
    }
    
    fn create_test_group_info() -> GroupInfo {
        let group_jid = create_test_group_jid();
        let admin = create_test_jid("admin");
        let member = create_test_jid("member");
        
        GroupInfo::new(
            group_jid,
            "Test Disappearing Group".to_string(),
            admin.clone(),
            vec![admin.clone(), member],
        )
    }
    
    #[test]
    fn test_disappearing_timer() {
        let timer = DisappearingTimer::OneDay;
        assert_eq!(timer.duration_seconds(), 24 * 60 * 60);
        assert_eq!(timer.description(), "1 day");
        assert!(timer.validate().is_ok());
        
        let custom_timer = DisappearingTimer::Custom(3600);
        assert_eq!(custom_timer.duration_seconds(), 3600);
        assert!(custom_timer.validate().is_ok());
        
        let invalid_timer = DisappearingTimer::Custom(30); // Too short
        assert!(invalid_timer.validate().is_err());
    }
    
    #[test]
    fn test_disappearing_timer_from_seconds() {
        assert_eq!(DisappearingTimer::from_seconds(86400), DisappearingTimer::OneDay);
        assert_eq!(DisappearingTimer::from_seconds(604800), DisappearingTimer::OneWeek);
        assert_eq!(DisappearingTimer::from_seconds(7776000), DisappearingTimer::NinetyDays);
        
        match DisappearingTimer::from_seconds(3600) {
            DisappearingTimer::Custom(3600) => {},
            _ => panic!("Expected custom timer"),
        }
    }
    
    #[test]
    fn test_group_disappearing_config() {
        let admin = create_test_jid("admin");
        let timer = DisappearingTimer::OneWeek;
        
        let config = GroupDisappearingConfig::new(admin.clone(), timer.clone());
        assert!(config.enabled);
        assert_eq!(config.timer, timer);
        assert_eq!(config.enabled_by, admin);
        assert!(config.validate().is_ok());
        
        let disabled_config = GroupDisappearingConfig::disabled(admin.clone());
        assert!(!disabled_config.enabled);
        assert_eq!(disabled_config.enabled_by, admin);
    }
    
    #[test]
    fn test_disappearing_message() {
        let group_jid = create_test_group_jid();
        let sender = create_test_jid("sender");
        let timer = DisappearingTimer::Custom(60); // 1 minute
        
        let mut message = DisappearingMessage::new(
            "msg_123".to_string(),
            group_jid,
            sender,
            timer,
            MessageContentType::Text,
        );
        
        assert_eq!(message.message_id, "msg_123");
        assert!(!message.deleted);
        assert!(message.time_remaining().is_some());
        
        // Add media file
        message.add_media_file("path/to/file.jpg".to_string());
        assert_eq!(message.media_files.len(), 1);
        
        // Mark as deleted
        message.mark_deleted();
        assert!(message.deleted);
        assert!(message.time_remaining().is_none());
    }
    
    #[test]
    fn test_disappearing_stats() {
        let mut stats = DisappearingStats::new();
        assert_eq!(stats.total_scheduled, 0);
        assert_eq!(stats.pending_count, 0);
        
        stats.message_scheduled();
        assert_eq!(stats.total_scheduled, 1);
        assert_eq!(stats.pending_count, 1);
        
        stats.message_disappeared(2);
        assert_eq!(stats.pending_count, 0);
        assert_eq!(stats.disappeared_count, 1);
        assert_eq!(stats.media_files_cleaned, 2);
    }
    
    #[test]
    fn test_group_disappearing_manager() {
        let mut manager = GroupDisappearingManager::new();
        let group_jid = create_test_group_jid();
        let group_info = create_test_group_info();
        let admin = create_test_jid("admin");
        let timer = DisappearingTimer::OneDay;
        
        // Enable disappearing messages
        assert!(manager.enable_disappearing_messages(
            &group_jid,
            timer,
            admin.clone(),
            &group_info
        ).is_ok());
        
        assert!(manager.is_enabled(&group_jid));
        
        // Schedule a message
        assert!(manager.schedule_message(
            "msg_123".to_string(),
            &group_jid,
            admin.clone(),
            MessageContentType::Text,
        ).is_ok());
        
        assert_eq!(manager.get_pending_count(&group_jid), 1);
        
        // Check stats
        let stats = manager.get_stats(&group_jid).unwrap();
        assert_eq!(stats.total_scheduled, 1);
        assert_eq!(stats.pending_count, 1);
    }
    
    #[test]
    fn test_permission_checks() {
        let mut manager = GroupDisappearingManager::new();
        let group_jid = create_test_group_jid();
        let group_info = create_test_group_info();
        let member = create_test_jid("member"); // Not an admin
        let timer = DisappearingTimer::OneDay;
        
        // Non-admin cannot enable disappearing messages
        let result = manager.enable_disappearing_messages(
            &group_jid,
            timer,
            member,
            &group_info
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_group_settings_integration() {
        let admin = create_test_jid("admin");
        let config = GroupDisappearingConfig::new(admin, DisappearingTimer::OneWeek);
        let mut group_settings = GroupSettings::default();
        
        // Apply config to group settings
        GroupDisappearingManager::apply_to_group_settings(&config, &mut group_settings);
        
        assert!(group_settings.disappearing_messages.is_some());
        let dm_settings = group_settings.disappearing_messages.unwrap();
        assert_eq!(dm_settings.duration, 7 * 24 * 60 * 60);
        assert!(dm_settings.enabled_by_admin);
    }
}