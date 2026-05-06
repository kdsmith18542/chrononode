use async_trait::async_trait;
use chrononode_core::{Result, StorageBackend, StorageHealth, StoragePointer};

pub struct IpfsBackend {
    api_url: String,
}

impl IpfsBackend {
    pub fn new(api_url: &str) -> Self {
        Self { api_url: api_url.to_string() }
    }
}

#[async_trait]
impl StorageBackend for IpfsBackend {
    async fn put(&self, _bytes: &[u8]) -> Result<StoragePointer> {
        Err(chrononode_core::CoreError::Storage("IPFS backend not yet implemented".to_string()))
    }

    async fn get(&self, _pointer: &StoragePointer) -> Result<Vec<u8>> {
        Err(chrononode_core::CoreError::Storage("IPFS backend not yet implemented".to_string()))
    }

    async fn pin(&self, _pointer: &StoragePointer) -> Result<()> {
        Err(chrononode_core::CoreError::Storage("IPFS backend not yet implemented".to_string()))
    }

    async fn health_check(&self) -> Result<StorageHealth> {
        Ok(StorageHealth {
            healthy: false,
            latency_ms: 0,
            message: "IPFS backend not yet implemented".to_string(),
        })
    }
}
