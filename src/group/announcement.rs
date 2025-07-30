/// WhatsApp Announcement Groups implementation
/// 
/// Announcement groups are special groups where only administrators can send messages.
/// Regular members can only read messages but cannot send them. This is commonly used
/// for broadcasting information, updates, and announcements to large audiences.

use crate::{
    error::{Error, Result},
    types::JID,
    group::{GroupInfo, GroupSettings, ParticipantPermission},
};
use serde::{Deserialize, Serialize};
use std::{time::SystemTime, collections::HashMap};

/// Announcement group configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnouncementGroupConfig {
    /// Whether only admins can send messages (core announcement feature)
    pub admin_only_messaging: bool,
    /// Whether members can react to messages
    pub allow_member_reactions: bool,
    /// Whether members can reply to messages (in private)
    pub allow_member_replies: bool,
    /// Whether to show member list to participants
    pub show_member_list: bool,
    /// Whether to allow members to forward messages
    pub allow_message_forwarding: bool,
    /// Auto-delete old announcements after this duration (in seconds)
    pub auto_delete_duration: Option<u64>,
    /// Maximum number of announcements to keep
    pub max_announcements: Option<u32>,
}

impl Default for AnnouncementGroupConfig {
    fn default() -> Self {
        Self {
            admin_only_messaging: true,
            allow_member_reactions: true,
            allow_member_replies: false,
            show_member_list: false,
            allow_message_forwarding: true,
            auto_delete_duration: None,
            max_announcements: None,
        }
    }
}

impl AnnouncementGroupConfig {
    /// Create strict announcement config (minimal member privileges)
    pub fn strict() -> Self {
        Self {
            admin_only_messaging: true,
            allow_member_reactions: false,
            allow_member_replies: false,
            show_member_list: false,
            allow_message_forwarding: false,
            auto_delete_duration: None,
            max_announcements: Some(100),
        }
    }
    
    /// Create permissive announcement config (more member privileges)
    pub fn permissive() -> Self {
        Self {
            admin_only_messaging: true,
            allow_member_reactions: true,
            allow_member_replies: true,
            show_member_list: true,
            allow_message_forwarding: true,
            auto_delete_duration: Some(30 * 24 * 60 * 60), // 30 days
            max_announcements: Some(500),
        }
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Admin-only messaging must be enabled for announcement groups
        if !self.admin_only_messaging {
            return Err(Error::Protocol(
                "Announcement groups require admin-only messaging".to_string()
            ));
        }
        
        // Validate auto-delete duration
        if let Some(duration) = self.auto_delete_duration {
            if duration < 60 {  // Minimum 1 minute
                return Err(Error::Protocol(
                    "Auto-delete duration too short".to_string()
                ));
            }
            if duration > 365 * 24 * 60 * 60 {  // Maximum 1 year
                return Err(Error::Protocol(
                    "Auto-delete duration too long".to_string()
                ));
            }
        }
        
        // Validate max announcements
        if let Some(max) = self.max_announcements {
            if max == 0 {
                return Err(Error::Protocol(
                    "Max announcements must be greater than 0".to_string()
                ));
            }
            if max > 10000 {
                return Err(Error::Protocol(
                    "Max announcements limit too high".to_string()
                ));
            }
        }
        
        Ok(())
    }
}

/// Announcement message metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnouncementMessage {
    /// Message ID
    pub id: String,
    /// Message content
    pub content: String,
    /// Sender (must be admin)
    pub sender: JID,
    /// Timestamp when announced
    pub announced_at: SystemTime,
    /// Number of members who read the announcement
    pub read_count: u32,
    /// Number of reactions received
    pub reaction_count: u32,
    /// Whether this is a pinned announcement
    pub pinned: bool,
    /// Priority level (high, normal, low)
    pub priority: AnnouncementPriority,
    /// Optional expiration time
    pub expires_at: Option<SystemTime>,
    /// Optional media attachments
    pub media_attachments: Vec<String>,
    /// Message thread/category
    pub category: Option<String>,
}

impl AnnouncementMessage {
    /// Create new announcement message
    pub fn new(id: String, content: String, sender: JID) -> Self {
        Self {
            id,
            content,
            sender,
            announced_at: SystemTime::now(),
            read_count: 0,
            reaction_count: 0,
            pinned: false,
            priority: AnnouncementPriority::Normal,
            expires_at: None,
            media_attachments: Vec::new(),
            category: None,
        }
    }
    
    /// Set priority
    pub fn with_priority(mut self, priority: AnnouncementPriority) -> Self {
        self.priority = priority;
        self
    }
    
    /// Set expiration
    pub fn with_expiration(mut self, expires_at: SystemTime) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
    
    /// Set as pinned
    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }
    
    /// Add media attachment
    pub fn with_media(mut self, media_id: String) -> Self {
        self.media_attachments.push(media_id);
        self
    }
    
    /// Set category
    pub fn with_category(mut self, category: String) -> Self {
        self.category = Some(category);
        self
    }
    
    /// Check if announcement has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            SystemTime::now() > expires_at
        } else {
            false
        }
    }
    
    /// Increment read count
    pub fn mark_read(&mut self) {
        self.read_count += 1;
    }
    
    /// Increment reaction count
    pub fn add_reaction(&mut self) {
        self.reaction_count += 1;
    }
    
    /// Remove reaction
    pub fn remove_reaction(&mut self) {
        if self.reaction_count > 0 {
            self.reaction_count -= 1;
        }
    }
}

/// Announcement priority levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnnouncementPriority {
    /// High priority - urgent announcements
    High,
    /// Normal priority - regular announcements  
    Normal,
    /// Low priority - informational announcements
    Low,
}

impl Default for AnnouncementPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Member's announcement interaction tracking
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberAnnouncementStatus {
    /// Member JID
    pub member: JID,
    /// Last announcement they read
    pub last_read_announcement: Option<String>,
    /// Announcements they've reacted to
    pub reacted_announcements: Vec<String>,
    /// Whether they have notifications enabled
    pub notifications_enabled: bool,
    /// Preferred announcement categories
    pub subscribed_categories: Vec<String>,
}

impl MemberAnnouncementStatus {
    /// Create new member status
    pub fn new(member: JID) -> Self {
        Self {
            member,
            last_read_announcement: None,
            reacted_announcements: Vec::new(),
            notifications_enabled: true,
            subscribed_categories: Vec::new(),
        }
    }
    
    /// Mark announcement as read
    pub fn mark_announcement_read(&mut self, announcement_id: String) {
        self.last_read_announcement = Some(announcement_id);
    }
    
    /// Add reaction to announcement
    pub fn add_reaction(&mut self, announcement_id: String) {
        if !self.reacted_announcements.contains(&announcement_id) {
            self.reacted_announcements.push(announcement_id);
        }
    }
    
    /// Remove reaction from announcement
    pub fn remove_reaction(&mut self, announcement_id: &str) {
        self.reacted_announcements.retain(|id| id != announcement_id);
    }
    
    /// Check if member has reacted to announcement
    pub fn has_reacted(&self, announcement_id: &str) -> bool {
        self.reacted_announcements.contains(&announcement_id.to_string())
    }
    
    /// Subscribe to category
    pub fn subscribe_to_category(&mut self, category: String) {
        if !self.subscribed_categories.contains(&category) {
            self.subscribed_categories.push(category);
        }
    }
    
    /// Unsubscribe from category
    pub fn unsubscribe_from_category(&mut self, category: &str) {
        self.subscribed_categories.retain(|c| c != category);
    }
}

/// Announcement group manager
#[derive(Debug)]
pub struct AnnouncementGroupManager {
    /// Group configurations
    group_configs: HashMap<JID, AnnouncementGroupConfig>,
    /// Announcements by group
    announcements: HashMap<JID, Vec<AnnouncementMessage>>,
    /// Member status tracking
    member_status: HashMap<JID, HashMap<JID, MemberAnnouncementStatus>>,
    /// Pinned announcements by group
    pinned_announcements: HashMap<JID, Vec<String>>,
}

impl AnnouncementGroupManager {
    /// Create new announcement group manager
    pub fn new() -> Self {
        Self {
            group_configs: HashMap::new(),
            announcements: HashMap::new(),
            member_status: HashMap::new(),
            pinned_announcements: HashMap::new(),
        }
    }
    
    /// Configure group as announcement group
    pub fn configure_announcement_group(
        &mut self,
        group_jid: JID,
        config: AnnouncementGroupConfig,
    ) -> Result<()> {
        // Validate configuration
        config.validate()?;
        
        // Store configuration
        self.group_configs.insert(group_jid.clone(), config);
        
        // Initialize announcements list
        self.announcements.insert(group_jid.clone(), Vec::new());
        
        // Initialize member status tracking
        self.member_status.insert(group_jid.clone(), HashMap::new());
        
        // Initialize pinned announcements
        self.pinned_announcements.insert(group_jid.clone(), Vec::new());
        
        tracing::info!("Configured announcement group: {}", group_jid);
        
        Ok(())
    }
    
    /// Check if group is configured as announcement group
    pub fn is_announcement_group(&self, group_jid: &JID) -> bool {
        self.group_configs.contains_key(group_jid)
    }
    
    /// Post announcement to group
    pub fn post_announcement(
        &mut self,
        group_jid: &JID,
        announcement: AnnouncementMessage,
        sender: &JID,group_info: &GroupInfo,
    ) -> Result<String> {
        // Check if group is announcement group
        if !self.is_announcement_group(group_jid) {
            return Err(Error::Protocol(
                "Group is not configured as announcement group".to_string()
            ));
        }
        
        // Check if sender is admin
        if !group_info.is_admin(sender) {
            return Err(Error::Protocol(
                "Only admins can post announcements".to_string()
            ));
        }
        
        // Get group config
        let config = self.group_configs.get(group_jid).unwrap();
        
        // Check announcement limits
        let announcements = self.announcements.get_mut(group_jid).unwrap();
        if let Some(max) = config.max_announcements {
            if announcements.len() >= max as usize {
                // Remove oldest non-pinned announcement
                self.cleanup_old_announcements(group_jid, 1)?;
            }
        }
        
        // Add announcement
        let announcement_id = announcement.id.clone();
        announcements.push(announcement);
        
        // Sort by timestamp (newest first)
        announcements.sort_by(|a, b| b.announced_at.cmp(&a.announced_at));
        
        tracing::info!("Posted announcement {} to group {}", announcement_id, group_jid);
        
        Ok(announcement_id)
    }
    
    /// Pin an announcement
    pub fn pin_announcement(
        &mut self,
        group_jid: &JID,
        announcement_id: &str,
        sender: &JID,
        group_info: &GroupInfo,
    ) -> Result<()> {
        // Check if group is announcement group
        if !self.is_announcement_group(group_jid) {
            return Err(Error::Protocol(
                "Group is not configured as announcement group".to_string()
            ));
        }
        
        // Check if sender is admin
        if !group_info.is_admin(sender) {
            return Err(Error::Protocol(
                "Only admins can pin announcements".to_string()
            ));
        }
        
        // Find and pin announcement
        let announcements = self.announcements.get_mut(group_jid).unwrap();
        let announcement = announcements.iter_mut()
            .find(|a| a.id == announcement_id)
            .ok_or_else(|| Error::Protocol("Announcement not found".to_string()))?;
        
        announcement.pinned = true;
        
        // Add to pinned list
        let pinned = self.pinned_announcements.get_mut(group_jid).unwrap();
        if !pinned.contains(&announcement_id.to_string()) {
            pinned.push(announcement_id.to_string());
        }
        
        tracing::info!("Pinned announcement {} in group {}", announcement_id, group_jid);
        
        Ok(())
    }
    
    /// Unpin an announcement
    pub fn unpin_announcement(
        &mut self,
        group_jid: &JID,
        announcement_id: &str,
        sender: &JID,
        group_info: &GroupInfo,
    ) -> Result<()> {
        // Check if group is announcement group
        if !self.is_announcement_group(group_jid) {
            return Err(Error::Protocol(
                "Group is not configured as announcement group".to_string()
            ));
        }
        
        // Check if sender is admin
        if !group_info.is_admin(sender) {
            return Err(Error::Protocol(
                "Only admins can unpin announcements".to_string()
            ));
        }
        
        // Find and unpin announcement
        let announcements = self.announcements.get_mut(group_jid).unwrap();
        let announcement = announcements.iter_mut()
            .find(|a| a.id == announcement_id)
            .ok_or_else(|| Error::Protocol("Announcement not found".to_string()))?;
        
        announcement.pinned = false;
        
        // Remove from pinned list
        let pinned = self.pinned_announcements.get_mut(group_jid).unwrap();
        pinned.retain(|id| id != announcement_id);
        
        tracing::info!("Unpinned announcement {} in group {}", announcement_id, group_jid);
        
        Ok(())
    }
    
    /// Mark announcement as read by member
    pub fn mark_announcement_read(
        &mut self,
        group_jid: &JID,
        announcement_id: &str,
        member: &JID,
    ) -> Result<()> {
        // Get or create member status
        let group_members = self.member_status.get_mut(group_jid)
            .ok_or_else(|| Error::Protocol("Group not found".to_string()))?;
        
        let member_status = group_members.entry(member.clone())
            .or_insert_with(|| MemberAnnouncementStatus::new(member.clone()));
        
        // Mark as read
        member_status.mark_announcement_read(announcement_id.to_string());
        
        // Increment read count on announcement
        let announcements = self.announcements.get_mut(group_jid).unwrap();
        if let Some(announcement) = announcements.iter_mut().find(|a| a.id == announcement_id) {
            announcement.mark_read();
        }
        
        Ok(())
    }
    
    /// Add reaction to announcement
    pub fn add_reaction_to_announcement(
        &mut self,
        group_jid: &JID,
        announcement_id: &str,
        member: &JID,
    ) -> Result<()> {
        // Check if reactions are allowed
        let config = self.group_configs.get(group_jid)
            .ok_or_else(|| Error::Protocol("Group not found".to_string()))?;
        
        if !config.allow_member_reactions {
            return Err(Error::Protocol(
                "Member reactions not allowed in this group".to_string()
            ));
        }
        
        // Get or create member status
        let group_members = self.member_status.get_mut(group_jid).unwrap();
        let member_status = group_members.entry(member.clone())
            .or_insert_with(|| MemberAnnouncementStatus::new(member.clone()));
        
        // Add reaction
        member_status.add_reaction(announcement_id.to_string());
        
        // Increment reaction count on announcement
        let announcements = self.announcements.get_mut(group_jid).unwrap();
        if let Some(announcement) = announcements.iter_mut().find(|a| a.id == announcement_id) {
            announcement.add_reaction();
        }
        
        Ok(())
    }
    
    /// Remove reaction from announcement
    pub fn remove_reaction_from_announcement(
        &mut self,
        group_jid: &JID,
        announcement_id: &str,
        member: &JID,
    ) -> Result<()> {
        // Get member status
        let group_members = self.member_status.get_mut(group_jid)
            .ok_or_else(|| Error::Protocol("Group not found".to_string()))?;
        
        let member_status = group_members.get_mut(member)
            .ok_or_else(|| Error::Protocol("Member not found".to_string()))?;
        
        // Remove reaction
        member_status.remove_reaction(announcement_id);
        
        // Decrement reaction count on announcement
        let announcements = self.announcements.get_mut(group_jid).unwrap();
        if let Some(announcement) = announcements.iter_mut().find(|a| a.id == announcement_id) {
            announcement.remove_reaction();
        }
        
        Ok(())
    }
    
    /// Get announcements for group
    pub fn get_announcements(&self, group_jid: &JID) -> Option<&Vec<AnnouncementMessage>> {
        self.announcements.get(group_jid)
    }
    
    /// Get pinned announcements for group
    pub fn get_pinned_announcements(&self, group_jid: &JID) -> Vec<&AnnouncementMessage> {
        if let (Some(announcements), Some(pinned_ids)) = (
            self.announcements.get(group_jid),
            self.pinned_announcements.get(group_jid)
        ) {
            announcements.iter()
                .filter(|a| pinned_ids.contains(&a.id))
                .collect()
        } else {
            Vec::new()
        }
    }
    
    /// Clean up old announcements based on configuration
    pub fn cleanup_old_announcements(&mut self, group_jid: &JID, count: usize) -> Result<()> {
        let announcements = self.announcements.get_mut(group_jid)
            .ok_or_else(|| Error::Protocol("Group not found".to_string()))?;
        
        // Sort by timestamp (oldest first for removal)
        announcements.sort_by(|a, b| a.announced_at.cmp(&b.announced_at));
        
        // Remove oldest non-pinned announcements
        let mut removed = 0;
        announcements.retain(|announcement| {
            if removed < count && !announcement.pinned {
                removed += 1;
                false
            } else {
                true
            }
        });
        
        // Sort back to newest first
        announcements.sort_by(|a, b| b.announced_at.cmp(&a.announced_at));
        
        tracing::info!("Cleaned up {} old announcements from group {}", removed, group_jid);
        
        Ok(())
    }
    
    /// Clean up expired announcements
    pub fn cleanup_expired_announcements(&mut self, group_jid: &JID) -> Result<()> {
        let announcements = self.announcements.get_mut(group_jid)
            .ok_or_else(|| Error::Protocol("Group not found".to_string()))?;
        
        let initial_count = announcements.len();
        announcements.retain(|announcement| !announcement.is_expired());
        let removed_count = initial_count - announcements.len();
        
        if removed_count > 0 {
            tracing::info!("Cleaned up {} expired announcements from group {}", removed_count, group_jid);
        }
        
        Ok(())
    }
    
    /// Get member announcement status
    pub fn get_member_status(&self, group_jid: &JID, member: &JID) -> Option<&MemberAnnouncementStatus> {
        self.member_status.get(group_jid)?.get(member)
    }
    
    /// Update group settings to enable announcement mode
    pub fn convert_to_announcement_group(group_settings: &mut GroupSettings) {
        group_settings.announcement_only = true;
        group_settings.send_messages = ParticipantPermission::AdminsOnly;
    }
    
    /// Update group settings to disable announcement mode
    pub fn convert_from_announcement_group(group_settings: &mut GroupSettings) {
        group_settings.announcement_only = false;
        group_settings.send_messages = ParticipantPermission::Everyone;
    }
}

impl Default for AnnouncementGroupManager {
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
        JID::new("announcement_group".to_string(), "g.us".to_string())
    }
    
    fn create_test_group_info() -> GroupInfo {
        let group_jid = create_test_group_jid();
        let admin = create_test_jid("admin");
        let member = create_test_jid("member");
        
        GroupInfo::new(
            group_jid,
            "Test Announcement Group".to_string(),
            admin.clone(),
            vec![admin.clone(), member],
        )
    }
    
    #[test]
    fn test_announcement_group_config() {
        let config = AnnouncementGroupConfig::default();
        assert!(config.admin_only_messaging);
        assert!(config.allow_member_reactions);
        assert!(config.validate().is_ok());
        
        let strict_config = AnnouncementGroupConfig::strict();
        assert!(!strict_config.allow_member_reactions);
        assert!(!strict_config.allow_member_replies);
        
        let permissive_config = AnnouncementGroupConfig::permissive();
        assert!(permissive_config.allow_member_reactions);
        assert!(permissive_config.allow_member_replies);
    }
    
    #[test]
    fn test_announcement_message() {
        let sender = create_test_jid("admin");
        let mut message = AnnouncementMessage::new(
            "msg_123".to_string(),
            "Test announcement".to_string(),
            sender,
        );
        
        assert_eq!(message.content, "Test announcement");
        assert!(!message.pinned);
        assert_eq!(message.read_count, 0);
        assert_eq!(message.reaction_count, 0);
        
        // Test interactions
        message.mark_read();
        assert_eq!(message.read_count, 1);
        
        message.add_reaction();
        assert_eq!(message.reaction_count, 1);
        
        message.remove_reaction();
        assert_eq!(message.reaction_count, 0);
    }
    
    #[test]
    fn test_member_announcement_status() {
        let member = create_test_jid("member");
        let mut status = MemberAnnouncementStatus::new(member.clone());
        
        assert_eq!(status.member, member);
        assert!(status.notifications_enabled);
        assert!(status.subscribed_categories.is_empty());
        
        // Test reactions
        status.add_reaction("msg_123".to_string());
        assert!(status.has_reacted("msg_123"));
        
        status.remove_reaction("msg_123");
        assert!(!status.has_reacted("msg_123"));
        
        // Test categories
        status.subscribe_to_category("updates".to_string());
        assert!(status.subscribed_categories.contains(&"updates".to_string()));
        
        status.unsubscribe_from_category("updates");
        assert!(!status.subscribed_categories.contains(&"updates".to_string()));
    }
    
    #[test]
    fn test_announcement_group_manager() {
        let mut manager = AnnouncementGroupManager::new();
        let group_jid = create_test_group_jid();
        let config = AnnouncementGroupConfig::default();
        
        // Configure group
        assert!(manager.configure_announcement_group(group_jid.clone(), config).is_ok());
        assert!(manager.is_announcement_group(&group_jid));
        
        // Test posting announcement
        let group_info = create_test_group_info();
        let admin = create_test_jid("admin");
        let announcement = AnnouncementMessage::new(
            "msg_123".to_string(),
            "Test announcement".to_string(),
            admin.clone(),
        );
        
        let result = manager.post_announcement(&group_jid, announcement, &admin, &group_info);
        assert!(result.is_ok());
        
        // Check announcements
        let announcements = manager.get_announcements(&group_jid).unwrap();
        assert_eq!(announcements.len(), 1);
        assert_eq!(announcements[0].content, "Test announcement");
    }
    
    #[test]
    fn test_pin_unpin_announcements() {
        let mut manager = AnnouncementGroupManager::new();
        let group_jid = create_test_group_jid();
        let config = AnnouncementGroupConfig::default();
        let group_info = create_test_group_info();
        let admin = create_test_jid("admin");
        
        // Setup
        manager.configure_announcement_group(group_jid.clone(), config).unwrap();
        let announcement = AnnouncementMessage::new(
            "msg_123".to_string(),
            "Test announcement".to_string(),
            admin.clone(),
        );
        manager.post_announcement(&group_jid, announcement, &admin, &group_info).unwrap();
        
        // Pin announcement
        assert!(manager.pin_announcement(&group_jid, "msg_123", &admin, &group_info).is_ok());
        let pinned = manager.get_pinned_announcements(&group_jid);
        assert_eq!(pinned.len(), 1);
        
        // Unpin announcement
        assert!(manager.unpin_announcement(&group_jid, "msg_123", &admin, &group_info).is_ok());
        let pinned = manager.get_pinned_announcements(&group_jid);
        assert_eq!(pinned.len(), 0);
    }
    
    #[test]
    fn test_group_settings_conversion() {
        let mut settings = GroupSettings::default();
        assert!(!settings.announcement_only);
        assert_eq!(settings.send_messages, ParticipantPermission::Everyone);
        
        // Convert to announcement group
        AnnouncementGroupManager::convert_to_announcement_group(&mut settings);
        assert!(settings.announcement_only);
        assert_eq!(settings.send_messages, ParticipantPermission::AdminsOnly);
        
        // Convert back
        AnnouncementGroupManager::convert_from_announcement_group(&mut settings);
        assert!(!settings.announcement_only);
        assert_eq!(settings.send_messages, ParticipantPermission::Everyone);
    }
}