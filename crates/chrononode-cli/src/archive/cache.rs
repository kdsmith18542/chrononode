use chrononode_core::{ChronoBlock, StoragePointer};
use moka::sync::Cache;
use std::sync::Arc;

#[derive(Clone)]
pub struct CacheLayer {
    pub pending: Cache<u64, Arc<ChronoBlock>>,
    pub archived: Cache<u64, Arc<(ChronoBlock, StoragePointer)>>,
    pub by_hash: Cache<String, Arc<ChronoBlock>>,
}

impl CacheLayer {
    pub fn with_capacity(capacity: u64) -> Self {
        Self {
            pending: Cache::new(capacity),
            archived: Cache::new(capacity),
            by_hash: Cache::new(capacity),
        }
    }
}

impl Default for CacheLayer {
    fn default() -> Self {
        Self::with_capacity(10_000)
    }
}
