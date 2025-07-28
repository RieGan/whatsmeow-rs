/// Group permissions and access control for WhatsApp groups

use crate::{
    error::{Error, Result},
    types::JID,
    group::ParticipantRole,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

/// Group permissions manager
pub struct PermissionManager {
    /// Permission cache by group
    permission_cache: HashMap<JID, CachedPermissions>,
    /// Permission templates
    templates: HashMap<String, PermissionTemplate>,
    /// Configuration
    config: PermissionManagerConfig,
}

/// Configuration for permission manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionManagerConfig {
    /// Whether to enable fine-grained permissions
    pub enable_fine_grained: bool,
    /// Default permission template
    pub default_template: String,
    /// Cache TTL in seconds
    pub cache_ttl: u64,
    /// Whether to audit permission changes
    pub enable_audit: bool,
}

impl Default for PermissionManagerConfig {
    fn default() -> Self {
        Self {
            enable_fine_grained: true,
            default_template: "default".to_string(),
            cache_ttl: 3600, // 1 hour
            enable_audit: true,
        }
    }
}

/// Cached permissions with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPermissions {
    /// Group permissions
    pub permissions: GroupPermissions,
    /// Cache timestamp
    pub cached_at: SystemTime,
    /// Last updated timestamp
    pub last_updated: SystemTime,
}

/// Complete group permissions structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupPermissions {
    /// Group JID
    pub group_jid: JID,
    /// Global group settings
    pub global_settings: GlobalPermissions,
    /// Role-based permissions
    pub role_permissions: HashMap<ParticipantRole, RolePermissions>,
    /// Individual participant overrides
    pub participant_overrides: HashMap<JID, ParticipantPermissions>,
    /// Permission restrictions
    pub restrictions: PermissionRestrictions,
    /// Custom permission rules
    pub custom_rules: Vec<PermissionRule>,
}

/// Global group permissions that apply to the group as a whole
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalPermissions {
    /// Whether group is locked (no new members allowed)
    pub locked: bool,
    /// Whether group is archived
    pub archived: bool,
    /// Whether group is announcement-only
    pub announcement_only: bool,
    /// Whether to allow message forwarding
    pub allow_forwarding: bool,
    /// Whether to allow external sharing
    pub allow_external_sharing: bool,
    /// Whether to show participant phone numbers
    pub show_phone_numbers: bool,
    /// Maximum number of participants
    pub max_participants: usize,
    /// Minimum age requirement
    pub min_age: Option<u8>,
    /// Whether group history is visible to new members
    pub history_visible: bool,
    /// Disappearing messages duration (seconds, None = disabled)
    pub disappearing_duration: Option<u64>,
}

impl Default for GlobalPermissions {
    fn default() -> Self {
        Self {
            locked: false,
            archived: false,
            announcement_only: false,
            allow_forwarding: true,
            allow_external_sharing: true,
            show_phone_numbers: false,
            max_participants: 1024,
            min_age: None,
            history_visible: true,
            disappearing_duration: None,
        }
    }
}

/// Permissions for a specific role
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RolePermissions {
    /// Role this applies to
    pub role: ParticipantRole,
    /// Basic messaging permissions
    pub messaging: MessagingPermissions,
    /// Group management permissions
    pub management: ManagementPermissions,
    /// Media permissions
    pub media: MediaPermissions,
    /// Advanced permissions
    pub advanced: AdvancedPermissions,
}

/// Messaging-related permissions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessagingPermissions {
    /// Can send text messages
    pub send_text: bool,
    /// Can send media messages
    pub send_media: bool,
    /// Can send voice messages
    pub send_voice: bool,
    /// Can send documents
    pub send_documents: bool,
    /// Can send stickers
    pub send_stickers: bool,
    /// Can send location messages
    pub send_location: bool,
    /// Can send contact cards
    pub send_contacts: bool,
    /// Can reply to messages
    pub reply_to_messages: bool,
    /// Can forward messages
    pub forward_messages: bool,
    /// Can edit own messages
    pub edit_own_messages: bool,
    /// Can delete own messages
    pub delete_own_messages: bool,
    /// Can react to messages
    pub react_to_messages: bool,
    /// Can mention participants
    pub mention_participants: bool,
}

impl Default for MessagingPermissions {
    fn default() -> Self {
        Self {
            send_text: true,
            send_media: true,
            send_voice: true,
            send_documents: true,
            send_stickers: true,
            send_location: true,
            send_contacts: true,
            reply_to_messages: true,
            forward_messages: true,
            edit_own_messages: true,
            delete_own_messages: true,
            react_to_messages: true,
            mention_participants: true,
        }
    }
}

/// Group management permissions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagementPermissions {
    /// Can add participants
    pub add_participants: bool,
    /// Can remove participants
    pub remove_participants: bool,
    /// Can promote to admin
    pub promote_participants: bool,
    /// Can demote from admin
    pub demote_participants: bool,
    /// Can edit group info (name, description)
    pub edit_group_info: bool,
    /// Can change group settings
    pub change_group_settings: bool,
    /// Can manage group photo
    pub manage_group_photo: bool,
    /// Can create invite links
    pub create_invite_links: bool,
    /// Can revoke invite links
    pub revoke_invite_links: bool,
    /// Can delete messages of others
    pub delete_others_messages: bool,
    /// Can pin/unpin messages
    pub pin_messages: bool,
    /// Can manage announcements
    pub manage_announcements: bool,
}

impl Default for ManagementPermissions {
    fn default() -> Self {
        Self {
            add_participants: false,
            remove_participants: false,
            promote_participants: false,
            demote_participants: false,
            edit_group_info: false,
            change_group_settings: false,
            manage_group_photo: false,
            create_invite_links: false,
            revoke_invite_links: false,
            delete_others_messages: false,
            pin_messages: false,
            manage_announcements: false,
        }
    }
}

/// Media-related permissions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaPermissions {
    /// Can upload images
    pub upload_images: bool,
    /// Can upload videos
    pub upload_videos: bool,
    /// Can upload audio files
    pub upload_audio: bool,
    /// Can upload documents
    pub upload_documents: bool,
    /// Can share external media links
    pub share_media_links: bool,
    /// Maximum file size for uploads (bytes)
    pub max_upload_size: u64,
    /// Can view media sent by others
    pub view_media: bool,
    /// Can download media
    pub download_media: bool,
}

impl Default for MediaPermissions {
    fn default() -> Self {
        Self {
            upload_images: true,
            upload_videos: true,
            upload_audio: true,
            upload_documents: true,
            share_media_links: true,
            max_upload_size: 16 * 1024 * 1024, // 16MB
            view_media: true,
            download_media: true,
        }
    }
}

/// Advanced permissions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdvancedPermissions {
    /// Can view participant list
    pub view_participants: bool,
    /// Can view participant phone numbers
    pub view_phone_numbers: bool,
    /// Can view message read receipts
    pub view_read_receipts: bool,
    /// Can initiate group calls
    pub initiate_calls: bool,
    /// Can join group calls
    pub join_calls: bool,
    /// Can share live location
    pub share_live_location: bool,
    /// Can use disappearing messages
    pub use_disappearing_messages: bool,
    /// Can export chat history
    pub export_chat: bool,
    /// Can create polls
    pub create_polls: bool,
    /// Can vote in polls
    pub vote_in_polls: bool,
}

impl Default for AdvancedPermissions {
    fn default() -> Self {
        Self {
            view_participants: true,
            view_phone_numbers: false,
            view_read_receipts: true,
            initiate_calls: true,
            join_calls: true,
            share_live_location: true,
            use_disappearing_messages: true,
            export_chat: true,
            create_polls: true,
            vote_in_polls: true,
        }
    }
}

/// Individual participant permissions (overrides role permissions)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticipantPermissions {
    /// Participant JID
    pub participant_jid: JID,
    /// Messaging overrides
    pub messaging_overrides: Option<MessagingPermissions>,
    /// Management overrides
    pub management_overrides: Option<ManagementPermissions>,
    /// Media overrides
    pub media_overrides: Option<MediaPermissions>,
    /// Advanced overrides
    pub advanced_overrides: Option<AdvancedPermissions>,
    /// Whether participant is temporarily muted
    pub muted: bool,
    /// Mute expiration time
    pub mute_expires: Option<SystemTime>,
    /// Custom attributes
    pub custom_attributes: HashMap<String, String>,
}

/// Permission restrictions and limits
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionRestrictions {
    /// Time-based restrictions
    pub time_restrictions: Vec<TimeRestriction>,
    /// Content filters
    pub content_filters: Vec<ContentFilter>,
    /// Rate limits
    pub rate_limits: HashMap<String, RateLimit>,
    /// Banned actions
    pub banned_actions: HashSet<String>,
}

impl Default for PermissionRestrictions {
    fn default() -> Self {
        Self {
            time_restrictions: Vec::new(),
            content_filters: Vec::new(),
            rate_limits: HashMap::new(),
            banned_actions: HashSet::new(),
        }
    }
}

/// Time-based permission restriction
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeRestriction {
    /// Restriction ID
    pub id: String,
    /// Actions this restriction applies to
    pub actions: Vec<String>,
    /// Days of week (0 = Sunday, 6 = Saturday)
    pub days_of_week: Vec<u8>,
    /// Start hour (0-23)
    pub start_hour: u8,
    /// End hour (0-23)
    pub end_hour: u8,
    /// Whether restriction is active during specified time (true) or outside it (false)
    pub active_during: bool,
}

/// Content filter for messages
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentFilter {
    /// Filter ID
    pub id: String,
    /// Filter type
    pub filter_type: ContentFilterType,
    /// Pattern to match
    pub pattern: String,
    /// Action to take when matched
    pub action: FilterAction,
    /// Whether filter is case sensitive
    pub case_sensitive: bool,
}

/// Types of content filters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContentFilterType {
    /// Regex pattern matching
    Regex,
    /// Keyword matching
    Keyword,
    /// URL filtering
    Url,
    /// Phone number filtering
    PhoneNumber,
    /// Profanity filter
    Profanity,
}

/// Actions to take when content filter matches
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterAction {
    /// Block the message
    Block,
    /// Show warning but allow
    Warn,
    /// Replace content
    Replace(String),
    /// Mute participant temporarily
    MuteParticipant(u64), // seconds
}

/// Rate limiting configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateLimit {
    /// Maximum actions per time window
    pub max_actions: u32,
    /// Time window in seconds
    pub window_seconds: u64,
    /// Action to take when limit exceeded
    pub action: RateLimitAction,
}

/// Actions for rate limit violations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RateLimitAction {
    /// Block further actions
    Block,
    /// Slow down actions
    SlowDown,
    /// Temporarily mute
    TempMute(u64), // seconds
    /// Remove from group
    Remove,
}

/// Custom permission rule
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Condition for rule to apply
    pub condition: RuleCondition,
    /// Action to take when condition is met
    pub action: RuleAction,
    /// Whether rule is active
    pub active: bool,
    /// Rule priority (higher = more important)
    pub priority: u32,
}

/// Conditions for permission rules
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RuleCondition {
    /// Participant has specific role
    HasRole(ParticipantRole),
    /// Participant joined before/after time
    JoinedBefore(SystemTime),
    JoinedAfter(SystemTime),
    /// Message count threshold
    MessageCount(u64),
    /// Custom condition
    Custom(String),
}

/// Actions for permission rules
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RuleAction {
    /// Grant specific permission
    Grant(String),
    /// Revoke specific permission
    Revoke(String),
    /// Apply permission template
    ApplyTemplate(String),
    /// Custom action
    Custom(String),
}

/// Permission template for easy management
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionTemplate {
    /// Template ID
    pub id: String,
    /// Template name
    pub name: String,
    /// Description
    pub description: String,
    /// Role permissions
    pub role_permissions: HashMap<ParticipantRole, RolePermissions>,
    /// Global settings
    pub global_settings: GlobalPermissions,
    /// Default restrictions
    pub restrictions: PermissionRestrictions,
}

impl PermissionManager {
    /// Create new permission manager
    pub fn new() -> Self {
        let mut manager = Self::with_config(PermissionManagerConfig::default());
        manager.init_default_templates();
        manager
    }
    
    /// Create permission manager with custom config
    pub fn with_config(config: PermissionManagerConfig) -> Self {
        Self {
            permission_cache: HashMap::new(),
            templates: HashMap::new(),
            config,
        }
    }
    
    /// Initialize default permission templates
    fn init_default_templates(&mut self) {
        // Default template - balanced permissions
        let default_template = PermissionTemplate {
            id: "default".to_string(),
            name: "Default".to_string(),
            description: "Balanced permissions for most groups".to_string(),
            role_permissions: self.create_default_role_permissions(),
            global_settings: GlobalPermissions::default(),
            restrictions: PermissionRestrictions::default(),
        };
        
        // Strict template - limited permissions
        let strict_template = PermissionTemplate {
            id: "strict".to_string(),
            name: "Strict".to_string(),
            description: "Limited permissions for controlled groups".to_string(),
            role_permissions: self.create_strict_role_permissions(),
            global_settings: GlobalPermissions {
                announcement_only: true,
                allow_forwarding: false,
                allow_external_sharing: false,
                show_phone_numbers: false,
                ..GlobalPermissions::default()
            },
            restrictions: PermissionRestrictions::default(),
        };
        
        // Open template - permissive permissions
        let open_template = PermissionTemplate {
            id: "open".to_string(),
            name: "Open".to_string(),
            description: "Permissive permissions for casual groups".to_string(),
            role_permissions: self.create_open_role_permissions(),
            global_settings: GlobalPermissions {
                allow_forwarding: true,
                allow_external_sharing: true,
                show_phone_numbers: true,
                max_participants: 2048,
                ..GlobalPermissions::default()
            },
            restrictions: PermissionRestrictions::default(),
        };
        
        self.templates.insert("default".to_string(), default_template);
        self.templates.insert("strict".to_string(), strict_template);
        self.templates.insert("open".to_string(), open_template);
    }
    
    /// Create default role permissions
    fn create_default_role_permissions(&self) -> HashMap<ParticipantRole, RolePermissions> {
        let mut role_permissions = HashMap::new();
        
        // Creator permissions (full access)
        role_permissions.insert(
            ParticipantRole::Creator,
            RolePermissions {
                role: ParticipantRole::Creator,
                messaging: MessagingPermissions::default(),
                management: ManagementPermissions {
                    add_participants: true,
                    remove_participants: true,
                    promote_participants: true,
                    demote_participants: true,
                    edit_group_info: true,
                    change_group_settings: true,
                    manage_group_photo: true,
                    create_invite_links: true,
                    revoke_invite_links: true,
                    delete_others_messages: true,
                    pin_messages: true,
                    manage_announcements: true,
                },
                media: MediaPermissions::default(),
                advanced: AdvancedPermissions {
                    view_phone_numbers: true,
                    ..AdvancedPermissions::default()
                },
            },
        );
        
        // Admin permissions (most access)
        role_permissions.insert(
            ParticipantRole::Admin,
            RolePermissions {
                role: ParticipantRole::Admin,
                messaging: MessagingPermissions::default(),
                management: ManagementPermissions {
                    add_participants: true,
                    remove_participants: true,
                    promote_participants: false, // Only creator can promote
                    demote_participants: false,  // Only creator can demote
                    edit_group_info: true,
                    change_group_settings: true,
                    manage_group_photo: true,
                    create_invite_links: true,
                    revoke_invite_links: true,
                    delete_others_messages: true,
                    pin_messages: true,
                    manage_announcements: true,
                },
                media: MediaPermissions::default(),
                advanced: AdvancedPermissions::default(),
            },
        );
        
        // Member permissions (basic access)
        role_permissions.insert(
            ParticipantRole::Member,
            RolePermissions {
                role: ParticipantRole::Member,
                messaging: MessagingPermissions::default(),
                management: ManagementPermissions::default(),
                media: MediaPermissions::default(),
                advanced: AdvancedPermissions::default(),
            },
        );
        
        role_permissions
    }
    
    /// Create strict role permissions
    fn create_strict_role_permissions(&self) -> HashMap<ParticipantRole, RolePermissions> {
        let mut role_permissions = self.create_default_role_permissions();
        
        // Restrict member permissions
        if let Some(member_perms) = role_permissions.get_mut(&ParticipantRole::Member) {
            member_perms.messaging.send_media = false;
            member_perms.messaging.send_documents = false;
            member_perms.messaging.forward_messages = false;
            member_perms.media.max_upload_size = 1024 * 1024; // 1MB limit
        }
        
        role_permissions
    }
    
    /// Create open role permissions
    fn create_open_role_permissions(&self) -> HashMap<ParticipantRole, RolePermissions> {
        let mut role_permissions = self.create_default_role_permissions();
        
        // Expand member permissions
        if let Some(member_perms) = role_permissions.get_mut(&ParticipantRole::Member) {
            member_perms.management.add_participants = true;
            member_perms.management.edit_group_info = true;
            member_perms.media.max_upload_size = 64 * 1024 * 1024; // 64MB limit
        }
        
        role_permissions
    }
    
    /// Get permissions for a group
    pub async fn get_permissions(&mut self, group_jid: &JID) -> Result<GroupPermissions> {
        // Check cache first
        if let Some(cached) = self.permission_cache.get(group_jid) {
            if !self.is_cache_expired(cached) {
                return Ok(cached.permissions.clone());
            }
        }
        
        // Fetch or create permissions
        let permissions = self.fetch_or_create_permissions(group_jid).await?;
        
        // Cache the result
        self.cache_permissions(group_jid.clone(), permissions.clone());
        
        Ok(permissions)
    }
    
    /// Fetch or create permissions for a group
    async fn fetch_or_create_permissions(&self, group_jid: &JID) -> Result<GroupPermissions> {
        // In a real implementation, this would fetch from storage
        // For now, create default permissions
        let template = self.templates.get(&self.config.default_template)
            .ok_or_else(|| Error::Protocol("Default template not found".to_string()))?;
        
        let permissions = GroupPermissions {
            group_jid: group_jid.clone(),
            global_settings: template.global_settings.clone(),
            role_permissions: template.role_permissions.clone(),
            participant_overrides: HashMap::new(),
            restrictions: template.restrictions.clone(),
            custom_rules: Vec::new(),
        };
        
        Ok(permissions)
    }
    
    /// Check if a participant has a specific permission
    pub async fn has_permission(
        &mut self,
        group_jid: &JID,
        participant_jid: &JID,
        permission: &str,
        role: ParticipantRole,
    ) -> Result<bool> {
        let permissions = self.get_permissions(group_jid).await?;
        
        // Check for participant-specific override first
        if let Some(override_perms) = permissions.participant_overrides.get(participant_jid) {
            if let Some(result) = self.check_permission_override(&override_perms, permission) {
                return Ok(result);
            }
        }
        
        // Check role-based permissions
        if let Some(role_perms) = permissions.role_permissions.get(&role) {
            let has_perm = self.check_role_permission(&role_perms, permission);
            return Ok(has_perm);
        }
        
        // Default to false if no permissions found
        Ok(false)
    }
    
    /// Check permission in override
    fn check_permission_override(&self, _override_perms: &ParticipantPermissions, _permission: &str) -> Option<bool> {
        // This would check specific permission fields
        // For brevity, just return None to fall back to role permissions
        None
    }
    
    /// Check role-based permission
    fn check_role_permission(&self, role_perms: &RolePermissions, permission: &str) -> bool {
        match permission {
            "send_text" => role_perms.messaging.send_text,
            "send_media" => role_perms.messaging.send_media,
            "add_participants" => role_perms.management.add_participants,
            "remove_participants" => role_perms.management.remove_participants,
            "edit_group_info" => role_perms.management.edit_group_info,
            "delete_others_messages" => role_perms.management.delete_others_messages,
            "pin_messages" => role_perms.management.pin_messages,
            _ => false, // Unknown permission defaults to false
        }
    }
    
    /// Apply permission template to group
    pub async fn apply_template(
        &mut self,
        group_jid: &JID,
        template_id: &str,
    ) -> Result<GroupPermissions> {
        let template = self.templates.get(template_id)
            .ok_or_else(|| Error::Protocol("Template not found".to_string()))?
            .clone();
        
        let permissions = GroupPermissions {
            group_jid: group_jid.clone(),
            global_settings: template.global_settings,
            role_permissions: template.role_permissions,
            participant_overrides: HashMap::new(),
            restrictions: template.restrictions,
            custom_rules: Vec::new(),
        };
        
        // Cache the updated permissions
        self.cache_permissions(group_jid.clone(), permissions.clone());
        
        tracing::info!("Applied template {} to group {}", template_id, group_jid);
        
        Ok(permissions)
    }
    
    /// Update permissions for a group
    pub async fn update_permissions(
        &mut self,
        group_jid: &JID,
        updates: PermissionUpdate,
    ) -> Result<GroupPermissions> {
        let mut permissions = self.get_permissions(group_jid).await?;
        
        // Apply updates
        if let Some(global) = updates.global_settings {
            permissions.global_settings = global;
        }
        
        for (role, role_perms) in updates.role_permissions {
            permissions.role_permissions.insert(role, role_perms);
        }
        
        for (participant, participant_perms) in updates.participant_overrides {
            permissions.participant_overrides.insert(participant, participant_perms);
        }
        
        if let Some(restrictions) = updates.restrictions {
            permissions.restrictions = restrictions;
        }
        
        permissions.custom_rules.extend(updates.custom_rules);
        
        // Cache updated permissions
        self.cache_permissions(group_jid.clone(), permissions.clone());
        
        tracing::info!("Updated permissions for group {}", group_jid);
        
        Ok(permissions)
    }
    
    /// Cache permissions
    fn cache_permissions(&mut self, group_jid: JID, permissions: GroupPermissions) {
        let cached = CachedPermissions {
            permissions,
            cached_at: SystemTime::now(),
            last_updated: SystemTime::now(),
        };
        
        self.permission_cache.insert(group_jid, cached);
    }
    
    /// Check if cache entry is expired
    fn is_cache_expired(&self, cached: &CachedPermissions) -> bool {
        if let Ok(elapsed) = cached.cached_at.elapsed() {
            elapsed.as_secs() > self.config.cache_ttl
        } else {
            true
        }
    }
    
    /// Clear permission cache
    pub fn clear_cache(&mut self) {
        self.permission_cache.clear();
    }
    
    /// Get available templates
    pub fn get_templates(&self) -> &HashMap<String, PermissionTemplate> {
        &self.templates
    }
    
    /// Add custom template
    pub fn add_template(&mut self, template: PermissionTemplate) {
        self.templates.insert(template.id.clone(), template);
    }
}

/// Permission update request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionUpdate {
    /// Global settings update
    pub global_settings: Option<GlobalPermissions>,
    /// Role permission updates
    pub role_permissions: HashMap<ParticipantRole, RolePermissions>,
    /// Participant override updates
    pub participant_overrides: HashMap<JID, ParticipantPermissions>,
    /// Restrictions update
    pub restrictions: Option<PermissionRestrictions>,
    /// Custom rules to add
    pub custom_rules: Vec<PermissionRule>,
}

impl Default for PermissionManager {
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
        JID::new("1234567890".to_string(), "g.us".to_string())
    }
    
    #[tokio::test]
    async fn test_permission_manager_creation() {
        let manager = PermissionManager::new();
        assert_eq!(manager.templates.len(), 3); // default, strict, open
        assert!(manager.permission_cache.is_empty());
    }
    
    #[tokio::test]
    async fn test_get_permissions() {
        let mut manager = PermissionManager::new();
        let group_jid = create_test_group_jid();
        
        let permissions = manager.get_permissions(&group_jid).await.unwrap();
        
        assert_eq!(permissions.group_jid, group_jid);
        assert!(permissions.role_permissions.contains_key(&ParticipantRole::Creator));
        assert!(permissions.role_permissions.contains_key(&ParticipantRole::Admin));
        assert!(permissions.role_permissions.contains_key(&ParticipantRole::Member));
        
        // Should be cached now
        assert_eq!(manager.permission_cache.len(), 1);
    }
    
    #[tokio::test]
    async fn test_has_permission() {
        let mut manager = PermissionManager::new();
        let group_jid = create_test_group_jid();
        let participant_jid = create_test_jid("participant");
        
        // Creator should have all permissions
        let can_add = manager.has_permission(
            &group_jid,
            &participant_jid,
            "add_participants",
            ParticipantRole::Creator,
        ).await.unwrap();
        assert!(can_add);
        
        // Member should not have admin permissions
        let can_remove = manager.has_permission(
            &group_jid,
            &participant_jid,
            "remove_participants",
            ParticipantRole::Member,
        ).await.unwrap();
        assert!(!can_remove);
        
        // Member should have basic messaging permissions
        let can_send_text = manager.has_permission(
            &group_jid,
            &participant_jid,
            "send_text",
            ParticipantRole::Member,
        ).await.unwrap();
        assert!(can_send_text);
    }
    
    #[tokio::test]
    async fn test_apply_template() {
        let mut manager = PermissionManager::new();
        let group_jid = create_test_group_jid();
        
        // Apply strict template
        let permissions = manager.apply_template(&group_jid, "strict").await.unwrap();
        
        assert!(permissions.global_settings.announcement_only);
        assert!(!permissions.global_settings.allow_forwarding);
        assert!(!permissions.global_settings.show_phone_numbers);
        
        // Apply open template
        let permissions = manager.apply_template(&group_jid, "open").await.unwrap();
        
        assert!(!permissions.global_settings.announcement_only);
        assert!(permissions.global_settings.allow_forwarding);
        assert!(permissions.global_settings.show_phone_numbers);
        assert_eq!(permissions.global_settings.max_participants, 2048);
    }
    
    #[tokio::test]
    async fn test_update_permissions() {
        let mut manager = PermissionManager::new();
        let group_jid = create_test_group_jid();
        let participant_jid = create_test_jid("participant");
        
        let mut update = PermissionUpdate::default();
        
        // Update global settings
        update.global_settings = Some(GlobalPermissions {
            locked: true,
            announcement_only: true,
            max_participants: 500,
            ..GlobalPermissions::default()
        });
        
        // Add participant override
        update.participant_overrides.insert(
            participant_jid.clone(),
            ParticipantPermissions {
                participant_jid: participant_jid.clone(),
                messaging_overrides: Some(MessagingPermissions {
                    send_media: false,
                    ..MessagingPermissions::default()
                }),
                management_overrides: None,
                media_overrides: None,
                advanced_overrides: None,
                muted: false,
                mute_expires: None,
                custom_attributes: HashMap::new(),
            },
        );
        
        let updated_permissions = manager.update_permissions(&group_jid, update).await.unwrap();
        
        assert!(updated_permissions.global_settings.locked);
        assert!(updated_permissions.global_settings.announcement_only);
        assert_eq!(updated_permissions.global_settings.max_participants, 500);
        assert!(updated_permissions.participant_overrides.contains_key(&participant_jid));
    }
    
    #[test]
    fn test_permission_templates() {
        let manager = PermissionManager::new();
        let templates = manager.get_templates();
        
        assert!(templates.contains_key("default"));
        assert!(templates.contains_key("strict"));
        assert!(templates.contains_key("open"));
        
        let default_template = templates.get("default").unwrap();
        assert_eq!(default_template.name, "Default");
        
        let strict_template = templates.get("strict").unwrap();
        assert_eq!(strict_template.name, "Strict");
        assert!(strict_template.global_settings.announcement_only);
        
        let open_template = templates.get("open").unwrap();
        assert_eq!(open_template.name, "Open");
        assert_eq!(open_template.global_settings.max_participants, 2048);
    }
    
    #[test]
    fn test_role_permissions() {
        let creator_perms = RolePermissions {
            role: ParticipantRole::Creator,
            messaging: MessagingPermissions::default(),
            management: ManagementPermissions {
                add_participants: true,
                remove_participants: true,
                promote_participants: true,
                demote_participants: true,
                edit_group_info: true,
                change_group_settings: true,
                manage_group_photo: true,
                create_invite_links: true,
                revoke_invite_links: true,
                delete_others_messages: true,
                pin_messages: true,
                manage_announcements: true,
            },
            media: MediaPermissions::default(),
            advanced: AdvancedPermissions::default(),
        };
        
        assert!(creator_perms.management.add_participants);
        assert!(creator_perms.management.remove_participants);
        assert!(creator_perms.management.promote_participants);
        assert!(creator_perms.management.edit_group_info);
        
        let member_perms = RolePermissions {
            role: ParticipantRole::Member,
            messaging: MessagingPermissions::default(),
            management: ManagementPermissions::default(),
            media: MediaPermissions::default(),
            advanced: AdvancedPermissions::default(),
        };
        
        assert!(!member_perms.management.add_participants);
        assert!(!member_perms.management.remove_participants);
        assert!(!member_perms.management.promote_participants);
        assert!(!member_perms.management.edit_group_info);
    }
    
    #[test]
    fn test_permission_restrictions() {
        let time_restriction = TimeRestriction {
            id: "work_hours".to_string(),
            actions: vec!["send_media".to_string()],
            days_of_week: vec![1, 2, 3, 4, 5], // Monday to Friday
            start_hour: 9,
            end_hour: 17,
            active_during: false, // Restriction active outside work hours
        };
        
        assert_eq!(time_restriction.days_of_week.len(), 5);
        assert_eq!(time_restriction.start_hour, 9);
        assert_eq!(time_restriction.end_hour, 17);
        assert!(!time_restriction.active_during);
        
        let rate_limit = RateLimit {
            max_actions: 10,
            window_seconds: 60,
            action: RateLimitAction::SlowDown,
        };
        
        assert_eq!(rate_limit.max_actions, 10);
        assert_eq!(rate_limit.window_seconds, 60);
        assert_eq!(rate_limit.action, RateLimitAction::SlowDown);
    }
    
    #[test]
    fn test_content_filters() {
        let profanity_filter = ContentFilter {
            id: "profanity".to_string(),
            filter_type: ContentFilterType::Profanity,
            pattern: "bad_words_pattern".to_string(),
            action: FilterAction::Replace("***".to_string()),
            case_sensitive: false,
        };
        
        assert_eq!(profanity_filter.filter_type, ContentFilterType::Profanity);
        match &profanity_filter.action {
            FilterAction::Replace(replacement) => assert_eq!(replacement, "***"),
            _ => panic!("Wrong filter action"),
        }
        
        let url_filter = ContentFilter {
            id: "no_external_links".to_string(),
            filter_type: ContentFilterType::Url,
            pattern: r"https?://.*".to_string(),
            action: FilterAction::Block,
            case_sensitive: false,
        };
        
        assert_eq!(url_filter.filter_type, ContentFilterType::Url);
        assert_eq!(url_filter.action, FilterAction::Block);
    }
}