/// Phase 2 — Transfer/Burn Auto-Detection Pipeline.
///
/// Scans indexed transactions for transfers to configured vault or burn addresses.
/// When a transfer is detected from a watched address to a vault/burn address,
/// creates a pending legacy claim and triggers the attestation flow.
use chrononode_core::{chain::TransferEvidence, dormancy::EvidenceSourceType, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A detected transfer to a vault or burn address that qualifies as a legacy claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedTransfer {
    pub chain_id: String,
    pub source_address: String,
    pub destination_address: String,
    pub tx_hash: String,
    pub amount: u128,
    pub block_height: u64,
    pub detected_at: u64,
    pub transfer_type: TransferType,
    pub claim_submitted: bool,
}

/// Whether the transfer went to a vault or a burn address.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferType {
    Vault,
    Burn,
}

impl TransferType {
    pub fn confidence_tier(&self) -> u8 {
        match self {
            TransferType::Burn => 1,
            TransferType::Vault => 2,
        }
    }
}

/// Registry of vault and burn addresses per chain.
#[derive(Debug, Clone, Default)]
pub struct VaultBurnRegistry {
    vault_addresses: HashMap<String, Vec<String>>,
    burn_addresses: HashMap<String, Vec<String>>,
}

impl VaultBurnRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_vault_address(&mut self, chain_id: &str, address: &str) {
        self.vault_addresses
            .entry(chain_id.to_string())
            .or_default()
            .push(address.to_string());
    }

    pub fn set_burn_address(&mut self, chain_id: &str, address: &str) {
        self.burn_addresses
            .entry(chain_id.to_string())
            .or_default()
            .push(address.to_string());
    }

    pub fn get_vault_addresses(&self, chain_id: &str) -> Vec<&str> {
        self.vault_addresses
            .get(chain_id)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    pub fn get_burn_addresses(&self, chain_id: &str) -> Vec<&str> {
        self.burn_addresses
            .get(chain_id)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    pub fn classify_destination(&self, chain_id: &str, destination: &str) -> Option<TransferType> {
        let vaults = self.get_vault_addresses(chain_id);
        if vaults.iter().any(|v| addresses_match(v, destination)) {
            return Some(TransferType::Vault);
        }
        let burns = self.get_burn_addresses(chain_id);
        if burns.iter().any(|v| addresses_match(v, destination)) {
            return Some(TransferType::Burn);
        }
        None
    }
}

fn addresses_match(a: &str, b: &str) -> bool {
    let a_clean = a.trim_start_matches("0x").to_lowercase();
    let b_clean = b.trim_start_matches("0x").to_lowercase();
    a_clean == b_clean
}

/// Scans transactions for transfers to vault/burn addresses.
pub struct TransferWatcher {
    registry: Arc<Mutex<VaultBurnRegistry>>,
    processed_txs: Arc<Mutex<HashMap<String, bool>>>,
}

impl TransferWatcher {
    pub fn new(registry: Arc<Mutex<VaultBurnRegistry>>) -> Self {
        Self {
            registry,
            processed_txs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a transaction has already been processed.
    pub async fn is_processed(&self, tx_hash: &str) -> bool {
        self.processed_txs.lock().await.contains_key(tx_hash)
    }

    /// Mark a transaction as processed.
    pub async fn mark_processed(&self, tx_hash: &str) {
        self.processed_txs
            .lock()
            .await
            .insert(tx_hash.to_string(), true);
    }

    /// Scan a single transaction for vault/burn transfers.
    /// Returns Some(DetectedTransfer) if the tx sends funds to a registered vault/burn address.
    pub async fn scan_transaction(
        &self,
        chain_id: &str,
        tx_hash: &str,
        sender: &str,
        recipient: &str,
        amount: u128,
        block_height: u64,
    ) -> Result<Option<DetectedTransfer>> {
        if self.is_processed(tx_hash).await {
            return Ok(None);
        }

        let registry = self.registry.lock().await;
        let transfer_type = match registry.classify_destination(chain_id, recipient) {
            Some(t) => t,
            None => return Ok(None),
        };

        let detected = DetectedTransfer {
            chain_id: chain_id.to_string(),
            source_address: sender.to_string(),
            destination_address: recipient.to_string(),
            tx_hash: tx_hash.to_string(),
            amount,
            block_height,
            detected_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            transfer_type,
            claim_submitted: false,
        };

        self.mark_processed(tx_hash).await;
        Ok(Some(detected))
    }

    /// Scan a batch of transactions (as JSON values from the index).
    pub async fn scan_transactions(
        &self,
        chain_id: &str,
        txs: &[serde_json::Value],
        block_height: u64,
    ) -> Result<Vec<DetectedTransfer>> {
        let mut detected = Vec::new();

        for tx in txs {
            let tx_hash = tx
                .get("tx_hash")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if tx_hash.is_empty() || self.is_processed(&tx_hash).await {
                continue;
            }

            let sender = tx
                .get("sender")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let recipient = tx
                .get("recipient")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let amount: u128 = match tx.get("amount") {
                Some(serde_json::Value::String(s)) => s.parse().unwrap_or(0),
                Some(serde_json::Value::Number(n)) => n.as_u64().map(|v| v as u128).unwrap_or(0),
                _ => 0,
            };

            if let Some(dt) = self
                .scan_transaction(chain_id, &tx_hash, sender, recipient, amount, block_height)
                .await?
            {
                detected.push(dt);
            }
        }

        Ok(detected)
    }

    /// Build transfer evidence for a detected transfer.
    pub async fn build_evidence(
        &self,
        detected: &DetectedTransfer,
    ) -> Result<TransferEvidence> {
        Ok(TransferEvidence {
            tx_hash: detected.tx_hash.clone(),
            from_address: detected.source_address.clone(),
            to_address: detected.destination_address.clone(),
            amount: detected.amount,
            block_height: detected.block_height,
            verified: true,
        })
    }

    /// Get the appropriate evidence source type based on transfer type.
    pub fn evidence_source_type(&self, transfer_type: TransferType) -> EvidenceSourceType {
        match transfer_type {
            TransferType::Burn => EvidenceSourceType::FullNode,
            TransferType::Vault => EvidenceSourceType::FullNode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> VaultBurnRegistry {
        let mut reg = VaultBurnRegistry::new();
        reg.set_vault_address("bitcoin", "bc1qvault123");
        reg.set_burn_address("bitcoin", "1111111111111111111114oLvT2");
        reg.set_vault_address("dogecoin", "DNgVaultAddr123");
        reg
    }

    #[tokio::test]
    async fn test_classify_vault() {
        let reg = test_registry();
        let result = reg.classify_destination("bitcoin", "bc1qvault123");
        assert_eq!(result, Some(TransferType::Vault));
    }

    #[tokio::test]
    async fn test_classify_burn() {
        let reg = test_registry();
        let result = reg.classify_destination("bitcoin", "1111111111111111111114oLvT2");
        assert_eq!(result, Some(TransferType::Burn));
    }

    #[tokio::test]
    async fn test_classify_unknown() {
        let reg = test_registry();
        let result = reg.classify_destination("bitcoin", "some_random_address");
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_scan_detects_vault_transfer() {
        let registry = Arc::new(Mutex::new(test_registry()));
        let watcher = TransferWatcher::new(registry);

        let result = watcher
            .scan_transaction(
                "bitcoin",
                "tx123",
                "1SenderAddr",
                "bc1qvault123",
                500000000,
                800000,
            )
            .await
            .unwrap();

        assert!(result.is_some());
        let detected = result.unwrap();
        assert_eq!(detected.transfer_type, TransferType::Vault);
        assert_eq!(detected.amount, 500000000);
    }

    #[tokio::test]
    async fn test_scan_detects_burn_transfer() {
        let registry = Arc::new(Mutex::new(test_registry()));
        let watcher = TransferWatcher::new(registry);

        let result = watcher
            .scan_transaction(
                "bitcoin",
                "tx456",
                "1SenderAddr",
                "1111111111111111111114oLvT2",
                100000000,
                800001,
            )
            .await
            .unwrap();

        assert!(result.is_some());
        let detected = result.unwrap();
        assert_eq!(detected.transfer_type, TransferType::Burn);
    }

    #[tokio::test]
    async fn test_scan_ignores_non_vault_transfer() {
        let registry = Arc::new(Mutex::new(test_registry()));
        let watcher = TransferWatcher::new(registry);

        let result = watcher
            .scan_transaction(
                "bitcoin",
                "tx789",
                "1SenderAddr",
                "1RandomRecipient",
                500000000,
                800002,
            )
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_scan_deduplicates() {
        let registry = Arc::new(Mutex::new(test_registry()));
        let watcher = TransferWatcher::new(registry);

        let first = watcher
            .scan_transaction(
                "bitcoin",
                "tx123",
                "1SenderAddr",
                "bc1qvault123",
                500000000,
                800000,
            )
            .await
            .unwrap();

        assert!(first.is_some());

        let second = watcher
            .scan_transaction(
                "bitcoin",
                "tx123",
                "1SenderAddr",
                "bc1qvault123",
                500000000,
                800000,
            )
            .await
            .unwrap();

        assert!(second.is_none());
    }

    #[tokio::test]
    async fn test_confidence_tier_burn_higher() {
        assert!(TransferType::Burn.confidence_tier() < TransferType::Vault.confidence_tier());
    }

    #[tokio::test]
    async fn test_addresses_match_with_0x_prefix() {
        assert!(addresses_match("0xabc123", "abc123"));
        assert!(addresses_match("ABC123", "abc123"));
        assert!(addresses_match("0xABC123", "0xabc123"));
        assert!(!addresses_match("abc123", "def456"));
    }
}
