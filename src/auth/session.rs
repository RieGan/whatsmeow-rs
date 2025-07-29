/// Session management and persistence for WhatsApp authentication
/// 
/// This module handles:
/// - Session storage and retrieval
/// - Authentication state persistence
/// - Session validation and expiration
/// - Multi-device session synchronization

use crate::{
    error::{Error, Result},
    types::JID,
    auth::{
        DeviceRegistration, PairingKeys, DeviceInfo,
        QRData, PairingState, PairingMethod,
    },
    database::Database,
};
use serde::{Deserialize, Serialize};
use std::{
    time::{SystemTime, Duration},
    collections::HashMap,
    sync::Arc,
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

/// Session state for authentication
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionState {
    /// No active session
    None,
    /// Session in progress (QR/phone pairing)
    InProgress {
        method: PairingMethod,
        started_at: SystemTime,
        expires_at: SystemTime,
    },
    /// Session established and authenticated
    Authenticated {
        registration: DeviceRegistration,
        established_at: SystemTime,
        last_active: SystemTime,
    },
    /// Session expired and needs re-authentication
    Expired {
        last_registration: Option<DeviceRegistration>,
        expired_at: SystemTime,
    },
    /// Session invalidated due to error
    Invalidated {
        reason: String,
        invalidated_at: SystemTime,
    },
}

impl SessionState {
    /// Check if session is currently authenticated
    pub fn is_authenticated(&self) -> bool {
        matches!(self, SessionState::Authenticated { .. })
    }
    
    /// Check if session is in progress
    pub fn is_in_progress(&self) -> bool {
        matches!(self, SessionState::InProgress { .. })
    }
    
    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        match self {
            SessionState::Expired { .. } => true,
            SessionState::InProgress { expires_at, .. } => {
                SystemTime::now() > *expires_at
            }
            SessionState::Authenticated { .. } => false,
            _ => false,
        }
    }
    
    /// Get device registration if authenticated
    pub fn registration(&self) -> Option<&DeviceRegistration> {
        match self {
            SessionState::Authenticated { registration, .. } => Some(registration),
            _ => None,
        }
    }
    
    /// Get last active time if available
    pub fn last_active(&self) -> Option<SystemTime> {
        match self {
            SessionState::Authenticated { last_active, .. } => Some(*last_active),
            _ => None,
        }
    }
    
    /// Update last active time
    pub fn update_activity(&mut self) {
        if let SessionState::Authenticated { last_active, .. } = self {
            *last_active = SystemTime::now();
        }
    }
}

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Maximum time for pairing session to complete (default: 5 minutes)
    pub pairing_timeout: Duration,
    /// Session inactivity timeout (default: 24 hours)
    pub inactivity_timeout: Duration,
    /// Maximum session duration before re-authentication required (default: 30 days)
    pub max_session_duration: Duration,
    /// Enable session persistence to database
    pub enable_persistence: bool,
    /// Session validation interval (default: 1 hour)
    pub validation_interval: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            pairing_timeout: Duration::from_secs(300), // 5 minutes
            inactivity_timeout: Duration::from_secs(86400), // 24 hours
            max_session_duration: Duration::from_secs(2592000), // 30 days
            enable_persistence: true,
            validation_interval: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Session data for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub jid: JID,
    pub state: SessionState,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub metadata: HashMap<String, String>,
}

impl SessionData {
    /// Create new session data
    pub fn new(jid: JID, state: SessionState) -> Self {
        let now = SystemTime::now();
        Self {
            jid,
            state,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        }
    }
    
    /// Update session state
    pub fn update_state(&mut self, state: SessionState) {
        self.state = state;
        self.updated_at = SystemTime::now();
    }
    
    /// Add metadata
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
        self.updated_at = SystemTime::now();
    }
    
    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
    
    /// Check if session data is stale
    pub fn is_stale(&self, max_age: Duration) -> bool {
        SystemTime::now()
            .duration_since(self.updated_at)
            .map(|age| age > max_age)
            .unwrap_or(true)
    }
}

/// Session manager for handling authentication sessions
pub struct SessionManager {
    config: SessionConfig,
    sessions: Arc<RwLock<HashMap<JID, SessionData>>>,
    database: Option<Arc<Database>>,
    validation_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Clone for SessionManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            sessions: self.sessions.clone(),
            database: self.database.clone(),
            validation_handle: None, // JoinHandle is not cloneable
        }
    }
}

impl SessionManager {
    /// Create new session manager
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            database: None,
            validation_handle: None,
        }
    }
    
    /// Create session manager with database persistence
    pub fn with_database(config: SessionConfig, database: Arc<Database>) -> Self {
        Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            database: Some(database),
            validation_handle: None,
        }
    }
    
    /// Start session validation background task
    pub async fn start_validation(&mut self) -> Result<()> {
        if self.validation_handle.is_some() {
            return Ok(());
        }
        
        let sessions = self.sessions.clone();
        let config = self.config.clone();
        let database = self.database.clone();
        
        let handle = tokio::spawn(async move {
            Self::validation_task(sessions, config, database).await;
        });
        
        self.validation_handle = Some(handle);
        info!("Session validation task started");
        Ok(())
    }
    
    /// Stop session validation task
    pub async fn stop_validation(&mut self) {
        if let Some(handle) = self.validation_handle.take() {
            handle.abort();
            debug!("Session validation task stopped");
        }
    }
    
    /// Create new session
    pub async fn create_session(&self, jid: JID, method: PairingMethod) -> Result<()> {
        let expires_at = SystemTime::now() + self.config.pairing_timeout;
        let state = SessionState::InProgress {
            method,
            started_at: SystemTime::now(),
            expires_at,
        };
        
        let session_data = SessionData::new(jid.clone(), state);
        
        // Store in memory
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(jid.clone(), session_data.clone());
        }
        
        // Persist to database if enabled
        if self.config.enable_persistence {
            if let Some(database) = &self.database {
                self.persist_session(database, &session_data).await?;
            }
        }
        
        info!("Created new session for JID: {}", jid);
        Ok(())
    }
    
    /// Update session state
    pub async fn update_session(&self, jid: &JID, state: SessionState) -> Result<()> {
        let mut updated_data = None;
        
        // Update in memory
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session_data) = sessions.get_mut(jid) {
                session_data.update_state(state);
                updated_data = Some(session_data.clone());
            } else {
                return Err(Error::Auth(format!("Session not found for JID: {}", jid)));
            }
        }
        
        // Persist to database if enabled
        if let Some(data) = updated_data {
            if self.config.enable_persistence {
                if let Some(database) = &self.database {
                    self.persist_session(database, &data).await?;
                }
            }
        }
        
        debug!("Updated session state for JID: {}", jid);
        Ok(())
    }
    
    /// Authenticate session with device registration
    pub async fn authenticate_session(&self, jid: &JID, registration: DeviceRegistration) -> Result<()> {
        let state = SessionState::Authenticated {
            registration,
            established_at: SystemTime::now(),
            last_active: SystemTime::now(),
        };
        
        self.update_session(jid, state).await?;
        info!("Session authenticated for JID: {}", jid);
        Ok(())
    }
    
    /// Mark session as expired
    pub async fn expire_session(&self, jid: &JID) -> Result<()> {
        let last_registration = self.get_session(jid).await?
            .and_then(|data| data.state.registration().cloned());
            
        let state = SessionState::Expired {
            last_registration,
            expired_at: SystemTime::now(),
        };
        
        self.update_session(jid, state).await?;
        info!("Session expired for JID: {}", jid);
        Ok(())
    }
    
    /// Invalidate session with reason
    pub async fn invalidate_session(&self, jid: &JID, reason: String) -> Result<()> {
        let state = SessionState::Invalidated {
            reason: reason.clone(),
            invalidated_at: SystemTime::now(),
        };
        
        self.update_session(jid, state).await?;
        warn!("Session invalidated for JID: {} - {}", jid, reason);
        Ok(())
    }
    
    /// Get session data
    pub async fn get_session(&self, jid: &JID) -> Result<Option<SessionData>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(jid).cloned())
    }
    
    /// Get session state
    pub async fn get_session_state(&self, jid: &JID) -> Result<Option<SessionState>> {
        Ok(self.get_session(jid).await?.map(|data| data.state))
    }
    
    /// Check if session is authenticated
    pub async fn is_authenticated(&self, jid: &JID) -> bool {
        self.get_session_state(jid).await
            .map(|state| state.map(|s| s.is_authenticated()).unwrap_or(false))
            .unwrap_or(false)
    }
    
    /// Update session activity
    pub async fn update_activity(&self, jid: &JID) -> Result<()> {
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session_data) = sessions.get_mut(jid) {
                session_data.state.update_activity();
                session_data.updated_at = SystemTime::now();
            } else {
                return Err(Error::Auth(format!("Session not found for JID: {}", jid)));
            }
        }
        
        debug!("Updated activity for session: {}", jid);
        Ok(())
    }
    
    /// Remove session completely
    pub async fn remove_session(&self, jid: &JID) -> Result<()> {
        // Remove from memory
        {
            let mut sessions = self.sessions.write().await;
            sessions.remove(jid);
        }
        
        // Remove from database if enabled
        if self.config.enable_persistence {
            if let Some(database) = &self.database {
                self.remove_persisted_session(database, jid).await?;
            }
        }
        
        info!("Removed session for JID: {}", jid);
        Ok(())
    }
    
    /// List all active sessions
    pub async fn list_sessions(&self) -> Result<Vec<SessionData>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.values().cloned().collect())
    }
    
    /// Count sessions by state
    pub async fn count_sessions_by_state(&self) -> HashMap<String, usize> {
        let sessions = self.sessions.read().await;
        let mut counts = HashMap::new();
        
        for session in sessions.values() {
            let state_name = match &session.state {
                SessionState::None => "none",
                SessionState::InProgress { .. } => "in_progress",
                SessionState::Authenticated { .. } => "authenticated",
                SessionState::Expired { .. } => "expired",
                SessionState::Invalidated { .. } => "invalidated",
            };
            
            *counts.entry(state_name.to_string()).or_insert(0) += 1;
        }
        
        counts
    }
    
    /// Load sessions from database on startup
    pub async fn load_sessions(&self) -> Result<usize> {
        if !self.config.enable_persistence {
            return Ok(0);
        }
        
        let database = self.database.as_ref()
            .ok_or_else(|| Error::Database("No database configured for session persistence".to_string()))?;
            
        let loaded_sessions = self.load_persisted_sessions(database).await?;
        let count = loaded_sessions.len();
        
        {
            let mut sessions = self.sessions.write().await;
            for session_data in loaded_sessions {
                sessions.insert(session_data.jid.clone(), session_data);
            }
        }
        
        info!("Loaded {} sessions from database", count);
        Ok(count)
    }
    
    /// Save all sessions to database
    pub async fn save_sessions(&self) -> Result<usize> {
        if !self.config.enable_persistence {
            return Ok(0);
        }
        
        let database = self.database.as_ref()
            .ok_or_else(|| Error::Database("No database configured for session persistence".to_string()))?;
            
        let sessions = {
            let sessions_guard = self.sessions.read().await;
            sessions_guard.values().cloned().collect::<Vec<_>>()
        };
        
        let count = sessions.len();
        for session_data in sessions {
            self.persist_session(database, &session_data).await?;
        }
        
        info!("Saved {} sessions to database", count);
        Ok(count)
    }
    
    /// Cleanup expired and stale sessions
    pub async fn cleanup_sessions(&self) -> Result<usize> {
        let mut expired_jids = Vec::new();
        
        {
            let sessions = self.sessions.read().await;
            for (jid, session_data) in sessions.iter() {
                if session_data.state.is_expired() || 
                   session_data.is_stale(self.config.max_session_duration) {
                    expired_jids.push(jid.clone());
                }
            }
        }
        
        let count = expired_jids.len();
        for jid in expired_jids {
            if let Err(e) = self.expire_session(&jid).await {
                warn!("Failed to expire session {}: {}", jid, e);
            }
        }
        
        debug!("Cleaned up {} expired sessions", count);
        Ok(count)
    }
    
    // Private helper methods
    
    /// Background task for session validation
    async fn validation_task(
        sessions: Arc<RwLock<HashMap<JID, SessionData>>>,
        config: SessionConfig,
        database: Option<Arc<Database>>,
    ) {
        let mut interval = tokio::time::interval(config.validation_interval);
        
        loop {
            interval.tick().await;
            
            // Check for expired sessions
            let mut expired_jids = Vec::new();
            {
                let sessions_guard = sessions.read().await;
                for (jid, session_data) in sessions_guard.iter() {
                    if session_data.state.is_expired() ||
                       session_data.is_stale(config.max_session_duration) {
                        expired_jids.push(jid.clone());
                    }
                }
            }
            
            // Mark expired sessions
            if !expired_jids.is_empty() {
                let mut sessions_guard = sessions.write().await;
                let expired_count = expired_jids.len();
                for jid in expired_jids {
                    if let Some(session_data) = sessions_guard.get_mut(&jid) {
                        let last_registration = session_data.state.registration().cloned();
                        session_data.update_state(SessionState::Expired {
                            last_registration,
                            expired_at: SystemTime::now(),
                        });
                        
                        // Persist expiration if database available
                        if let Some(db) = &database {
                            // Note: This is a simplified persistence call
                            // In practice, you'd want proper error handling
                            let _ = Self::persist_session_static(db, session_data).await;
                        }
                    }
                }
                
                debug!("Validation task expired {} sessions", expired_count);
            }
        }
    }
    
    /// Persist session to database
    async fn persist_session(&self, database: &Database, session_data: &SessionData) -> Result<()> {
        // Serialize session data
        let data_json = serde_json::to_string(session_data)
            .map_err(|e| Error::Serialization(format!("Failed to serialize session: {}", e)))?;
            
        // Store in database (simplified - would use actual database operations)
        database.store_session(&session_data.jid, &data_json).await
    }
    
    /// Static version for background task
    async fn persist_session_static(database: &Database, session_data: &SessionData) -> Result<()> {
        let data_json = serde_json::to_string(session_data)
            .map_err(|e| Error::Serialization(format!("Failed to serialize session: {}", e)))?;
        database.store_session(&session_data.jid, &data_json).await
    }
    
    /// Load sessions from database
    async fn load_persisted_sessions(&self, database: &Database) -> Result<Vec<SessionData>> {
        let session_strings = database.load_all_sessions().await?;
        let mut sessions = Vec::new();
        
        for session_string in session_strings {
            match serde_json::from_str::<SessionData>(&session_string) {
                Ok(session_data) => sessions.push(session_data),
                Err(e) => warn!("Failed to deserialize session data: {}", e),
            }
        }
        
        Ok(sessions)
    }
    
    /// Remove session from database
    async fn remove_persisted_session(&self, database: &Database, jid: &JID) -> Result<()> {
        database.remove_session(jid).await
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        if let Some(handle) = self.validation_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, timeout};
    
    #[test]
    fn test_session_state_creation() {
        let state = SessionState::None;
        assert!(!state.is_authenticated());
        assert!(!state.is_in_progress());
        assert!(!state.is_expired());
    }
    
    #[test]
    fn test_session_state_in_progress() {
        let expires_at = SystemTime::now() + Duration::from_secs(300);
        let state = SessionState::InProgress {
            method: PairingMethod::QRCode,
            started_at: SystemTime::now(),
            expires_at,
        };
        
        assert!(!state.is_authenticated());
        assert!(state.is_in_progress());
        assert!(!state.is_expired());
    }
    
    #[test]
    fn test_session_config_default() {
        let config = SessionConfig::default();
        assert_eq!(config.pairing_timeout, Duration::from_secs(300));
        assert_eq!(config.inactivity_timeout, Duration::from_secs(86400));
        assert!(config.enable_persistence);
    }
    
    #[test]
    fn test_session_data_creation() {
        let jid = JID::new("1234567890".to_string(), "s.whatsapp.net".to_string());
        let state = SessionState::None;
        let session_data = SessionData::new(jid.clone(), state);
        
        assert_eq!(session_data.jid, jid);
        assert!(matches!(session_data.state, SessionState::None));
        assert!(session_data.metadata.is_empty());
    }
    
    #[test]
    fn test_session_data_metadata() {
        let jid = JID::new("1234567890".to_string(), "s.whatsapp.net".to_string());
        let mut session_data = SessionData::new(jid, SessionState::None);
        
        session_data.add_metadata("key1".to_string(), "value1".to_string());
        assert_eq!(session_data.get_metadata("key1"), Some(&"value1".to_string()));
        assert_eq!(session_data.get_metadata("key2"), None);
    }
    
    #[tokio::test]
    async fn test_session_manager_creation() {
        let config = SessionConfig::default();
        let manager = SessionManager::new(config);
        
        let sessions = manager.list_sessions().await.unwrap();
        assert!(sessions.is_empty());
    }
    
    #[tokio::test]
    async fn test_session_lifecycle() {
        let config = SessionConfig::default();
        let manager = SessionManager::new(config);
        let jid = JID::new("1234567890".to_string(), "s.whatsapp.net".to_string());
        
        // Create session
        manager.create_session(jid.clone(), PairingMethod::QRCode).await.unwrap();
        assert!(manager.get_session(&jid).await.unwrap().is_some());
        
        // Check state
        let state = manager.get_session_state(&jid).await.unwrap().unwrap();
        assert!(state.is_in_progress());
        assert!(!state.is_authenticated());
        
        // Remove session
        manager.remove_session(&jid).await.unwrap();
        assert!(manager.get_session(&jid).await.unwrap().is_none());
    }
    
    #[tokio::test]
    async fn test_session_authentication() {
        let config = SessionConfig::default();
        let manager = SessionManager::new(config);
        let jid = JID::new("1234567890".to_string(), "s.whatsapp.net".to_string());
        
        // Create session
        manager.create_session(jid.clone(), PairingMethod::QRCode).await.unwrap();
        
        // Create mock registration
        let keys = crate::auth::PairingKeys::generate();
        let device_info = crate::auth::DeviceInfo::default();
        let registration = crate::auth::DeviceRegistration::new(
            jid.clone(),
            1,
            keys,
            device_info,
            "test-token".to_string(),
            None,
            "web".to_string(),
            vec![1, 2, 3, 4],
        ).unwrap();
        
        // Authenticate
        manager.authenticate_session(&jid, registration).await.unwrap();
        
        // Check authentication
        assert!(manager.is_authenticated(&jid).await);
        let state = manager.get_session_state(&jid).await.unwrap().unwrap();
        assert!(state.is_authenticated());
    }
    
    #[tokio::test]
    async fn test_session_expiration() {
        let config = SessionConfig {
            pairing_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let manager = SessionManager::new(config);
        let jid = JID::new("1234567890".to_string(), "s.whatsapp.net".to_string());
        
        // Create session with short timeout
        manager.create_session(jid.clone(), PairingMethod::QRCode).await.unwrap();
        
        // Wait for expiration
        sleep(Duration::from_millis(150)).await;
        
        // Session should be expired
        let state = manager.get_session_state(&jid).await.unwrap().unwrap();
        assert!(state.is_expired());
    }
    
    #[tokio::test]
    async fn test_session_activity_update() {
        let config = SessionConfig::default();
        let manager = SessionManager::new(config);
        let jid = JID::new("1234567890".to_string(), "s.whatsapp.net".to_string());
        
        // Create and authenticate session
        manager.create_session(jid.clone(), PairingMethod::QRCode).await.unwrap();
        let keys = crate::auth::PairingKeys::generate();
        let device_info = crate::auth::DeviceInfo::default();
        let registration = crate::auth::DeviceRegistration::new(
            jid.clone(),
            1,
            keys,
            device_info,
            "test-token".to_string(),
            None,
            "web".to_string(),
            vec![1, 2, 3, 4],
        ).unwrap();
        manager.authenticate_session(&jid, registration).await.unwrap();
        
        // Get initial last active time
        let initial_last_active = manager.get_session(&jid).await.unwrap()
            .unwrap().state.last_active().unwrap();
        
        // Wait a bit and update activity
        sleep(Duration::from_millis(50)).await;
        manager.update_activity(&jid).await.unwrap();
        
        // Last active should be updated
        let updated_last_active = manager.get_session(&jid).await.unwrap()
            .unwrap().state.last_active().unwrap();
        
        assert!(updated_last_active > initial_last_active);
    }
    
    #[tokio::test]
    async fn test_session_counts() {
        let config = SessionConfig::default();
        let manager = SessionManager::new(config);
        
        let jid1 = JID::new("1111111111".to_string(), "s.whatsapp.net".to_string());
        let jid2 = JID::new("2222222222".to_string(), "s.whatsapp.net".to_string());
        
        // Create different types of sessions
        manager.create_session(jid1.clone(), PairingMethod::QRCode).await.unwrap();
        manager.create_session(jid2.clone(), PairingMethod::QRCode).await.unwrap();
        manager.expire_session(&jid2).await.unwrap();
        
        let counts = manager.count_sessions_by_state().await;
        assert_eq!(counts.get("in_progress"), Some(&1));
        assert_eq!(counts.get("expired"), Some(&1));
    }
}

// Extension trait for Database to support session operations
// This would be implemented in the database module
pub trait SessionStore {
    async fn store_session(&self, jid: &JID, data: &str) -> Result<()>;
    async fn load_session(&self, jid: &JID) -> Result<Option<String>>;
    async fn load_all_sessions(&self) -> Result<Vec<String>>;
    async fn remove_session(&self, jid: &JID) -> Result<()>;
}

// Implement for Database (this would go in database module)
impl SessionStore for Database {
    async fn store_session(&self, jid: &JID, data: &str) -> Result<()> {
        // Simplified implementation - would use actual SQL operations
        // This is a placeholder that would be implemented in database module
        debug!("Storing session for JID: {}", jid);
        Ok(())
    }
    
    async fn load_session(&self, jid: &JID) -> Result<Option<String>> {
        // Simplified implementation
        debug!("Loading session for JID: {}", jid);
        Ok(None)
    }
    
    async fn load_all_sessions(&self) -> Result<Vec<String>> {
        // Simplified implementation
        debug!("Loading all sessions");
        Ok(Vec::new())
    }
    
    async fn remove_session(&self, jid: &JID) -> Result<()> {
        // Simplified implementation
        debug!("Removing session for JID: {}", jid);
        Ok(())
    }
}