use async_trait::async_trait;
use chrononode_core::{Result, StorageBackend, StorageHealth, StoragePointer};

pub struct PinataBackend {
    api_key: String,
    api_secret: String,
}

impl PinataBackend {
    pub fn new(_endpoint: &str) -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
        }
    }
}

#[async_trait]
impl StorageBackend for PinataBackend {
    async fn put(&self, _bytes: &[u8]) -> Result<StoragePointer> {
        Err(chrononode_core::CoreError::Storage("Pinata backend not yet implemented".to_string()))
    }

    async fn get(&self, _pointer: &StoragePointer) -> Result<Vec<u8>> {
        Err(chrononode_core::CoreError::Storage("Pinata backend not yet implemented".to_string()))
    }

    async fn pin(&self, _pointer: &StoragePointer) -> Result<()> {
        Err(chrononode_core::CoreError::Storage("Pinata backend not yet implemented".to_string()))
    }

    async fn health_check(&self) -> Result<StorageHealth> {
        Ok(StorageHealth {
            healthy: false,
            latency_ms: 0,
            message: "Pinata backend not yet implemented".to_string(),
        })
    }
}
