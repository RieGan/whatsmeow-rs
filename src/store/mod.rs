use crate::{error::Result, types::JID};
use async_trait::async_trait;

/// Device store trait for persisting device information
#[async_trait]
pub trait DeviceStore: Send + Sync {
    /// Save device data
    async fn save_device(&self, data: &DeviceData) -> Result<()>;
    
    /// Load device data
    async fn load_device(&self) -> Result<Option<DeviceData>>;
    
    /// Delete device data
    async fn delete_device(&self) -> Result<()>;
    
    /// Check if device is registered
    async fn is_registered(&self) -> Result<bool>;
}

/// Device registration data
#[derive(Debug, Clone)]
pub struct DeviceData {
    pub jid: JID,
    pub registration_id: u32,
    pub noise_key: Vec<u8>,
    pub identity_key: Vec<u8>,
    pub signed_pre_key: Vec<u8>,
    pub signed_pre_key_id: u32,
    pub signed_pre_key_signature: Vec<u8>,
}

/// In-memory device store implementation
pub struct MemoryStore {
    device_data: tokio::sync::RwLock<Option<DeviceData>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            device_data: tokio::sync::RwLock::new(None),
        }
    }
}

#[async_trait]
impl DeviceStore for MemoryStore {
    async fn save_device(&self, data: &DeviceData) -> Result<()> {
        let mut device_data = self.device_data.write().await;
        *device_data = Some(data.clone());
        Ok(())
    }
    
    async fn load_device(&self) -> Result<Option<DeviceData>> {
        let device_data = self.device_data.read().await;
        Ok(device_data.clone())
    }
    
    async fn delete_device(&self) -> Result<()> {
        let mut device_data = self.device_data.write().await;
        *device_data = None;
        Ok(())
    }
    
    async fn is_registered(&self) -> Result<bool> {
        let device_data = self.device_data.read().await;
        Ok(device_data.is_some())
    }
}