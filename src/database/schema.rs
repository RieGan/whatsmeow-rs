/// Database schema definitions for WhatsApp client

/// Database schema version
pub const SCHEMA_VERSION: i32 = 1;

/// SQL statements for creating tables
pub const CREATE_TABLES: &[&str] = &[
    // Device information table
    r#"
    CREATE TABLE IF NOT EXISTS devices (
        id INTEGER PRIMARY KEY,
        jid TEXT NOT NULL UNIQUE,
        registration_id INTEGER NOT NULL,
        noise_key BLOB NOT NULL,
        identity_key BLOB NOT NULL,
        signed_pre_key BLOB NOT NULL,
        signed_pre_key_id INTEGER NOT NULL,
        signed_pre_key_signature BLOB NOT NULL,
        push_token TEXT,
        server_token TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
    "#,
    
    // Identity keys for Signal protocol
    r#"
    CREATE TABLE IF NOT EXISTS identity_keys (
        address TEXT PRIMARY KEY,
        identity_key BLOB NOT NULL,
        trust_level INTEGER NOT NULL DEFAULT 0,
        registration_id INTEGER,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
    "#,
    
    // Session state for Signal protocol
    r#"
    CREATE TABLE IF NOT EXISTS sessions (
        address TEXT PRIMARY KEY,
        device_id INTEGER NOT NULL,
        session_data BLOB NOT NULL,
        local_registration_id INTEGER NOT NULL,
        remote_registration_id INTEGER NOT NULL,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
    "#,
    
    // Pre-keys for Signal protocol
    r#"
    CREATE TABLE IF NOT EXISTS pre_keys (
        key_id INTEGER PRIMARY KEY,
        public_key BLOB NOT NULL,
        private_key BLOB NOT NULL,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
    "#,
    
    // Signed pre-keys for Signal protocol
    r#"
    CREATE TABLE IF NOT EXISTS signed_pre_keys (
        key_id INTEGER PRIMARY KEY,
        public_key BLOB NOT NULL,
        private_key BLOB NOT NULL,
        signature BLOB NOT NULL,
        timestamp INTEGER NOT NULL,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
    "#,
    
    // Group sessions for Signal protocol
    r#"
    CREATE TABLE IF NOT EXISTS group_sessions (
        group_id TEXT NOT NULL,
        sender_key_id INTEGER NOT NULL,
        session_data BLOB NOT NULL,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (group_id, sender_key_id)
    )
    "#,
    
    // Sender keys for group messaging
    r#"
    CREATE TABLE IF NOT EXISTS sender_keys (
        group_id TEXT NOT NULL,
        sender_id TEXT NOT NULL,
        device_id INTEGER NOT NULL,
        sender_key_data BLOB NOT NULL,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (group_id, sender_id, device_id)
    )
    "#,
    
    // Groups information
    r#"
    CREATE TABLE IF NOT EXISTS groups (
        jid TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        creator TEXT NOT NULL,
        created_at DATETIME NOT NULL,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        avatar_url TEXT,
        avatar_id TEXT,
        invite_link TEXT,
        settings_json TEXT
    )
    "#,
    
    // Group participants
    r#"
    CREATE TABLE IF NOT EXISTS group_participants (
        group_jid TEXT NOT NULL,
        participant_jid TEXT NOT NULL,
        role INTEGER NOT NULL DEFAULT 2, -- 0=Creator, 1=Admin, 2=Member
        joined_at DATETIME NOT NULL,
        added_by TEXT,
        permissions_json TEXT,
        status INTEGER NOT NULL DEFAULT 0, -- 0=Active, 1=Muted, 2=Kicked, etc.
        PRIMARY KEY (group_jid, participant_jid),
        FOREIGN KEY (group_jid) REFERENCES groups(jid) ON DELETE CASCADE
    )
    "#,
    
    // Contacts
    r#"
    CREATE TABLE IF NOT EXISTS contacts (
        jid TEXT PRIMARY KEY,
        name TEXT,
        notify_name TEXT,
        phone_number TEXT,
        avatar_url TEXT,
        status_text TEXT,
        last_seen DATETIME,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
    "#,
    
    // Messages
    r#"
    CREATE TABLE IF NOT EXISTS messages (
        id TEXT PRIMARY KEY,
        from_jid TEXT NOT NULL,
        to_jid TEXT NOT NULL,
        chat_jid TEXT NOT NULL, -- Group JID or individual JID
        message_type INTEGER NOT NULL, -- 0=Text, 1=Image, 2=Video, etc.
        content TEXT,
        media_type TEXT,
        media_url TEXT,
        media_sha256 TEXT,
        media_size INTEGER,
        thumbnail BLOB,
        quoted_message_id TEXT,
        timestamp DATETIME NOT NULL,
        status INTEGER NOT NULL DEFAULT 0, -- 0=Pending, 1=Sent, 2=Delivered, 3=Read
        is_from_me BOOLEAN NOT NULL DEFAULT FALSE,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (quoted_message_id) REFERENCES messages(id)
    )
    "#,
    
    // Chat sessions
    r#"
    CREATE TABLE IF NOT EXISTS chats (
        jid TEXT PRIMARY KEY,
        name TEXT,
        chat_type INTEGER NOT NULL DEFAULT 0, -- 0=Individual, 1=Group, 2=Broadcast
        last_message_id TEXT,
        last_message_time DATETIME,
        unread_count INTEGER DEFAULT 0,
        muted_until DATETIME,
        archived BOOLEAN DEFAULT FALSE,
        pinned BOOLEAN DEFAULT FALSE,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (last_message_id) REFERENCES messages(id)
    )
    "#,
    
    // Media files storage
    r#"
    CREATE TABLE IF NOT EXISTS media_files (
        sha256 TEXT PRIMARY KEY,
        file_path TEXT NOT NULL,
        file_size INTEGER NOT NULL,
        mime_type TEXT NOT NULL,
        encryption_key BLOB,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
    "#,
    
    // Application settings and key-value store
    r#"
    CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
    "#,
    
    // Schema version tracking
    r#"
    CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER PRIMARY KEY,
        applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
    "#,
];

/// SQL statements for creating indexes
pub const CREATE_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_messages_chat_timestamp ON messages(chat_jid, timestamp)",
    "CREATE INDEX IF NOT EXISTS idx_messages_from_jid ON messages(from_jid)",
    "CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp)",
    "CREATE INDEX IF NOT EXISTS idx_messages_status ON messages(status)",
    "CREATE INDEX IF NOT EXISTS idx_sessions_device_id ON sessions(device_id)",
    "CREATE INDEX IF NOT EXISTS idx_group_participants_role ON group_participants(role)",
    "CREATE INDEX IF NOT EXISTS idx_contacts_phone ON contacts(phone_number)",
    "CREATE INDEX IF NOT EXISTS idx_chats_last_message_time ON chats(last_message_time)",
    "CREATE INDEX IF NOT EXISTS idx_chats_type ON chats(chat_type)",
    "CREATE INDEX IF NOT EXISTS idx_media_files_mime_type ON media_files(mime_type)",
];

/// SQL triggers for automatic timestamp updates
pub const CREATE_TRIGGERS: &[&str] = &[
    r#"
    CREATE TRIGGER IF NOT EXISTS update_devices_timestamp 
    AFTER UPDATE ON devices
    BEGIN
        UPDATE devices SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END
    "#,
    
    r#"
    CREATE TRIGGER IF NOT EXISTS update_identity_keys_timestamp 
    AFTER UPDATE ON identity_keys
    BEGIN
        UPDATE identity_keys SET updated_at = CURRENT_TIMESTAMP WHERE address = NEW.address;
    END
    "#,
    
    r#"
    CREATE TRIGGER IF NOT EXISTS update_sessions_timestamp 
    AFTER UPDATE ON sessions
    BEGIN
        UPDATE sessions SET updated_at = CURRENT_TIMESTAMP WHERE address = NEW.address;
    END
    "#,
    
    r#"
    CREATE TRIGGER IF NOT EXISTS update_groups_timestamp 
    AFTER UPDATE ON groups
    BEGIN
        UPDATE groups SET updated_at = CURRENT_TIMESTAMP WHERE jid = NEW.jid;
    END
    "#,
    
    r#"
    CREATE TRIGGER IF NOT EXISTS update_contacts_timestamp 
    AFTER UPDATE ON contacts
    BEGIN
        UPDATE contacts SET updated_at = CURRENT_TIMESTAMP WHERE jid = NEW.jid;
    END
    "#,
    
    r#"
    CREATE TRIGGER IF NOT EXISTS update_chats_timestamp 
    AFTER UPDATE ON chats
    BEGIN
        UPDATE chats SET updated_at = CURRENT_TIMESTAMP WHERE jid = NEW.jid;
    END
    "#,
    
    r#"
    CREATE TRIGGER IF NOT EXISTS update_chat_on_new_message
    AFTER INSERT ON messages
    BEGIN
        INSERT OR REPLACE INTO chats (jid, last_message_id, last_message_time, chat_type)
        VALUES (
            NEW.chat_jid, 
            NEW.id, 
            NEW.timestamp,
            CASE WHEN NEW.chat_jid LIKE '%@g.us' THEN 1 ELSE 0 END
        );
        
        UPDATE chats 
        SET unread_count = unread_count + CASE WHEN NEW.is_from_me THEN 0 ELSE 1 END
        WHERE jid = NEW.chat_jid;
    END
    "#,
];

/// Table information for introspection
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
}

/// Get information about all tables in the database
pub async fn get_table_info(pool: &sqlx::SqlitePool) -> Result<Vec<TableInfo>, sqlx::Error> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
    )
    .fetch_all(pool)
    .await?;
    
    let mut table_infos = Vec::new();
    
    for table in tables {
        let columns: Vec<(String, String, bool, bool)> = sqlx::query_as(
            &format!("PRAGMA table_info({})", table)
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row: (i32, String, String, bool, Option<String>, bool)| {
            (row.1, row.2, !row.3, row.5)
        })
        .collect();
        
        let column_infos = columns
            .into_iter()
            .map(|(name, data_type, nullable, primary_key)| ColumnInfo {
                name,
                data_type,
                nullable,
                primary_key,
            })
            .collect();
        
        table_infos.push(TableInfo {
            name: table,
            columns: column_infos,
        });
    }
    
    Ok(table_infos)
}