use crate::block::ChronoBlock;
use crate::Result;
use async_trait::async_trait;
use std::fmt;

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
        Self {
            backend: backend.into(),
            key: key.into(),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        let colon = s.find(':')?;
        Some(Self {
            backend: s[..colon].to_string(),
            key: s[colon + 1..].to_string(),
        })
    }
}

impl fmt::Display for StoragePointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.backend, self.key)
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

    async fn delete(&self, _pointer: &StoragePointer) -> Result<()> {
        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealth>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransferEvidence {
    pub tx_hash: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: u128,
    pub block_height: u64,
    pub verified: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddressActivity {
    pub address: String,
    pub chain_id: String,
    pub last_seen_tx: Option<String>,
    pub last_seen_block: Option<u64>,
    pub last_seen_timestamp: Option<u64>,
    pub current_height: u64,
    pub is_dormant: bool,
    pub dormancy_blocks: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DormancyEvidenceRequest {
    pub chain_id: String,
    pub address: String,
    pub evm_wallet: Option<String>,
    pub dormancy_threshold_blocks: u64,
    pub current_height: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DormancyEvidence {
    pub version: String,
    pub chain_id: String,
    pub source_type: crate::dormancy::EvidenceSourceType,
    pub source_count: u8,
    pub source_address_hash: String,
    pub evm_wallet: String,
    pub last_seen_tx: Option<String>,
    pub last_seen_block: Option<u64>,
    pub last_seen_timestamp: Option<u64>,
    pub current_height: u64,
    pub checked_at: u64,
    pub dormancy_seconds: u64,
    pub confidence_tier: u8,
    pub confidence_score: u8,
    pub evidence_hash: String,
    pub raw_evidence_pointer: Option<String>,
    pub zk_proof: Option<String>,
    pub public_inputs: Option<String>,
    pub attester_pubkey: String,
    pub attester_signature: String,
}

#[async_trait]
pub trait ChainEvidenceAdapter: Send + Sync {
    fn chain_id(&self) -> &str;
    fn source_type(&self) -> crate::dormancy::EvidenceSourceType;
    async fn latest_height(&self) -> Result<u64>;

    async fn get_address_activity(
        &self,
        address: &str,
    ) -> Result<AddressActivity>;

    async fn verify_ownership_signature(
        &self,
        address: &str,
        message: &str,
        signature: &str,
    ) -> Result<bool>;

    async fn verify_transfer_claim(
        &self,
        tx_hash: &str,
        expected_from: Option<&str>,
        expected_to: &str,
        min_amount: Option<u128>,
    ) -> Result<TransferEvidence>;

    async fn build_dormancy_evidence(
        &self,
        request: DormancyEvidenceRequest,
    ) -> Result<DormancyEvidence>;

    async fn verify_evidence(
        &self,
        evidence: &DormancyEvidence,
    ) -> Result<bool>;
}
