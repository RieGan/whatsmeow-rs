/// App State synchronization protocol handler for WhatsApp
/// 
/// Handles the WhatsApp-specific app state synchronization protocol including:
/// - App state patches and mutations
/// - Snapshot management
/// - Protocol message handling
/// - Sync session management
/// - Version tracking and conflict resolution

use crate::{
    appstate::{
        AppStateSync, AppStateEvent, AppStateOperation, AppStateDataType, 
        AppStateKey, SyncContext, SyncStatus, SyncConflict, AppStateVersion,
        ContactSync, ChatMetadataSync, SettingsSync
    },
    binary::{BinaryNode, BinaryDecoder, BinaryEncoder},
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

/// WhatsApp App State protocol handler
pub struct AppStateSyncProtocol {
    /// Contact synchronization handler
    contact_sync: Arc<ContactSync>,
    /// Chat metadata synchronization handler
    chat_metadata_sync: Arc<ChatMetadataSync>,
    /// Settings synchronization handler
    settings_sync: Arc<SettingsSync>,
    /// Active sync sessions
    sync_sessions: Arc<RwLock<HashMap<String, SyncSession>>>,
    /// Protocol configuration
    config: AppStateSyncConfig,
    /// Current app state snapshots
    snapshots: Arc<RwLock<HashMap<AppStateDataType, AppStateSnapshot>>>,
}

/// App State synchronization configuration
#[derive(Debug, Clone)]
pub struct AppStateSyncConfig {
    /// Maximum number of mutations per patch
    pub max_mutations_per_patch: usize,
    /// Sync timeout in seconds
    pub sync_timeout_seconds: u64,
    /// Retry attempts for failed syncs
    pub max_retry_attempts: u32,
    /// Batch size for processing mutations
    pub mutation_batch_size: usize,
    /// Enable incremental sync
    pub enable_incremental_sync: bool,
    /// Sync interval in seconds
    pub sync_interval_seconds: u64,
}

/// App State sync session
#[derive(Debug, Clone)]
pub struct SyncSession {
    /// Session ID
    pub session_id: String,
    /// Data type being synced
    pub data_type: AppStateDataType,
    /// Session state
    pub state: SyncSessionState,
    /// Current version
    pub current_version: Option<AppStateVersion>,
    /// Target version
    pub target_version: Option<AppStateVersion>,
    /// Pending mutations
    pub pending_mutations: Vec<AppStateMutation>,
    /// Session start time
    pub started_at: SystemTime,
    /// Last activity time
    pub last_activity: SystemTime,
    /// Retry count
    pub retry_count: u32,
}

/// App State sync session states
#[derive(Debug, Clone, PartialEq)]
pub enum SyncSessionState {
    /// Session initializing
    Initializing,
    /// Requesting snapshot
    RequestingSnapshot,
    /// Processing snapshot
    ProcessingSnapshot,
    /// Applying mutations
    ApplyingMutations,
    /// Sending mutations
    SendingMutations,
    /// Session completed successfully
    Completed,
    /// Session failed
    Failed { error: String },
    /// Session cancelled
    Cancelled,
}

/// App State snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSnapshot {
    /// Data type
    pub data_type: AppStateDataType,
    /// Snapshot version
    pub version: AppStateVersion,
    /// Snapshot data
    pub data: Vec<u8>,
    /// Number of records in snapshot
    pub record_count: u32,
    /// Snapshot creation time
    pub created_at: SystemTime,
    /// Snapshot hash for integrity
    pub hash: String,
}

/// App State mutation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateMutation {
    /// Mutation ID
    pub mutation_id: String,
    /// Data type
    pub data_type: AppStateDataType,
    /// Operation type
    pub operation: AppStateOperation,
    /// Target key
    pub key: String,
    /// Mutation data
    pub data: Option<Vec<u8>>,
    /// Mutation version
    pub version: AppStateVersion,
    /// Dependencies on other mutations
    pub dependencies: Vec<String>,
}

/// App State patch containing multiple mutations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStatePatch {
    /// Patch ID
    pub patch_id: String,
    /// Data type
    pub data_type: AppStateDataType,
    /// Mutations in this patch
    pub mutations: Vec<AppStateMutation>,
    /// Patch version
    pub version: AppStateVersion,
    /// Patch creation time
    pub created_at: SystemTime,
}

/// Protocol message types for app state sync
#[derive(Debug, Clone)]
pub enum AppStateProtocolMessage {
    /// Request snapshot
    SnapshotRequest {
        data_type: AppStateDataType,
        version: Option<AppStateVersion>,
    },
    /// Snapshot response
    SnapshotResponse {
        snapshot: AppStateSnapshot,
    },
    /// Mutation patch
    MutationPatch {
        patch: AppStatePatch,
    },
    /// Sync status update
    SyncStatus {
        data_type: AppStateDataType,
        status: SyncStatus,
        version: Option<AppStateVersion>,
    },
    /// Sync completion notification
    SyncComplete {
        data_type: AppStateDataType,
        final_version: AppStateVersion,
    },
    /// Error notification
    Error {
        error_code: String,
        error_message: String,
        data_type: Option<AppStateDataType>,
    },
}

impl Default for AppStateSyncConfig {
    fn default() -> Self {
        Self {
            max_mutations_per_patch: 100,
            sync_timeout_seconds: 300, // 5 minutes
            max_retry_attempts: 3,
            mutation_batch_size: 50,
            enable_incremental_sync: true,
            sync_interval_seconds: 3600, // 1 hour
        }
    }
}

impl AppStateSyncProtocol {
    /// Create new app state sync protocol handler
    pub fn new(
        contact_sync: Arc<ContactSync>,
        chat_metadata_sync: Arc<ChatMetadataSync>,
        settings_sync: Arc<SettingsSync>,
    ) -> Self {
        Self::with_config(
            contact_sync,
            chat_metadata_sync,
            settings_sync,
            AppStateSyncConfig::default(),
        )
    }

    /// Create new app state sync protocol handler with custom config
    pub fn with_config(
        contact_sync: Arc<ContactSync>,
        chat_metadata_sync: Arc<ChatMetadataSync>,
        settings_sync: Arc<SettingsSync>,
        config: AppStateSyncConfig,
    ) -> Self {
        Self {
            contact_sync,
            chat_metadata_sync,
            settings_sync,
            sync_sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
            snapshots: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start synchronization for a data type
    pub async fn start_sync(&self, data_type: AppStateDataType, ctx: &SyncContext) -> Result<String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        
        let session = SyncSession {
            session_id: session_id.clone(),
            data_type: data_type.clone(),
            state: SyncSessionState::Initializing,
            current_version: None,
            target_version: None,
            pending_mutations: Vec::new(),
            started_at: SystemTime::now(),
            last_activity: SystemTime::now(),
            retry_count: 0,
        };

        // Store session
        {
            let mut sessions = self.sync_sessions.write().await;
            sessions.insert(session_id.clone(), session);
        }

        // Start sync process
        self.process_sync_session(&session_id, ctx).await?;

        Ok(session_id)
    }

    /// Process a sync session
    async fn process_sync_session(&self, session_id: &str, ctx: &SyncContext) -> Result<()> {
        let mut session = {
            let sessions = self.sync_sessions.read().await;
            sessions.get(session_id).cloned()
                .ok_or_else(|| Error::Protocol(format!("Sync session not found: {}", session_id)))?
        };

        match session.state {
            SyncSessionState::Initializing => {
                // Check if we need a full sync or incremental sync
                let last_sync = ctx.get_last_sync(&session.data_type).await;
                
                if last_sync.is_some() && self.config.enable_incremental_sync {
                    // Perform incremental sync
                    session.state = SyncSessionState::ApplyingMutations;
                    self.perform_incremental_sync(&mut session, ctx).await?;
                } else {
                    // Request full snapshot
                    session.state = SyncSessionState::RequestingSnapshot;
                    self.request_snapshot(&mut session, ctx).await?;
                }
            }
            SyncSessionState::RequestingSnapshot => {
                // Handle snapshot request
                self.handle_snapshot_request(&mut session, ctx).await?;
            }
            SyncSessionState::ProcessingSnapshot => {
                // Process received snapshot
                self.process_snapshot(&mut session, ctx).await?;
            }
            SyncSessionState::ApplyingMutations => {
                // Apply pending mutations
                self.apply_mutations(&mut session, ctx).await?;
            }
            SyncSessionState::SendingMutations => {
                // Send local mutations to remote
                self.send_mutations(&mut session, ctx).await?;
            }
            SyncSessionState::Completed | SyncSessionState::Failed { .. } | SyncSessionState::Cancelled => {
                // Session is finished
                return Ok(());
            }
        }

        // Update session
        {
            let mut sessions = self.sync_sessions.write().await;
            sessions.insert(session_id.to_string(), session);
        }

        Ok(())
    }

    /// Request snapshot for data type
    async fn request_snapshot(&self, session: &mut SyncSession, _ctx: &SyncContext) -> Result<()> {
        // In a real implementation, this would send a snapshot request message
        // For now, we'll simulate receiving a snapshot
        
        let snapshot = self.create_local_snapshot(&session.data_type).await?;
        
        {
            let mut snapshots = self.snapshots.write().await;
            snapshots.insert(session.data_type.clone(), snapshot);
        }

        session.state = SyncSessionState::ProcessingSnapshot;
        session.last_activity = SystemTime::now();

        Ok(())
    }

    /// Handle snapshot request
    async fn handle_snapshot_request(&self, session: &mut SyncSession, ctx: &SyncContext) -> Result<()> {
        // Create snapshot of current data
        let snapshot = self.create_local_snapshot(&session.data_type).await?;
        
        // Store snapshot
        {
            let mut snapshots = self.snapshots.write().await;
            snapshots.insert(session.data_type.clone(), snapshot.clone());
        }

        // Process the snapshot
        self.apply_snapshot(&snapshot, ctx).await?;

        session.state = SyncSessionState::SendingMutations;
        session.last_activity = SystemTime::now();

        Ok(())
    }

    /// Process received snapshot
    async fn process_snapshot(&self, session: &mut SyncSession, ctx: &SyncContext) -> Result<()> {
        let snapshot = {
            let snapshots = self.snapshots.read().await;
            snapshots.get(&session.data_type).cloned()
                .ok_or_else(|| Error::Protocol("Snapshot not found".to_string()))?
        };

        self.apply_snapshot(&snapshot, ctx).await?;

        session.state = SyncSessionState::SendingMutations;
        session.current_version = Some(snapshot.version);
        session.last_activity = SystemTime::now();

        Ok(())
    }

    /// Apply snapshot data
    async fn apply_snapshot(&self, snapshot: &AppStateSnapshot, ctx: &SyncContext) -> Result<()> {
        match snapshot.data_type {
            AppStateDataType::Contacts => {
                // Parse and apply contact data
                let events: Vec<AppStateEvent> = serde_json::from_slice(&snapshot.data)
                    .map_err(|e| Error::Protocol(format!("Failed to parse contact snapshot: {}", e)))?;
                self.contact_sync.sync_from_remote(ctx, events).await?;
            }
            AppStateDataType::ChatMetadata => {
                // Parse and apply chat metadata
                let events: Vec<AppStateEvent> = serde_json::from_slice(&snapshot.data)
                    .map_err(|e| Error::Protocol(format!("Failed to parse chat metadata snapshot: {}", e)))?;
                self.chat_metadata_sync.sync_from_remote(ctx, events).await?;
            }
            AppStateDataType::Settings => {
                // Parse and apply settings
                let events: Vec<AppStateEvent> = serde_json::from_slice(&snapshot.data)
                    .map_err(|e| Error::Protocol(format!("Failed to parse settings snapshot: {}", e)))?;
                self.settings_sync.sync_from_remote(ctx, events).await?;
            }
            _ => {
                return Err(Error::Protocol(format!("Unsupported data type for snapshot: {:?}", snapshot.data_type)));
            }
        }

        Ok(())
    }

    /// Perform incremental sync
    async fn perform_incremental_sync(&self, session: &mut SyncSession, ctx: &SyncContext) -> Result<()> {
        let since = ctx.get_last_sync(&session.data_type).await
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let events = match session.data_type {
            AppStateDataType::Contacts => {
                self.contact_sync.incremental_sync(ctx, since).await?
            }
            AppStateDataType::ChatMetadata => {
                self.chat_metadata_sync.incremental_sync(ctx, since).await?
            }
            AppStateDataType::Settings => {
                self.settings_sync.incremental_sync(ctx, since).await?
            }
            _ => Vec::new(),
        };

        // Convert events to mutations
        for event in events {
            let mutation = AppStateMutation {
                mutation_id: uuid::Uuid::new_v4().to_string(),
                data_type: event.data_type,
                operation: event.operation,
                key: event.key,
                data: event.data,
                version: AppStateVersion {
                    timestamp: event.timestamp,
                    hash: "incremental_sync".to_string(),
                    device_id: "local".to_string(),
                },
                dependencies: Vec::new(),
            };
            session.pending_mutations.push(mutation);
        }

        session.state = SyncSessionState::SendingMutations;
        session.last_activity = SystemTime::now();

        Ok(())
    }

    /// Apply mutations to local state
    async fn apply_mutations(&self, session: &mut SyncSession, ctx: &SyncContext) -> Result<()> {
        let mutations = session.pending_mutations.clone();
        
        // Group mutations by data type and apply in batches
        let mut mutations_by_type: HashMap<AppStateDataType, Vec<AppStateMutation>> = HashMap::new();
        
        for mutation in mutations {
            mutations_by_type.entry(mutation.data_type.clone())
                .or_insert_with(Vec::new)
                .push(mutation);
        }

        for (data_type, type_mutations) in mutations_by_type {
            // Convert mutations to events
            let events: Vec<AppStateEvent> = type_mutations.into_iter()
                .map(|mutation| AppStateEvent {
                    data_type: mutation.data_type,
                    operation: mutation.operation,
                    timestamp: mutation.version.timestamp,
                    key: mutation.key,
                    data: mutation.data,
                })
                .collect();

            // Apply events based on data type
            match data_type {
                AppStateDataType::Contacts => {
                    self.contact_sync.sync_from_remote(ctx, events).await?;
                }
                AppStateDataType::ChatMetadata => {
                    self.chat_metadata_sync.sync_from_remote(ctx, events).await?;
                }
                AppStateDataType::Settings => {
                    self.settings_sync.sync_from_remote(ctx, events).await?;
                }
                _ => {
                    tracing::warn!("Unsupported data type for mutations: {:?}", data_type);
                }
            }
        }

        session.pending_mutations.clear();
        session.state = SyncSessionState::Completed;
        session.last_activity = SystemTime::now();

        Ok(())
    }

    /// Send local mutations to remote
    async fn send_mutations(&self, session: &mut SyncSession, ctx: &SyncContext) -> Result<()> {
        // Get local changes that need to be synced
        let events = match session.data_type {
            AppStateDataType::Contacts => {
                self.contact_sync.sync_to_remote(ctx).await?
            }
            AppStateDataType::ChatMetadata => {
                self.chat_metadata_sync.sync_to_remote(ctx).await?
            }
            AppStateDataType::Settings => {
                self.settings_sync.sync_to_remote(ctx).await?
            }
            _ => Vec::new(),
        };

        if events.is_empty() {
            session.state = SyncSessionState::Completed;
            return Ok(());
        }

        // Create patches from events
        let patches = self.create_patches(&session.data_type, events)?;

        // In a real implementation, we would send these patches to the remote server
        // For now, we'll just mark them as sent
        for patch in patches {
            tracing::debug!("Would send patch: {} with {} mutations", patch.patch_id, patch.mutations.len());
        }

        session.state = SyncSessionState::Completed;
        session.last_activity = SystemTime::now();

        Ok(())
    }

    /// Create local snapshot for data type
    async fn create_local_snapshot(&self, data_type: &AppStateDataType) -> Result<AppStateSnapshot> {
        let ctx = SyncContext::new(Arc::new(crate::database::Database::new(
            crate::database::DatabaseConfig::in_memory()
        ).await?));

        let events = match data_type {
            AppStateDataType::Contacts => {
                self.contact_sync.full_sync(&ctx).await?
            }
            AppStateDataType::ChatMetadata => {
                self.chat_metadata_sync.full_sync(&ctx).await?
            }
            AppStateDataType::Settings => {
                self.settings_sync.full_sync(&ctx).await?
            }
            _ => Vec::new(),
        };

        let data = serde_json::to_vec(&events)
            .map_err(|e| Error::Protocol(format!("Failed to serialize snapshot: {}", e)))?;

        let snapshot = AppStateSnapshot {
            data_type: data_type.clone(),
            version: AppStateVersion {
                timestamp: SystemTime::now(),
                hash: self.calculate_snapshot_hash(&data),
                device_id: "local".to_string(),
            },
            data,
            record_count: events.len() as u32,
            created_at: SystemTime::now(),
            hash: self.calculate_snapshot_hash(&data),
        };

        Ok(snapshot)
    }

    /// Create patches from events
    fn create_patches(&self, data_type: &AppStateDataType, events: Vec<AppStateEvent>) -> Result<Vec<AppStatePatch>> {
        let mut patches = Vec::new();
        let mut current_mutations = Vec::new();

        for event in events {
            let mutation = AppStateMutation {
                mutation_id: uuid::Uuid::new_v4().to_string(),
                data_type: event.data_type,
                operation: event.operation,
                key: event.key,
                data: event.data,
                version: AppStateVersion {
                    timestamp: event.timestamp,
                    hash: "local_change".to_string(),
                    device_id: "local".to_string(),
                },
                dependencies: Vec::new(),
            };

            current_mutations.push(mutation);

            // Create patch when we reach the batch size
            if current_mutations.len() >= self.config.max_mutations_per_patch {
                let patch = AppStatePatch {
                    patch_id: uuid::Uuid::new_v4().to_string(),
                    data_type: data_type.clone(),
                    mutations: current_mutations.clone(),
                    version: AppStateVersion {
                        timestamp: SystemTime::now(),
                        hash: "patch".to_string(),
                        device_id: "local".to_string(),
                    },
                    created_at: SystemTime::now(),
                };
                patches.push(patch);
                current_mutations.clear();
            }
        }

        // Create final patch if there are remaining mutations
        if !current_mutations.is_empty() {
            let patch = AppStatePatch {
                patch_id: uuid::Uuid::new_v4().to_string(),
                data_type: data_type.clone(),
                mutations: current_mutations,
                version: AppStateVersion {
                    timestamp: SystemTime::now(),
                    hash: "patch".to_string(),
                    device_id: "local".to_string(),
                },
                created_at: SystemTime::now(),
            };
            patches.push(patch);
        }

        Ok(patches)
    }

    /// Calculate snapshot hash
    fn calculate_snapshot_hash(&self, data: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Get sync session
    pub async fn get_sync_session(&self, session_id: &str) -> Option<SyncSession> {
        let sessions = self.sync_sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Cancel sync session
    pub async fn cancel_sync_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sync_sessions.write().await;
        if let Some(mut session) = sessions.get_mut(session_id) {
            session.state = SyncSessionState::Cancelled;
            session.last_activity = SystemTime::now();
        }
        Ok(())
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> Result<u32> {
        let timeout = std::time::Duration::from_secs(self.config.sync_timeout_seconds);
        let cutoff_time = SystemTime::now() - timeout;
        
        let mut sessions = self.sync_sessions.write().await;
        let initial_count = sessions.len();
        
        sessions.retain(|_, session| {
            match session.state {
                SyncSessionState::Completed | SyncSessionState::Failed { .. } | SyncSessionState::Cancelled => {
                    session.last_activity > cutoff_time
                }
                _ => {
                    session.started_at > cutoff_time
                }
            }
        });
        
        let cleaned_count = initial_count - sessions.len();
        Ok(cleaned_count as u32)
    }

    /// Get sync statistics
    pub async fn get_sync_statistics(&self) -> SyncStatistics {
        let sessions = self.sync_sessions.read().await;
        let snapshots = self.snapshots.read().await;
        
        let total_sessions = sessions.len();
        let active_sessions = sessions.values().filter(|s| {
            !matches!(s.state, SyncSessionState::Completed | SyncSessionState::Failed { .. } | SyncSessionState::Cancelled)
        }).count();
        
        let completed_sessions = sessions.values().filter(|s| {
            matches!(s.state, SyncSessionState::Completed)
        }).count();
        
        let failed_sessions = sessions.values().filter(|s| {
            matches!(s.state, SyncSessionState::Failed { .. })
        }).count();
        
        SyncStatistics {
            total_sessions,
            active_sessions,
            completed_sessions,
            failed_sessions,
            total_snapshots: snapshots.len(),
        }
    }

    /// Process incoming protocol message
    pub async fn process_protocol_message(&self, message: AppStateProtocolMessage, ctx: &SyncContext) -> Result<()> {
        match message {
            AppStateProtocolMessage::SnapshotRequest { data_type, version: _ } => {
                // Handle incoming snapshot request
                let _session_id = self.start_sync(data_type, ctx).await?;
            }
            AppStateProtocolMessage::SnapshotResponse { snapshot } => {
                // Handle incoming snapshot
                self.apply_snapshot(&snapshot, ctx).await?;
            }
            AppStateProtocolMessage::MutationPatch { patch } => {
                // Handle incoming mutation patch
                self.apply_mutation_patch(patch, ctx).await?;
            }
            AppStateProtocolMessage::SyncStatus { data_type: _, status: _, version: _ } => {
                // Handle sync status update
                tracing::debug!("Received sync status update");
            }
            AppStateProtocolMessage::SyncComplete { data_type, final_version: _ } => {
                // Handle sync completion
                ctx.update_last_sync(data_type).await;
            }
            AppStateProtocolMessage::Error { error_code, error_message, data_type: _ } => {
                // Handle error
                tracing::error!("App state sync error {}: {}", error_code, error_message);
            }
        }
        
        Ok(())
    }

    /// Apply mutation patch
    async fn apply_mutation_patch(&self, patch: AppStatePatch, ctx: &SyncContext) -> Result<()> {
        // Convert mutations to events
        let events: Vec<AppStateEvent> = patch.mutations.into_iter()
            .map(|mutation| AppStateEvent {
                data_type: mutation.data_type,
                operation: mutation.operation,
                timestamp: mutation.version.timestamp,
                key: mutation.key,
                data: mutation.data,
            })
            .collect();

        // Apply events based on data type
        match patch.data_type {
            AppStateDataType::Contacts => {
                self.contact_sync.sync_from_remote(ctx, events).await?;
            }
            AppStateDataType::ChatMetadata => {
                self.chat_metadata_sync.sync_from_remote(ctx, events).await?;
            }
            AppStateDataType::Settings => {
                self.settings_sync.sync_from_remote(ctx, events).await?;
            }
            _ => {
                return Err(Error::Protocol(format!("Unsupported data type for patch: {:?}", patch.data_type)));
            }
        }

        Ok(())
    }
}

/// Sync statistics
#[derive(Debug, Clone)]
pub struct SyncStatistics {
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub completed_sessions: usize,
    pub failed_sessions: usize,
    pub total_snapshots: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{Database, DatabaseConfig};

    #[tokio::test]
    async fn test_sync_protocol_creation() {
        let contact_sync = Arc::new(ContactSync::new());
        let chat_metadata_sync = Arc::new(ChatMetadataSync::new());
        let settings_sync = Arc::new(SettingsSync::new());

        let protocol = AppStateSyncProtocol::new(
            contact_sync,
            chat_metadata_sync,
            settings_sync,
        );

        let stats = protocol.get_sync_statistics().await;
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.active_sessions, 0);
    }

    #[tokio::test]
    async fn test_sync_session_lifecycle() {
        let contact_sync = Arc::new(ContactSync::new());
        let chat_metadata_sync = Arc::new(ChatMetadataSync::new());
        let settings_sync = Arc::new(SettingsSync::new());

        let protocol = AppStateSyncProtocol::new(
            contact_sync,
            chat_metadata_sync,
            settings_sync,
        );

        let db = Arc::new(Database::new(DatabaseConfig::in_memory()).await.unwrap());
        let ctx = SyncContext::new(db);

        let session_id = protocol.start_sync(AppStateDataType::Contacts, &ctx).await.unwrap();
        assert!(!session_id.is_empty());

        let session = protocol.get_sync_session(&session_id).await;
        assert!(session.is_some());

        let session = session.unwrap();
        assert_eq!(session.data_type, AppStateDataType::Contacts);
    }

    #[tokio::test]
    async fn test_snapshot_creation() {
        let contact_sync = Arc::new(ContactSync::new());
        let chat_metadata_sync = Arc::new(ChatMetadataSync::new());
        let settings_sync = Arc::new(SettingsSync::new());

        let protocol = AppStateSyncProtocol::new(
            contact_sync,
            chat_metadata_sync,
            settings_sync,
        );

        let snapshot = protocol.create_local_snapshot(&AppStateDataType::Contacts).await.unwrap();
        assert_eq!(snapshot.data_type, AppStateDataType::Contacts);
        assert!(snapshot.record_count >= 0);
        assert!(!snapshot.hash.is_empty());
    }
}