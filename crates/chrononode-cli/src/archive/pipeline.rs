use chrononode_core::{ChainAdapter, ChronoBlock, Result, StorageBackend, StoragePointer};
use std::sync::Arc;

use crate::index::{sqlite::ArchivedBlockInsert, IndexBackend};

use super::cache::CacheLayer;

pub struct ArchivePipeline {
    pub adapter: Arc<tokio::sync::RwLock<Arc<dyn ChainAdapter>>>,
    pub storage: Arc<dyn StorageBackend>,
    pub index: Box<dyn IndexBackend>,
    pub cache: CacheLayer,
    pub config: chrononode_core::CoreConfig,
}

impl ArchivePipeline {
    pub fn new(
        adapter: Arc<dyn ChainAdapter>,
        storage: Arc<dyn StorageBackend>,
        index: Box<dyn IndexBackend>,
    ) -> Self {
        Self {
            adapter: Arc::new(tokio::sync::RwLock::new(adapter)),
            storage,
            index,
            cache: CacheLayer::default(),
            config: chrononode_core::CoreConfig::default(),
        }
    }

    pub fn with_config(
        adapter: Arc<dyn ChainAdapter>,
        storage: Arc<dyn StorageBackend>,
        index: Box<dyn IndexBackend>,
        config: chrononode_core::CoreConfig,
    ) -> Self {
        Self {
            adapter: Arc::new(tokio::sync::RwLock::new(adapter)),
            storage,
            index,
            cache: CacheLayer::default(),
            config,
        }
    }

    pub fn with_cache(
        adapter: Arc<dyn ChainAdapter>,
        storage: Arc<dyn StorageBackend>,
        index: Box<dyn IndexBackend>,
        cache: CacheLayer,
    ) -> Self {
        Self {
            adapter: Arc::new(tokio::sync::RwLock::new(adapter)),
            storage,
            index,
            cache,
            config: chrononode_core::CoreConfig::default(),
        }
    }

    pub async fn get_adapter(&self) -> Arc<dyn ChainAdapter> {
        self.adapter.read().await.clone()
    }

    pub async fn archive_block(&self, height: u64) -> Result<(ChronoBlock, StoragePointer)> {
        if let Some(pending) = self.cache.pending.get(&height) {
            let block = (*pending).clone();
            let bytes = super::serializer::serialize_block(&block, self.config.compression)?;

            let start_put = std::time::Instant::now();
            let put_res = self.storage.put(&bytes).await;
            let duration_put = start_put.elapsed();
            let backend = match &put_res {
                Ok(p) => p.backend.as_str(),
                Err(_) => "unknown",
            };
            crate::metrics::record_storage_operation(backend, "put", put_res.is_ok(), duration_put);
            let pointer = put_res?;

            let start_pin = std::time::Instant::now();
            let pin_res = self.storage.pin(&pointer).await;
            let duration_pin = start_pin.elapsed();
            crate::metrics::record_storage_operation(
                &pointer.backend,
                "pin",
                pin_res.is_ok(),
                duration_pin,
            );
            pin_res?;

            let result = (block.clone(), pointer);
            self.cache.archived.insert(height, Arc::new(result.clone()));
            crate::metrics::record_block_archived(&block.chain_id, height);
            return Ok(result);
        }

        let adapter = self.get_adapter().await;
        let block = adapter.fetch_block(height).await?;
        let bytes = super::serializer::serialize_block(&block, self.config.compression)?;
        let block_hash_hex = block.block_hash_hex();

        if self
            .index
            .check_reorg(&block.chain_id, height, &block_hash_hex)
            .await?
        {
            tracing::warn!(
                "Reorg detected at {} block {}: stored hash differs from fetched hash",
                block.chain_id,
                height
            );
            self.index.mark_degraded(&block.chain_id, height).await?;
        }

        let start_put = std::time::Instant::now();
        let put_res = self.storage.put(&bytes).await;
        let duration_put = start_put.elapsed();
        let backend = match &put_res {
            Ok(p) => p.backend.as_str(),
            Err(_) => "unknown",
        };
        crate::metrics::record_storage_operation(backend, "put", put_res.is_ok(), duration_put);
        let pointer = put_res?;

        let pointer_str = pointer.to_string();

        let start_pin = std::time::Instant::now();
        let pin_res = self.storage.pin(&pointer).await;
        let duration_pin = start_pin.elapsed();
        crate::metrics::record_storage_operation(
            &pointer.backend,
            "pin",
            pin_res.is_ok(),
            duration_pin,
        );
        pin_res?;

        let insert = ArchivedBlockInsert {
            chain_id: &block.chain_id,
            height,
            block_hash: &block.block_hash,
            block_hash_hex: &block_hash_hex,
            prev_hash: &block.prev_hash,
            storage_backend: &pointer.backend,
            storage_pointer: &pointer_str,
            timestamp: block.timestamp,
            byte_size: bytes.len() as u64,
        };

        self.index
            .archive_block_atomic(&insert, &block.transactions, &block.events)
            .await?;

        // Scan transactions against watch list and record activity
        let chain_id = &block.chain_id;
        for tx in &block.transactions {
            let sender_hex = hex::encode(&tx.sender);
            let recipient_hex = hex::encode(&tx.recipient);
            let tx_hash_hex = tx.tx_hash_hex();

            if self
                .index
                .is_address_watched(chain_id, &sender_hex)
                .await
                .unwrap_or(false)
            {
                if let Err(e) = self
                    .index
                    .record_activity(chain_id, &sender_hex, height, &tx_hash_hex)
                    .await
                {
                    tracing::warn!("Failed to record sender activity: {}", e);
                }
            }
            if self
                .index
                .is_address_watched(chain_id, &recipient_hex)
                .await
                .unwrap_or(false)
            {
                if let Err(e) = self
                    .index
                    .record_activity(chain_id, &recipient_hex, height, &tx_hash_hex)
                    .await
                {
                    tracing::warn!("Failed to record recipient activity: {}", e);
                }
            }
        }

        // Trigger block pruning logic based on configuration
        let mut prunable_blocks = Vec::new();
        match self.config.pruning.mode {
            chrononode_core::PruningMode::Height => {
                if height > self.config.pruning.keep_blocks {
                    let before_height = height - self.config.pruning.keep_blocks;
                    prunable_blocks = self
                        .index
                        .get_prunable_blocks_by_height(&block.chain_id, before_height)
                        .await?;
                }
            }
            chrononode_core::PruningMode::Age => {
                let current_time = block.timestamp;
                if current_time > self.config.pruning.keep_duration_secs {
                    let before_timestamp = current_time - self.config.pruning.keep_duration_secs;
                    prunable_blocks = self
                        .index
                        .get_prunable_blocks_by_age(&block.chain_id, before_timestamp)
                        .await?;
                }
            }
            chrononode_core::PruningMode::None => {}
        }

        let mut pruned_heights = Vec::new();
        if !prunable_blocks.is_empty() {
            for (h, ptr_str) in prunable_blocks {
                if let Some(ptr) = StoragePointer::from_string(&ptr_str) {
                    if let Err(e) = self.storage.delete(&ptr).await {
                        tracing::warn!(
                            "Failed to delete pruned block storage at height {}: {}",
                            h,
                            e
                        );
                    }
                }
                pruned_heights.push(h);
            }
            self.index
                .set_blocks_pruned(&block.chain_id, &pruned_heights)
                .await?;
        }

        // If UTXO pruning is active, trigger spent UTXO cleanup
        if self.config.pruning.prune_utxos {
            match self.config.pruning.mode {
                chrononode_core::PruningMode::Height => {
                    if height > self.config.pruning.keep_blocks {
                        let before_height = height - self.config.pruning.keep_blocks;
                        self.index
                            .prune_spent_utxos(&block.chain_id, before_height)
                            .await?;
                    }
                }
                chrononode_core::PruningMode::Age => {
                    if let Some(&max_h) = pruned_heights.iter().max() {
                        self.index.prune_spent_utxos(&block.chain_id, max_h).await?;
                    }
                }
                chrononode_core::PruningMode::None => {}
            }
        }

        self.cache
            .archived
            .insert(height, Arc::new((block.clone(), pointer.clone())));
        crate::metrics::record_block_archived(&block.chain_id, height);
        Ok((block, pointer))
    }

    pub async fn archive_range(
        &self,
        from: u64,
        to: u64,
    ) -> Result<Vec<(ChronoBlock, StoragePointer)>> {
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
        if let Some(cached) = self.cache.archived.get(&height) {
            return Ok(cached.0.clone());
        }

        let (_backend, pointer_str) = self.index.get_block_location(chain_id, height).await?;
        let pointer = StoragePointer::from_string(&pointer_str).ok_or_else(|| {
            chrononode_core::CoreError::NotFound("invalid storage pointer".to_string())
        })?;
        let start_get = std::time::Instant::now();
        let get_res = self.storage.get(&pointer).await;
        let duration_get = start_get.elapsed();
        crate::metrics::record_storage_operation(
            &pointer.backend,
            "get",
            get_res.is_ok(),
            duration_get,
        );
        let bytes = get_res?;
        super::serializer::deserialize_block(&bytes)
    }

    pub async fn get_block_by_hash(&self, chain_id: &str, block_hash: &str) -> Result<ChronoBlock> {
        let cache_key = format!("{}:{}", chain_id, block_hash);
        if let Some(cached) = self.cache.by_hash.get(&cache_key) {
            return Ok((*cached).clone());
        }

        let (_backend, pointer_str) = self
            .index
            .get_block_location_by_hash(chain_id, block_hash)
            .await?;
        let pointer = StoragePointer::from_string(&pointer_str).ok_or_else(|| {
            chrononode_core::CoreError::NotFound("invalid storage pointer".to_string())
        })?;
        let start_get = std::time::Instant::now();
        let get_res = self.storage.get(&pointer).await;
        let duration_get = start_get.elapsed();
        crate::metrics::record_storage_operation(
            &pointer.backend,
            "get",
            get_res.is_ok(),
            duration_get,
        );
        let bytes = get_res?;
        let block = super::serializer::deserialize_block(&bytes)?;
        self.cache
            .by_hash
            .insert(cache_key, Arc::new(block.clone()));
        Ok(block)
    }
}
