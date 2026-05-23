use async_trait::async_trait;
use chrononode_core::{CoreError, Result, StorageBackend, StorageHealth, StoragePointer};
use std::sync::Arc;

pub struct FallbackStorage {
    primary: Arc<dyn StorageBackend>,
    secondary: Arc<dyn StorageBackend>,
}

impl FallbackStorage {
    pub fn new(primary: Arc<dyn StorageBackend>, secondary: Arc<dyn StorageBackend>) -> Self {
        Self { primary, secondary }
    }
}

#[async_trait]
impl StorageBackend for FallbackStorage {
    async fn put(&self, bytes: &[u8]) -> Result<StoragePointer> {
        let pointer = self.primary.put(bytes).await?;
        if let Err(e) = self.secondary.put(bytes).await {
            tracing::warn!("Secondary storage put failed: {}", e);
        }
        Ok(pointer)
    }

    async fn get(&self, pointer: &StoragePointer) -> Result<Vec<u8>> {
        match self.primary.get(pointer).await {
            Ok(bytes) => Ok(bytes),
            Err(CoreError::NotFound(_)) => {
                tracing::info!(
                    "Primary storage miss for {}, falling back to secondary",
                    pointer
                );
                self.secondary.get(pointer).await
            }
            Err(e) => Err(e),
        }
    }

    async fn pin(&self, pointer: &StoragePointer) -> Result<()> {
        self.primary.pin(pointer).await?;
        if let Err(e) = self.secondary.pin(pointer).await {
            tracing::warn!("Secondary storage pin failed: {}", e);
        }
        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealth> {
        let primary_health = self.primary.health_check().await.unwrap_or(StorageHealth {
            healthy: false,
            latency_ms: 0,
            message: "primary unreachable".to_string(),
        });
        let secondary_health = self
            .secondary
            .health_check()
            .await
            .unwrap_or(StorageHealth {
                healthy: false,
                latency_ms: 0,
                message: "secondary unreachable".to_string(),
            });
        Ok(StorageHealth {
            healthy: primary_health.healthy || secondary_health.healthy,
            latency_ms: primary_health.latency_ms.max(secondary_health.latency_ms),
            message: format!(
                "primary: {}, secondary: {}",
                primary_health.message, secondary_health.message
            ),
        })
    }
}
