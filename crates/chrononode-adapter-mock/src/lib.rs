use async_trait::async_trait;
use chrononode_core::{BlockModel, ChainAdapter, ChronoBlock, ChronoEvent, ChronoTx, Result};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub struct MockAdapter {
    chain_id: String,
    display_name: String,
}

impl MockAdapter {
    pub fn new() -> Self {
        Self {
            chain_id: "mock".to_string(),
            display_name: "Mock Chain".to_string(),
        }
    }

    pub fn generate_block(&self, height: u64) -> ChronoBlock {
        let block_hash = {
            let mut h = Sha256::default();
            h.update(b"mock-block");
            h.update(height.to_be_bytes());
            h.finalize().to_vec()
        };
        let prev_hash = if height == 0 {
            vec![0u8; 32]
        } else {
            let mut h = Sha256::default();
            h.update(b"mock-block");
            h.update((height - 1).to_be_bytes());
            h.finalize().to_vec()
        };
        ChronoBlock {
            schema_version: 1,
            chain_id: self.chain_id.clone(),
            height,
            block_hash,
            prev_hash,
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
            events: vec![ChronoEvent {
                event_type: "BlockProduced".to_string(),
                emitter: vec![0xaa; 32],
                tx_index: 0,
                event_index: 0,
                payload: format!("{{\"height\":{}}}", height).into_bytes(),
            }],
            extra_data: vec![],
        }
    }
}

impl Default for MockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChainAdapter for MockAdapter {
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
        Ok(9999)
    }

    async fn fetch_block(&self, height: u64) -> Result<ChronoBlock> {
        Ok(self.generate_block(height))
    }

    async fn fetch_block_by_hash(&self, hash: &[u8]) -> Result<ChronoBlock> {
        let height = u64::from_be_bytes(hash[24..32].try_into().unwrap_or([0; 8]));
        Ok(self.generate_block(height))
    }
}

pub fn init() {
    chrononode_adapter_sdk::registry::register("mock", "Mock Chain", |_config| {
        Ok(Arc::new(MockAdapter::new()))
    });
}
