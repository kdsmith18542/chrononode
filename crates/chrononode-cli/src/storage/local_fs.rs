use async_trait::async_trait;
use chrononode_core::{Result, StorageBackend, StorageHealth, StoragePointer};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct LocalFsBackend {
    base_path: PathBuf,
}

impl LocalFsBackend {
    pub fn new(base_path: &str) -> Self {
        let path = PathBuf::from(base_path);
        std::fs::create_dir_all(&path).ok();
        Self { base_path: path }
    }

    fn object_path(&self, pointer: &StoragePointer) -> PathBuf {
        self.base_path.join(&pointer.key)
    }
}

#[async_trait]
impl StorageBackend for LocalFsBackend {
    async fn put(&self, bytes: &[u8]) -> Result<StoragePointer> {
        let hash = Sha256::digest(bytes);
        let key = hex::encode(hash);
        let path = self.base_path.join(&key);
        std::fs::create_dir_all(path.parent().unwrap_or(&self.base_path))?;
        std::fs::write(&path, bytes)?;
        Ok(StoragePointer::new("local_fs", key))
    }

    async fn get(&self, pointer: &StoragePointer) -> Result<Vec<u8>> {
        let path = self.object_path(pointer);
        let bytes = std::fs::read(&path)?;
        let computed = Sha256::digest(&bytes);
        let expected_hex = &pointer.key;
        let computed_hex = hex::encode(computed);
        if computed_hex != *expected_hex {
            return Err(chrononode_core::CoreError::Storage(format!(
                "Content mismatch for {}: expected {} got {}",
                pointer.key, expected_hex, computed_hex
            )));
        }
        Ok(bytes)
    }

    async fn pin(&self, _pointer: &StoragePointer) -> Result<()> {
        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealth> {
        let ok = self.base_path.exists();
        Ok(StorageHealth {
            healthy: ok,
            latency_ms: 0,
            message: if ok { "OK".to_string() } else { "path missing".to_string() },
        })
    }
}
