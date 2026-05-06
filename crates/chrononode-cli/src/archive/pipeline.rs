use chrononode_core::{ChainAdapter, ChronoBlock, Result, StorageBackend, StoragePointer};
use std::sync::Arc;

pub struct ArchivePipeline {
    pub adapter: Arc<dyn ChainAdapter>,
    pub storage: Arc<dyn StorageBackend>,
    pub index: Arc<super::super::index::sqlite::SqliteIndex>,
}

impl ArchivePipeline {
    pub fn new(
        adapter: Arc<dyn ChainAdapter>,
        storage: Arc<dyn StorageBackend>,
        index: Arc<super::super::index::sqlite::SqliteIndex>,
    ) -> Self {
        Self { adapter, storage, index }
    }

    pub async fn archive_block(&self, height: u64) -> Result<(ChronoBlock, StoragePointer)> {
        let block = self.adapter.fetch_block(height).await?;
        let bytes = super::serializer::serialize_block(&block)?;
        let pointer = self.storage.put(&bytes).await?;
        self.storage.pin(&pointer).await?;
        self.index
            .insert_block(
                &block.chain_id,
                height,
                &block.block_hash,
                &block.block_hash_hex(),
                &block.prev_hash,
                &pointer.backend,
                &pointer.to_string(),
                block.timestamp,
                bytes.len() as u64,
            )
            .await?;
        self.index.insert_txns_for_block(
            &block.chain_id,
            height,
            &block.block_hash_hex(),
            &block.transactions,
        ).await?;
        self.index.insert_events_for_block(
            &block.chain_id,
            height,
            &block.block_hash_hex(),
            &block.events,
        ).await?;
        self.index.update_ingest_state(&block.chain_id, height).await?;
        Ok((block, pointer))
    }

    pub async fn archive_range(&self, from: u64, to: u64) -> Result<Vec<(ChronoBlock, StoragePointer)>> {
        let mut results = Vec::with_capacity((to - from + 1) as usize);
        for h in from..=to {
            let result = self.archive_block(h).await?;
            results.push(result);
        }
        Ok(results)
    }

    pub async fn latest_archived_height(&self, chain_id: &str) -> Result<Option<u64>> {
        self.index.get_latest_archived_height(chain_id).await
    }

    pub async fn get_block_by_height(&self, chain_id: &str, height: u64) -> Result<ChronoBlock> {
        let (_backend, pointer_str) = self.index.get_block_location(chain_id, height).await?;
        let pointer = StoragePointer::from_string(&pointer_str)
            .ok_or_else(|| chrononode_core::CoreError::NotFound("invalid storage pointer".to_string()))?;
        let bytes = self.storage.get(&pointer).await?;
        super::serializer::deserialize_block(&bytes)
    }
}
