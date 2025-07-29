/// Contact synchronization system for WhatsApp App State
/// 
/// Handles synchronization of contact information including:
/// - Contact details (name, phone number, avatar)
/// - WhatsApp status and verification
/// - Contact organization and grouping
/// - Contact privacy settings

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

/// Contact information synchronized with WhatsApp
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Contact {
    /// Contact's JID
    pub jid: JID,
    /// Display name
    pub name: String,
    /// Push name (name set by the contact)
    pub push_name: Option<String>,
    /// Contact's phone number
    pub phone_number: String,
    /// Avatar image data
    pub avatar: Option<Vec<u8>>,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Whether contact is on WhatsApp
    pub is_whatsapp_user: bool,
    /// Contact verification status
    pub verified: bool,
    /// Contact status message
    pub status: Option<String>,
    /// Last seen timestamp
    pub last_seen: Option<SystemTime>,
    /// Whether contact is blocked
    pub blocked: bool,
    /// Whether contact is muted
    pub muted: bool,
    /// Contact labels/groups
    pub labels: Vec<String>,
    /// Contact's business information
    pub business_info: Option<BusinessInfo>,
    /// Last time contact was updated
    pub last_updated: SystemTime,
    /// Sync version for conflict resolution
    pub version: AppStateVersion,
}

/// Business contact information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BusinessInfo {
    /// Business name
    pub business_name: String,
    /// Business category
    pub category: String,
    /// Business description
    pub description: Option<String>,
    /// Business website
    pub website: Option<String>,
    /// Business email
    pub email: Option<String>,
    /// Business address
    pub address: Option<String>,
    /// Business hours
    pub hours: Option<String>,
    /// Verified business status
    pub verified: bool,
}

/// Contact synchronization manager
pub struct ContactSync {
    /// Contact storage
    contacts: Arc<RwLock<HashMap<JID, Contact>>>,
    /// Contact name cache (phone -> name)
    name_cache: Arc<RwLock<HashMap<String, String>>>,
    /// Contact status cache
    status_cache: Arc<RwLock<HashMap<JID, ContactStatus>>>,
}

/// Contact status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactStatus {
    /// Online status
    pub online: bool,
    /// Last seen timestamp
    pub last_seen: Option<SystemTime>,
    /// Typing status
    pub typing: bool,
    /// Recording status (voice message)
    pub recording: bool,
    /// Status message
    pub status_text: Option<String>,
}

/// Contact search and filtering
#[derive(Debug, Clone)]
pub struct ContactFilter {
    /// Filter by name (partial match)
    pub name_contains: Option<String>,
    /// Filter by WhatsApp users only
    pub whatsapp_users_only: bool,
    /// Filter by blocked status
    pub blocked: Option<bool>,
    /// Filter by verification status
    pub verified: Option<bool>,
    /// Filter by labels
    pub has_labels: Vec<String>,
    /// Filter by business accounts
    pub business_only: bool,
}

impl ContactSync {
    /// Create a new contact sync manager
    pub fn new() -> Self {
        Self {
            contacts: Arc::new(RwLock::new(HashMap::new())),
            name_cache: Arc::new(RwLock::new(HashMap::new())),
            status_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add or update a contact
    pub async fn update_contact(&self, contact: Contact) -> Result<()> {
        let jid = contact.jid.clone();
        let phone = contact.phone_number.clone();
        let name = contact.name.clone();

        // Update contact storage
        {
            let mut contacts = self.contacts.write().await;
            contacts.insert(jid.clone(), contact);
        }

        // Update name cache
        {
            let mut cache = self.name_cache.write().await;
            cache.insert(phone, name);
        }

        Ok(())
    }

    /// Get contact by JID
    pub async fn get_contact(&self, jid: &JID) -> Option<Contact> {
        let contacts = self.contacts.read().await;
        contacts.get(jid).cloned()
    }

    /// Get contact by phone number
    pub async fn get_contact_by_phone(&self, phone: &str) -> Option<Contact> {
        let contacts = self.contacts.read().await;
        contacts.values()
            .find(|c| c.phone_number == phone)
            .cloned()
    }

    /// Get contact name by phone (cached)
    pub async fn get_cached_name(&self, phone: &str) -> Option<String> {
        let cache = self.name_cache.read().await;
        cache.get(phone).cloned()
    }

    /// Get all contacts
    pub async fn get_all_contacts(&self) -> Vec<Contact> {
        let contacts = self.contacts.read().await;
        contacts.values().cloned().collect()
    }

    /// Search contacts with filter
    pub async fn search_contacts(&self, filter: ContactFilter) -> Vec<Contact> {
        let contacts = self.contacts.read().await;
        
        contacts.values()
            .filter(|contact| {
                // Name filter
                if let Some(name_filter) = &filter.name_contains {
                    if !contact.name.to_lowercase().contains(&name_filter.to_lowercase()) &&
                       !contact.push_name.as_ref().unwrap_or(&String::new()).to_lowercase().contains(&name_filter.to_lowercase()) {
                        return false;
                    }
                }

                // WhatsApp users only
                if filter.whatsapp_users_only && !contact.is_whatsapp_user {
                    return false;
                }

                // Blocked filter
                if let Some(blocked) = filter.blocked {
                    if contact.blocked != blocked {
                        return false;
                    }
                }

                // Verified filter
                if let Some(verified) = filter.verified {
                    if contact.verified != verified {
                        return false;
                    }
                }

                // Labels filter
                if !filter.has_labels.is_empty() {
                    let has_any_label = filter.has_labels.iter()
                        .any(|label| contact.labels.contains(label));
                    if !has_any_label {
                        return false;
                    }
                }

                // Business filter
                if filter.business_only && contact.business_info.is_none() {
                    return false;
                }

                true
            })
            .cloned()
            .collect()
    }

    /// Update contact status
    pub async fn update_contact_status(&self, jid: &JID, status: ContactStatus) {
        let mut cache = self.status_cache.write().await;
        cache.insert(jid.clone(), status);
    }

    /// Get contact status
    pub async fn get_contact_status(&self, jid: &JID) -> Option<ContactStatus> {
        let cache = self.status_cache.read().await;
        cache.get(jid).cloned()
    }

    /// Block a contact
    pub async fn block_contact(&self, jid: &JID) -> Result<()> {
        let mut contacts = self.contacts.write().await;
        if let Some(contact) = contacts.get_mut(jid) {
            contact.blocked = true;
            contact.last_updated = SystemTime::now();
            contact.version.timestamp = SystemTime::now();
            contact.version.hash = self.calculate_contact_hash(contact);
        }
        Ok(())
    }

    /// Unblock a contact
    pub async fn unblock_contact(&self, jid: &JID) -> Result<()> {
        let mut contacts = self.contacts.write().await;
        if let Some(contact) = contacts.get_mut(jid) {
            contact.blocked = false;
            contact.last_updated = SystemTime::now();
            contact.version.timestamp = SystemTime::now();
            contact.version.hash = self.calculate_contact_hash(contact);
        }
        Ok(())
    }

    /// Add label to contact
    pub async fn add_label_to_contact(&self, jid: &JID, label: String) -> Result<()> {
        let mut contacts = self.contacts.write().await;
        if let Some(contact) = contacts.get_mut(jid) {
            if !contact.labels.contains(&label) {
                contact.labels.push(label);
                contact.last_updated = SystemTime::now();
                contact.version.timestamp = SystemTime::now();
                contact.version.hash = self.calculate_contact_hash(contact);
            }
        }
        Ok(())
    }

    /// Remove label from contact
    pub async fn remove_label_from_contact(&self, jid: &JID, label: &str) -> Result<()> {
        let mut contacts = self.contacts.write().await;
        if let Some(contact) = contacts.get_mut(jid) {
            contact.labels.retain(|l| l != label);
            contact.last_updated = SystemTime::now();
            contact.version.timestamp = SystemTime::now();
            contact.version.hash = self.calculate_contact_hash(contact);
        }
        Ok(())
    }

    /// Delete a contact
    pub async fn delete_contact(&self, jid: &JID) -> Result<Option<Contact>> {
        let mut contacts = self.contacts.write().await;
        Ok(contacts.remove(jid))
    }

    /// Get contact statistics
    pub async fn get_contact_stats(&self) -> ContactStats {
        let contacts = self.contacts.read().await;
        let total = contacts.len();
        let whatsapp_users = contacts.values().filter(|c| c.is_whatsapp_user).count();
        let blocked = contacts.values().filter(|c| c.blocked).count();
        let verified = contacts.values().filter(|c| c.verified).count();
        let business = contacts.values().filter(|c| c.business_info.is_some()).count();

        ContactStats {
            total_contacts: total,
            whatsapp_users,
            blocked_contacts: blocked,
            verified_contacts: verified,
            business_contacts: business,
        }
    }

    /// Calculate hash for contact version
    fn calculate_contact_hash(&self, contact: &Contact) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        contact.name.hash(&mut hasher);
        contact.phone_number.hash(&mut hasher);
        contact.push_name.hash(&mut hasher);
        contact.blocked.hash(&mut hasher);
        contact.verified.hash(&mut hasher);
        contact.labels.hash(&mut hasher);
        
        format!("{:x}", hasher.finish())
    }

    /// Merge contacts for conflict resolution
    pub fn merge_contacts(&self, local: &Contact, remote: &Contact) -> Contact {
        let mut merged = local.clone();

        // Use the most recent version for each field
        if remote.version.timestamp > local.version.timestamp {
            merged.name = remote.name.clone();
            merged.push_name = remote.push_name.clone();
            merged.avatar = remote.avatar.clone();
            merged.avatar_url = remote.avatar_url.clone();
            merged.status = remote.status.clone();
            merged.version = remote.version.clone();
        }

        // Merge labels (union)
        for label in &remote.labels {
            if !merged.labels.contains(label) {
                merged.labels.push(label.clone());
            }
        }

        // Use most restrictive settings
        merged.blocked = local.blocked || remote.blocked;
        merged.verified = local.verified && remote.verified;

        // Update timestamp
        merged.last_updated = SystemTime::now();

        merged
    }
}

/// Contact statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactStats {
    pub total_contacts: usize,
    pub whatsapp_users: usize,
    pub blocked_contacts: usize,
    pub verified_contacts: usize,
    pub business_contacts: usize,
}

#[async_trait::async_trait]
impl AppStateSync for ContactSync {
    fn data_type(&self) -> AppStateDataType {
        AppStateDataType::Contacts
    }

    async fn sync_from_remote(&self, ctx: &SyncContext, events: Vec<AppStateEvent>) -> Result<()> {
        for event in events {
            match event.operation {
                AppStateOperation::Update => {
                    if let Some(data) = event.data {
                        let contact: Contact = serde_json::from_slice(&data)
                            .map_err(|e| Error::Protocol(format!("Failed to deserialize contact: {}", e)))?;
                        
                        let key = AppStateKey::contact(&contact.jid);
                        
                        // Check for conflicts
                        if let Some(existing) = self.get_contact(&contact.jid).await {
                            if existing.version.timestamp > contact.version.timestamp {
                                // Local version is newer, create conflict
                                let conflict = SyncConflict {
                                    key: key.clone(),
                                    local_version: existing.version,
                                    remote_version: contact.version,
                                    local_data: Some(serde_json::to_vec(&existing).unwrap()),
                                    remote_data: Some(data),
                                    detected_at: SystemTime::now(),
                                };
                                ctx.add_conflict(conflict).await;
                                ctx.update_sync_status(key, SyncStatus::Conflict).await;
                                continue;
                            }
                        }

                        self.update_contact(contact).await?;
                        ctx.update_sync_status(key, SyncStatus::Synced).await;
                    }
                }
                AppStateOperation::Delete => {
                    if let Ok(jid) = event.key.parse::<JID>() {
                        self.delete_contact(&jid).await?;
                        let key = AppStateKey::contact(&jid);
                        ctx.update_sync_status(key, SyncStatus::Synced).await;
                    }
                }
                _ => {
                    // Handle other operations as needed
                }
            }
        }

        ctx.update_last_sync(AppStateDataType::Contacts).await;
        Ok(())
    }

    async fn sync_to_remote(&self, ctx: &SyncContext) -> Result<Vec<AppStateEvent>> {
        let mut events = Vec::new();
        let contacts = self.get_all_contacts().await;

        for contact in contacts {
            let key = AppStateKey::contact(&contact.jid);
            let status = ctx.get_sync_status(&key).await;

            if status == SyncStatus::NotSynced {
                let data = serde_json::to_vec(&contact)
                    .map_err(|e| Error::Protocol(format!("Failed to serialize contact: {}", e)))?;

                events.push(AppStateEvent {
                    data_type: AppStateDataType::Contacts,
                    operation: AppStateOperation::Update,
                    timestamp: contact.last_updated,
                    key: contact.jid.to_string(),
                    data: Some(data),
                });

                ctx.update_sync_status(key, SyncStatus::Syncing).await;
            }
        }

        Ok(events)
    }

    async fn incremental_sync(&self, ctx: &SyncContext, since: SystemTime) -> Result<Vec<AppStateEvent>> {
        let mut events = Vec::new();
        let contacts = self.get_all_contacts().await;

        for contact in contacts {
            if contact.last_updated > since {
                let data = serde_json::to_vec(&contact)
                    .map_err(|e| Error::Protocol(format!("Failed to serialize contact: {}", e)))?;

                events.push(AppStateEvent {
                    data_type: AppStateDataType::Contacts,
                    operation: AppStateOperation::Update,
                    timestamp: contact.last_updated,
                    key: contact.jid.to_string(),
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
                let local_contact: Contact = serde_json::from_slice(local_data)
                    .map_err(|e| Error::Protocol(format!("Failed to deserialize local contact: {}", e)))?;
                let remote_contact: Contact = serde_json::from_slice(remote_data)
                    .map_err(|e| Error::Protocol(format!("Failed to deserialize remote contact: {}", e)))?;

                // Merge contacts
                let merged = self.merge_contacts(&local_contact, &remote_contact);
                self.update_contact(merged).await?;

                ctx.update_sync_status(conflict.key, SyncStatus::Synced).await;
            }
        }
        Ok(())
    }
}

impl Default for ContactFilter {
    fn default() -> Self {
        Self {
            name_contains: None,
            whatsapp_users_only: false,
            blocked: None,
            verified: None,
            has_labels: Vec::new(),
            business_only: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_contact_sync_basic_operations() {
        let sync = ContactSync::new();
        
        let jid = JID::new("test".to_string(), "s.whatsapp.net".to_string());
        let contact = Contact {
            jid: jid.clone(),
            name: "Test User".to_string(),
            push_name: Some("Test".to_string()),
            phone_number: "+1234567890".to_string(),
            avatar: None,
            avatar_url: None,
            is_whatsapp_user: true,
            verified: false,
            status: Some("Hello World".to_string()),
            last_seen: Some(SystemTime::now()),
            blocked: false,
            muted: false,
            labels: vec!["friend".to_string()],
            business_info: None,
            last_updated: SystemTime::now(),
            version: AppStateVersion {
                timestamp: SystemTime::now(),
                hash: "test_hash".to_string(),
                device_id: "test_device".to_string(),
            },
        };

        // Add contact
        sync.update_contact(contact.clone()).await.unwrap();

        // Get contact
        let retrieved = sync.get_contact(&jid).await.unwrap();
        assert_eq!(retrieved.name, "Test User");
        assert_eq!(retrieved.phone_number, "+1234567890");

        // Get by phone
        let by_phone = sync.get_contact_by_phone("+1234567890").await.unwrap();
        assert_eq!(by_phone.name, "Test User");

        // Test cached name
        let cached_name = sync.get_cached_name("+1234567890").await.unwrap();
        assert_eq!(cached_name, "Test User");
    }

    #[tokio::test]
    async fn test_contact_filtering() {
        let sync = ContactSync::new();
        
        // Add test contacts
        let contacts = vec![
            Contact {
                jid: JID::new("user1".to_string(), "s.whatsapp.net".to_string()),
                name: "John Doe".to_string(),
                push_name: None,
                phone_number: "+1111111111".to_string(),
                avatar: None,
                avatar_url: None,
                is_whatsapp_user: true,
                verified: true,
                status: None,
                last_seen: None,
                blocked: false,
                muted: false,
                labels: vec!["work".to_string()],
                business_info: None,
                last_updated: SystemTime::now(),
                version: AppStateVersion {
                    timestamp: SystemTime::now(),
                    hash: "hash1".to_string(),
                    device_id: "device1".to_string(),
                },
            },
            Contact {
                jid: JID::new("user2".to_string(), "s.whatsapp.net".to_string()),
                name: "Jane Smith".to_string(),
                push_name: None,
                phone_number: "+2222222222".to_string(),
                avatar: None,
                avatar_url: None,
                is_whatsapp_user: false,
                verified: false,
                status: None,
                last_seen: None,
                blocked: true,
                muted: false,
                labels: vec!["family".to_string()],
                business_info: None,
                last_updated: SystemTime::now(),
                version: AppStateVersion {
                    timestamp: SystemTime::now(),
                    hash: "hash2".to_string(),
                    device_id: "device2".to_string(),
                },
            },
        ];

        for contact in contacts {
            sync.update_contact(contact).await.unwrap();
        }

        // Test name filter
        let filter = ContactFilter {
            name_contains: Some("John".to_string()),
            ..Default::default()
        };
        let results = sync.search_contacts(filter).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "John Doe");

        // Test WhatsApp users only
        let filter = ContactFilter {
            whatsapp_users_only: true,
            ..Default::default()
        };
        let results = sync.search_contacts(filter).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "John Doe");

        // Test blocked filter
        let filter = ContactFilter {
            blocked: Some(true),
            ..Default::default()
        };
        let results = sync.search_contacts(filter).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Jane Smith");
    }
}