use async_trait::async_trait;
use chrononode_core::{CoreError, Result, StorageBackend, StorageHealth, StoragePointer};
use object_store::aws::AmazonS3Builder;
use object_store::{path::Path, ObjectStore, ObjectStoreExt, PutPayload};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Instant;

pub struct S3Backend {
    store: Arc<dyn ObjectStore>,
}

impl S3Backend {
    pub fn new(bucket: &str, region: &str, endpoint: Option<&str>) -> Result<Self> {
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(bucket)
            .with_region(region);

        if let Some(endpoint) = endpoint {
            builder = builder.with_endpoint(endpoint);
        }

        let store = builder
            .build()
            .map_err(|e| CoreError::Storage(format!("S3 client build failed: {}", e)))?;

        Ok(Self {
            store: Arc::new(store),
        })
    }

    pub fn from_env() -> Result<Self> {
        let bucket = std::env::var("CHRONONODE_S3_BUCKET")
            .map_err(|_| CoreError::Storage("CHRONONODE_S3_BUCKET not set".to_string()))?;
        let region =
            std::env::var("CHRONONODE_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let endpoint = std::env::var("CHRONONODE_S3_ENDPOINT").ok();
        Self::new(&bucket, &region, endpoint.as_deref())
    }

    fn object_path(key: &str) -> Path {
        Path::from(format!("chrononode/{}", key))
    }
}

#[async_trait]
impl StorageBackend for S3Backend {
    async fn put(&self, bytes: &[u8]) -> Result<StoragePointer> {
        let hash = Sha256::digest(bytes);
        let key = hex::encode(hash);
        let path = Self::object_path(&key);
        let object_bytes = PutPayload::from(bytes.to_vec());

        self.store
            .put(&path, object_bytes)
            .await
            .map_err(|e| CoreError::Storage(format!("S3 put failed for {}: {}", key, e)))?;

        Ok(StoragePointer::new("s3", key))
    }

    async fn get(&self, pointer: &StoragePointer) -> Result<Vec<u8>> {
        let path = Self::object_path(&pointer.key);
        let result =
            self.store.get(&path).await.map_err(|e| {
                CoreError::Storage(format!("S3 get failed for {}: {}", pointer.key, e))
            })?;

        let bytes = result
            .bytes()
            .await
            .map_err(|e| CoreError::Storage(format!("S3 read failed for {}: {}", pointer.key, e)))?
            .to_vec();

        let computed = Sha256::digest(&bytes);
        let computed_hex = hex::encode(computed);
        if computed_hex != pointer.key {
            return Err(CoreError::Storage(format!(
                "Content mismatch for S3 key {}: expected hash {} got {}",
                pointer.key, pointer.key, computed_hex
            )));
        }

        Ok(bytes)
    }

    async fn pin(&self, _pointer: &StoragePointer) -> Result<()> {
        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealth> {
        let start = Instant::now();
        let result = self.store.head(&Path::from("_health_check")).await;
        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(_) | Err(object_store::Error::NotFound { .. }) => Ok(StorageHealth {
                healthy: true,
                latency_ms: elapsed,
                message: "OK".to_string(),
            }),
            Err(e) => Ok(StorageHealth {
                healthy: false,
                latency_ms: elapsed,
                message: format!("S3 health check failed: {}", e),
            }),
        }
    }
}
