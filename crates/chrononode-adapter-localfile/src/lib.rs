use async_trait::async_trait;
use chrononode_core::{BlockModel, ChainAdapter, ChronoBlock, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct LocalFileAdapter {
    chain_id: String,
    display_name: String,
    blocks: BTreeMap<u64, ChronoBlock>,
    block_hash_to_height: BTreeMap<String, u64>,
}

impl LocalFileAdapter {
    pub fn new(base_path: &str) -> anyhow::Result<Self> {
        let path = PathBuf::from(base_path);
        let mut blocks = BTreeMap::new();
        let mut block_hash_to_height = BTreeMap::new();

        if path.is_file() {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(block) = serde_json::from_str::<ChronoBlock>(&content) {
                let hash_hex = hex::encode(&block.block_hash);
                block_hash_to_height.insert(hash_hex, block.height);
                blocks.insert(block.height, block);
            } else if let Ok(blocks_array) = serde_json::from_str::<Vec<ChronoBlock>>(&content) {
                for block in blocks_array {
                    let hash_hex = hex::encode(&block.block_hash);
                    block_hash_to_height.insert(hash_hex, block.height);
                    blocks.insert(block.height, block);
                }
            } else {
                anyhow::bail!("Failed to parse JSON file as ChronoBlock or array of ChronoBlocks");
            }
        } else if path.is_dir() {
            for entry in glob::glob(&format!("{}/**/*.json", path.display()))? {
                let entry = entry?;
                if let Ok(content) = std::fs::read_to_string(&entry) {
                    if let Ok(block) = serde_json::from_str::<ChronoBlock>(&content) {
                        let hash_hex = hex::encode(&block.block_hash);
                        block_hash_to_height.insert(hash_hex, block.height);
                        blocks.insert(block.height, block);
                    }
                }
            }
        } else {
            anyhow::bail!("Path does not exist: {}", path.display());
        }

        let chain_id = blocks
            .values()
            .next()
            .map(|b| b.chain_id.clone())
            .unwrap_or_else(|| "local-file".to_string());

        Ok(Self {
            chain_id,
            display_name: format!("Local File ({})", path.display()),
            blocks,
            block_hash_to_height,
        })
    }

    pub fn from_blocks(blocks: Vec<ChronoBlock>) -> Self {
        let mut map = BTreeMap::new();
        let mut hash_to_height = BTreeMap::new();
        let chain_id = blocks
            .first()
            .map(|b| b.chain_id.clone())
            .unwrap_or_else(|| "local-file".to_string());

        for block in blocks {
            let hash_hex = hex::encode(&block.block_hash);
            hash_to_height.insert(hash_hex, block.height);
            map.insert(block.height, block);
        }

        Self {
            chain_id,
            display_name: "Local File (in-memory)".to_string(),
            blocks: map,
            block_hash_to_height: hash_to_height,
        }
    }
}

#[async_trait]
impl ChainAdapter for LocalFileAdapter {
    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn block_model(&self) -> BlockModel {
        BlockModel::EventLedger
    }

    async fn latest_height(&self) -> Result<u64> {
        self.blocks
            .keys()
            .max()
            .copied()
            .ok_or_else(|| chrononode_core::CoreError::Adapter("no blocks available".to_string()))
    }

    async fn fetch_block(&self, height: u64) -> Result<ChronoBlock> {
        self.blocks
            .get(&height)
            .cloned()
            .ok_or_else(|| chrononode_core::CoreError::NotFound(format!("block {}", height)))
    }

    async fn fetch_block_by_hash(&self, hash: &[u8]) -> Result<ChronoBlock> {
        let hash_hex = hex::encode(hash);
        let height = self.block_hash_to_height.get(&hash_hex).ok_or_else(|| {
            chrononode_core::CoreError::NotFound(format!("block with hash {}", hash_hex))
        })?;
        self.fetch_block(*height).await
    }
}

pub fn init() {
    chrononode_adapter_sdk::registry::register("local-file", "Local File Adapter", |config| {
        let path = config
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("./blocks");
        let adapter = LocalFileAdapter::new(path).map_err(|e| e.to_string())?;
        Ok(Arc::new(adapter) as Arc<dyn ChainAdapter>)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrononode_core::ChronoTx;
    use sha2::{Digest, Sha256};

    fn make_test_block(height: u64) -> ChronoBlock {
        let mut h = Sha256::default();
        h.update(b"test-block");
        h.update(height.to_be_bytes());
        let block_hash = h.finalize().to_vec();

        ChronoBlock {
            schema_version: 1,
            chain_id: "test-chain".to_string(),
            height,
            block_hash,
            prev_hash: vec![0u8; 32],
            timestamp: 1700000000 + height,
            block_model: "EventLedger".to_string(),
            hash_algorithm: "sha256".to_string(),
            transactions: vec![ChronoTx {
                tx_hash: vec![height as u8; 32],
                sender: vec![0x01; 32],
                recipient: vec![0x02; 32],
                amount: 1000 + height,
                nonce: height,
                payload: vec![],
                gas_limit: 21000,
                gas_used: 21000,
                extra_data: vec![],
            }],
            events: vec![],
            extra_data: vec![],
        }
    }

    #[test]
    fn test_local_file_adapter_from_blocks() {
        let blocks = vec![make_test_block(0), make_test_block(1), make_test_block(2)];
        let adapter = LocalFileAdapter::from_blocks(blocks);

        assert_eq!(adapter.chain_id(), "test-chain");
        assert_eq!(adapter.block_model(), BlockModel::EventLedger);
    }

    #[tokio::test]
    async fn test_local_file_adapter_fetch_by_height() {
        let blocks = vec![make_test_block(0), make_test_block(1), make_test_block(2)];
        let adapter = LocalFileAdapter::from_blocks(blocks);

        let block = adapter.fetch_block(1).await.unwrap();
        assert_eq!(block.height, 1);
        assert_eq!(block.chain_id, "test-chain");
    }

    #[tokio::test]
    async fn test_local_file_adapter_fetch_by_hash() {
        let blocks = vec![make_test_block(0), make_test_block(1), make_test_block(2)];
        let adapter = LocalFileAdapter::from_blocks(blocks);

        let block = adapter.fetch_block(2).await.unwrap();
        let fetched = adapter
            .fetch_block_by_hash(&block.block_hash)
            .await
            .unwrap();
        assert_eq!(fetched.height, 2);
    }

    #[tokio::test]
    async fn test_local_file_adapter_latest_height() {
        let blocks = vec![make_test_block(0), make_test_block(5), make_test_block(3)];
        let adapter = LocalFileAdapter::from_blocks(blocks);

        let latest = adapter.latest_height().await.unwrap();
        assert_eq!(latest, 5);
    }

    #[tokio::test]
    async fn test_local_file_adapter_not_found() {
        let blocks = vec![make_test_block(0)];
        let adapter = LocalFileAdapter::from_blocks(blocks);

        let result = adapter.fetch_block(999).await;
        assert!(result.is_err());
    }
}
