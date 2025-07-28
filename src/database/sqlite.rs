/// SQLite implementations for persistent storage

use crate::{
    error::{Error, Result},
    types::JID,
    store::{DeviceStore, DeviceData},
    group::types::{GroupInfo, GroupSettings},
};
use async_trait::async_trait;
use sqlx::{SqlitePool, Row};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// SQLite implementation of DeviceStore
pub struct SqliteDeviceStore {
    pool: SqlitePool,
}

impl SqliteDeviceStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DeviceStore for SqliteDeviceStore {
    async fn save_device(&self, data: &DeviceData) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO devices 
            (jid, registration_id, noise_key, identity_key, signed_pre_key, signed_pre_key_id, signed_pre_key_signature)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&data.jid.to_string())
        .bind(data.registration_id as i64)
        .bind(&data.noise_key)
        .bind(&data.identity_key)
        .bind(&data.signed_pre_key)
        .bind(data.signed_pre_key_id as i64)
        .bind(&data.signed_pre_key_signature)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to save device: {}", e)))?;
        
        Ok(())
    }
    
    async fn load_device(&self) -> Result<Option<DeviceData>> {
        let row = sqlx::query(
            "SELECT jid, registration_id, noise_key, identity_key, signed_pre_key, signed_pre_key_id, signed_pre_key_signature FROM devices LIMIT 1"
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to load device: {}", e)))?;
        
        if let Some(row) = row {
            let jid_str: String = row.get(0);
            let jid = JID::parse(&jid_str)?;
            
            Ok(Some(DeviceData {
                jid,
                registration_id: row.get::<i64, _>(1) as u32,
                noise_key: row.get(2),
                identity_key: row.get(3),
                signed_pre_key: row.get(4),
                signed_pre_key_id: row.get::<i64, _>(5) as u32,
                signed_pre_key_signature: row.get(6),
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn delete_device(&self) -> Result<()> {
        sqlx::query("DELETE FROM devices")
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to delete device: {}", e)))?;
        
        Ok(())
    }
    
    async fn is_registered(&self) -> Result<bool> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM devices")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to check registration: {}", e)))?;
        
        Ok(count > 0)
    }
}

/// SQLite-based group store for persistence
pub struct SqliteGroupStore {
    pool: SqlitePool,
}

impl SqliteGroupStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    
    /// Store group information
    pub async fn store_group(&self, group: &GroupInfo) -> Result<()> {
        let settings_json = serde_json::to_string(&group.settings)
            .map_err(|e| Error::Database(format!("Failed to serialize group settings: {}", e)))?;
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO groups 
            (jid, name, description, creator, created_at, avatar_url, invite_link, settings_json)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&group.jid.to_string())
        .bind(&group.name)
        .bind(&group.description)
        .bind(&group.creator.to_string())
        .bind(DateTime::<Utc>::from(group.created_at))
        .bind(&group.invite_link)
        .bind(&group.invite_link)
        .bind(&settings_json)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to store group: {}", e)))?;
        
        // Store participants
        for participant in &group.participants {
            let role = if group.creator == *participant {
                0 // Creator
            } else if group.admins.contains(participant) {
                1 // Admin
            } else {
                2 // Member
            };
            
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO group_participants
                (group_jid, participant_jid, role, joined_at, status)
                VALUES (?, ?, ?, ?, ?)
                "#
            )
            .bind(&group.jid.to_string())
            .bind(&participant.to_string())
            .bind(role)
            .bind(DateTime::<Utc>::from(group.created_at))
            .bind(0) // Active status
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to store group participant: {}", e)))?;
        }
        
        Ok(())
    }
    
    /// Load group information
    pub async fn load_group(&self, group_jid: &JID) -> Result<Option<GroupInfo>> {
        let row = sqlx::query(
            r#"
            SELECT name, description, creator, created_at, avatar_url, invite_link, settings_json
            FROM groups WHERE jid = ?
            "#
        )
        .bind(&group_jid.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to load group: {}", e)))?;
        
        if let Some(row) = row {
            let name: String = row.get(0);
            let description: Option<String> = row.get(1);
            let creator_str: String = row.get(2);
            let created_at: DateTime<Utc> = row.get(3);
            let avatar_url: Option<String> = row.get(4);
            let invite_link: Option<String> = row.get(5);
            let settings_json: String = row.get(6);
            
            let creator = JID::parse(&creator_str)?;
            let settings: GroupSettings = serde_json::from_str(&settings_json)
                .map_err(|e| Error::Database(format!("Failed to deserialize group settings: {}", e)))?;
            
            // Load participants
            let participant_rows = sqlx::query(
                "SELECT participant_jid, role FROM group_participants WHERE group_jid = ? AND status = 0"
            )
            .bind(&group_jid.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to load group participants: {}", e)))?;
            
            let mut participants = Vec::new();
            let mut admins = Vec::new();
            
            for participant_row in participant_rows {
                let participant_str: String = participant_row.get(0);
                let role: i32 = participant_row.get(1);
                let participant_jid = JID::parse(&participant_str)?;
                
                participants.push(participant_jid.clone());
                
                if role <= 1 { // Creator or Admin
                    admins.push(participant_jid);
                }
            }
            
            Ok(Some(GroupInfo {
                jid: group_jid.clone(),
                name,
                description,
                participants,
                admins,
                creator,
                created_at: created_at.into(),
                settings,
                invite_link,
            }))
        } else {
            Ok(None)
        }
    }
    
    /// Delete group
    pub async fn delete_group(&self, group_jid: &JID) -> Result<()> {
        sqlx::query("DELETE FROM groups WHERE jid = ?")
            .bind(&group_jid.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to delete group: {}", e)))?;
        
        Ok(())
    }
    
    /// List all groups
    pub async fn list_groups(&self) -> Result<Vec<JID>> {
        let group_jids: Vec<String> = sqlx::query_scalar("SELECT jid FROM groups")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to list groups: {}", e)))?;
        
        let mut jids = Vec::new();
        for jid_str in group_jids {
            jids.push(JID::parse(&jid_str)?);
        }
        
        Ok(jids)
    }
    
    /// Update group participant role
    pub async fn update_participant_role(&self, group_jid: &JID, participant_jid: &JID, role: i32) -> Result<()> {
        sqlx::query(
            "UPDATE group_participants SET role = ? WHERE group_jid = ? AND participant_jid = ?"
        )
        .bind(role)
        .bind(&group_jid.to_string())
        .bind(&participant_jid.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to update participant role: {}", e)))?;
        
        Ok(())
    }
    
    /// Remove participant from group
    pub async fn remove_participant(&self, group_jid: &JID, participant_jid: &JID) -> Result<()> {
        sqlx::query(
            "UPDATE group_participants SET status = 2 WHERE group_jid = ? AND participant_jid = ?"
        )
        .bind(&group_jid.to_string())
        .bind(&participant_jid.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to remove participant: {}", e)))?;
        
        Ok(())
    }
}

/// SQLite-based contact store
pub struct SqliteContactStore {
    pool: SqlitePool,
}

impl SqliteContactStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    
    /// Store contact information
    pub async fn store_contact(&self, jid: &JID, name: Option<&str>, phone: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO contacts (jid, name, phone_number)
            VALUES (?, ?, ?)
            "#
        )
        .bind(&jid.to_string())
        .bind(name)
        .bind(phone)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to store contact: {}", e)))?;
        
        Ok(())
    }
    
    /// Load contact information
    pub async fn load_contact(&self, jid: &JID) -> Result<Option<ContactInfo>> {
        let row = sqlx::query(
            "SELECT name, notify_name, phone_number, status_text, last_seen FROM contacts WHERE jid = ?"
        )
        .bind(&jid.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to load contact: {}", e)))?;
        
        if let Some(row) = row {
            Ok(Some(ContactInfo {
                jid: jid.clone(),
                name: row.get(0),
                notify_name: row.get(1),
                phone_number: row.get(2),
                status_text: row.get(3),
                last_seen: row.get(4),
            }))
        } else {
            Ok(None)
        }
    }
    
    /// List all contacts
    pub async fn list_contacts(&self) -> Result<Vec<ContactInfo>> {
        let rows = sqlx::query(
            "SELECT jid, name, notify_name, phone_number, status_text, last_seen FROM contacts"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to list contacts: {}", e)))?;
        
        let mut contacts = Vec::new();
        for row in rows {
            let jid_str: String = row.get(0);
            let jid = JID::parse(&jid_str)?;
            
            contacts.push(ContactInfo {
                jid,
                name: row.get(1),
                notify_name: row.get(2),
                phone_number: row.get(3),
                status_text: row.get(4),
                last_seen: row.get(5),
            });
        }
        
        Ok(contacts)
    }
}

/// Contact information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInfo {
    pub jid: JID,
    pub name: Option<String>,
    pub notify_name: Option<String>,
    pub phone_number: Option<String>,
    pub status_text: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
}

/// Settings store for key-value configuration
pub struct SqliteSettingsStore {
    pool: SqlitePool,
}

impl SqliteSettingsStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    
    /// Store a setting
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)"
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to set setting: {}", e)))?;
        
        Ok(())
    }
    
    /// Get a setting
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let value: Option<String> = sqlx::query_scalar(
            "SELECT value FROM settings WHERE key = ?"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to get setting: {}", e)))?;
        
        Ok(value)
    }
    
    /// Delete a setting
    pub async fn delete_setting(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM settings WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to delete setting: {}", e)))?;
        
        Ok(())
    }
    
    /// Get all settings
    pub async fn get_all_settings(&self) -> Result<HashMap<String, String>> {
        let rows = sqlx::query("SELECT key, value FROM settings")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to get all settings: {}", e)))?;
        
        let mut settings = HashMap::new();
        for row in rows {
            let key: String = row.get(0);
            let value: String = row.get(1);
            settings.insert(key, value);
        }
        
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use tempfile::tempdir;
    use std::time::SystemTime;
    
    async fn create_test_db() -> Database {
        let config = crate::database::DatabaseConfig {
            database_url: "sqlite::memory:".to_string(),
            max_connections: 5,
            connection_timeout: 10,
            enable_wal: false,
        };
        
        Database::new(config).await.unwrap()
    }
    
    #[tokio::test]
    async fn test_device_store() {
        let db = create_test_db().await;
        let store = SqliteDeviceStore::new(db.pool().clone());
        
        let device_data = DeviceData {
            jid: JID::new("test".to_string(), "s.whatsapp.net".to_string()),
            registration_id: 12345,
            noise_key: vec![1, 2, 3],
            identity_key: vec![4, 5, 6],
            signed_pre_key: vec![7, 8, 9],
            signed_pre_key_id: 1,
            signed_pre_key_signature: vec![10, 11, 12],
        };
        
        // Test save and load
        store.save_device(&device_data).await.unwrap();
        assert!(store.is_registered().await.unwrap());
        
        let loaded = store.load_device().await.unwrap().unwrap();
        assert_eq!(loaded.jid, device_data.jid);
        assert_eq!(loaded.registration_id, device_data.registration_id);
        
        // Test delete
        store.delete_device().await.unwrap();
        assert!(!store.is_registered().await.unwrap());
        
        db.close().await;
    }
    
    #[tokio::test]
    async fn test_settings_store() {
        let db = create_test_db().await;
        let store = SqliteSettingsStore::new(db.pool().clone());
        
        // Test set and get
        store.set_setting("test_key", "test_value").await.unwrap();
        let value = store.get_setting("test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));
        
        // Test get non-existent
        let none_value = store.get_setting("non_existent").await.unwrap();
        assert_eq!(none_value, None);
        
        // Test delete
        store.delete_setting("test_key").await.unwrap();
        let deleted_value = store.get_setting("test_key").await.unwrap();
        assert_eq!(deleted_value, None);
        
        db.close().await;
    }
    
    #[tokio::test]
    async fn test_contact_store() {
        let db = create_test_db().await;
        let store = SqliteContactStore::new(db.pool().clone());
        
        let jid = JID::new("contact".to_string(), "s.whatsapp.net".to_string());
        
        // Test store and load
        store.store_contact(&jid, Some("Test Contact"), Some("+1234567890")).await.unwrap();
        
        let contact = store.load_contact(&jid).await.unwrap().unwrap();
        assert_eq!(contact.jid, jid);
        assert_eq!(contact.name, Some("Test Contact".to_string()));
        assert_eq!(contact.phone_number, Some("+1234567890".to_string()));
        
        // Test list contacts
        let contacts = store.list_contacts().await.unwrap();
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].jid, jid);
        
        db.close().await;
    }
}