use crate::block::ChronoBlock;
use crate::Result;
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq)]
pub enum BlockModel {
    Utxo,
    Account,
    EventLedger,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct StoragePointer {
    pub backend: String,
    pub key: String,
}

impl StoragePointer {
    pub fn new(backend: impl Into<String>, key: impl Into<String>) -> Self {
        Self { backend: backend.into(), key: key.into() }
    }

    pub fn to_string(&self) -> String {
        format!("{}:{}", self.backend, self.key)
    }

    pub fn from_string(s: &str) -> Option<Self> {
        let colon = s.find(':')?;
        Some(Self {
            backend: s[..colon].to_string(),
            key: s[colon + 1..].to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct StorageHealth {
    pub healthy: bool,
    pub latency_ms: u64,
    pub message: String,
}

#[async_trait]
pub trait ChainAdapter: Send + Sync {
    fn chain_id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn block_model(&self) -> BlockModel;

    async fn latest_height(&self) -> Result<u64>;

    async fn fetch_block(&self, height: u64) -> Result<ChronoBlock>;

    async fn fetch_block_by_hash(&self, hash: &[u8]) -> Result<ChronoBlock>;

    async fn fetch_range(&self, from: u64, to: u64) -> Result<Vec<ChronoBlock>> {
        let mut blocks = Vec::with_capacity((to - from + 1) as usize);
        for h in from..=to {
            blocks.push(self.fetch_block(h).await?);
        }
        Ok(blocks)
    }
}

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn put(&self, bytes: &[u8]) -> Result<StoragePointer>;

    async fn get(&self, pointer: &StoragePointer) -> Result<Vec<u8>>;

    async fn pin(&self, pointer: &StoragePointer) -> Result<()>;

    async fn health_check(&self) -> Result<StorageHealth>;
}
