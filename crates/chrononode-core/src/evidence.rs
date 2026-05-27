use crate::chain::{ChainAdapter, ChainEvidenceAdapter, TransferEvidence, AddressActivity, DormancyEvidenceRequest, DormancyEvidence};
use crate::dormancy::EvidenceSourceType;
use crate::Result;
use async_trait::async_trait;
use sha2::Digest;
use std::sync::Arc;

#[async_trait]
pub trait TxLookupBackend: Send + Sync {
    async fn get_transaction_by_hash(
        &self,
        chain_id: &str,
        tx_hash_hex: &str,
    ) -> Result<Option<serde_json::Value>>;
}

pub struct DefaultChainEvidenceAdapter {
    pub chain_id: String,
    pub source_type: EvidenceSourceType,
    pub adapter: Arc<dyn ChainAdapter>,
    pub tx_lookup: Arc<dyn TxLookupBackend>,
}

impl DefaultChainEvidenceAdapter {
    pub fn new(
        chain_id: String,
        source_type: EvidenceSourceType,
        adapter: Arc<dyn ChainAdapter>,
        tx_lookup: Arc<dyn TxLookupBackend>,
    ) -> Self {
        Self {
            chain_id,
            source_type,
            adapter,
            tx_lookup,
        }
    }
}

fn compare_addresses(db_addr_hex: &str, expected: &str) -> bool {
    let db_addr_hex_lower = db_addr_hex.to_lowercase();
    let expected_lower = expected.to_lowercase();

    // 1. Direct hex comparison (if expected is a hex string)
    let expected_hex = expected_lower.trim_start_matches("0x").to_string();
    if db_addr_hex_lower == expected_hex {
        return true;
    }

    // 2. String bytes representation hex comparison
    let expected_as_hex = hex::encode(expected.as_bytes());
    if db_addr_hex_lower == expected_as_hex {
        return true;
    }

    // 3. Try to decode db_addr_hex to bytes, and see if it matches expected string as utf8
    if let Ok(bytes) = hex::decode(db_addr_hex) {
        if let Ok(string_val) = String::from_utf8(bytes) {
            if string_val.to_lowercase() == expected_lower {
                return true;
            }
        }
    }

    false
}

#[async_trait]
impl ChainEvidenceAdapter for DefaultChainEvidenceAdapter {
    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn source_type(&self) -> EvidenceSourceType {
        self.source_type
    }

    async fn latest_height(&self) -> Result<u64> {
        self.adapter.latest_height().await
    }

    async fn verify_transfer_claim(
        &self,
        tx_hash: &str,
        expected_from: Option<&str>,
        expected_to: &str,
        min_amount: Option<u128>,
    ) -> Result<TransferEvidence> {
        let clean_tx_hash = tx_hash.trim_start_matches("0x");
        let tx_opt = self
            .tx_lookup
            .get_transaction_by_hash(&self.chain_id, clean_tx_hash)
            .await?;

        let tx_val = match tx_opt {
            Some(v) => v,
            None => {
                return Ok(TransferEvidence {
                    tx_hash: tx_hash.to_string(),
                    from_address: String::new(),
                    to_address: expected_to.to_string(),
                    amount: 0,
                    block_height: 0,
                    verified: false,
                });
            }
        };

        let from_address_hex = tx_val.get("sender").and_then(|v| v.as_str()).unwrap_or("");
        let to_address_hex = tx_val.get("recipient").and_then(|v| v.as_str()).unwrap_or("");

        let amount: u128 = match tx_val.get("amount") {
            Some(serde_json::Value::String(s)) => s.parse().unwrap_or(0),
            Some(serde_json::Value::Number(n)) => n.as_u64().map(|v| v as u128).unwrap_or(0),
            _ => 0,
        };

        let block_height = tx_val.get("block_height").and_then(|v| v.as_u64()).unwrap_or(0);

        // Decode from_address_hex back to string for the return struct if it's utf8
        let from_address = if let Ok(bytes) = hex::decode(from_address_hex) {
            String::from_utf8(bytes).unwrap_or_else(|_| format!("0x{}", from_address_hex))
        } else {
            format!("0x{}", from_address_hex)
        };

        let to_address = if let Ok(bytes) = hex::decode(to_address_hex) {
            String::from_utf8(bytes).unwrap_or_else(|_| format!("0x{}", to_address_hex))
        } else {
            format!("0x{}", to_address_hex)
        };

        let mut verified = true;

        if let Some(expected_from_addr) = expected_from {
            if !compare_addresses(from_address_hex, expected_from_addr) {
                verified = false;
            }
        }

        if !compare_addresses(to_address_hex, expected_to) {
            verified = false;
        }

        if let Some(min_amt) = min_amount {
            if amount < min_amt {
                verified = false;
            }
        }

        Ok(TransferEvidence {
            tx_hash: tx_hash.to_string(),
            from_address,
            to_address,
            amount,
            block_height,
            verified,
        })
    }

    async fn get_address_activity(
        &self,
        address: &str,
    ) -> Result<AddressActivity> {
        let current_height = self.adapter.latest_height().await?;
        let address_hex = hex::encode(address.as_bytes());

        let tx_opt = self
            .tx_lookup
            .get_transaction_by_hash(&self.chain_id, &address_hex)
            .await?;

        let (last_seen_tx, last_seen_block, last_seen_timestamp) = match tx_opt {
            Some(tx) => (
                tx.get("tx_hash").and_then(|v| v.as_str()).map(|s| s.to_string()),
                tx.get("block_height").and_then(|v| v.as_u64()),
                tx.get("timestamp").and_then(|v| v.as_u64()),
            ),
            None => (None, None, None),
        };

        let dormancy_blocks = match last_seen_block {
            Some(b) => current_height.saturating_sub(b),
            None => 0,
        };

        Ok(AddressActivity {
            address: address.to_string(),
            chain_id: self.chain_id.clone(),
            last_seen_tx,
            last_seen_block,
            last_seen_timestamp,
            current_height,
            is_dormant: dormancy_blocks > 0,
            dormancy_blocks,
        })
    }

    async fn verify_ownership_signature(
        &self,
        address: &str,
        message: &str,
        signature: &str,
    ) -> Result<bool> {
        let sig_bytes = hex::decode(signature.trim_start_matches("0x"))
            .map_err(|e| crate::CoreError::Adapter(format!("invalid signature hex: {}", e)))?;

        if sig_bytes.len() != 64 {
            return Ok(false);
        }

        let msg_bytes = message.as_bytes();
        let pubkey_opt = self
            .tx_lookup
            .get_transaction_by_hash(&self.chain_id, &hex::encode(address.as_bytes()))
            .await?;

        let expected_pubkey = match pubkey_opt {
            Some(tx) => tx.get("pubkey").and_then(|v| v.as_str()).map(|s| s.to_string()),
            None => None,
        };

        let pubkey_bytes = match expected_pubkey {
            Some(pk) => hex::decode(pk.trim_start_matches("0x"))
                .map_err(|e| crate::CoreError::Adapter(format!("invalid pubkey hex: {}", e)))?,
            None => return Ok(false),
        };

        if pubkey_bytes.len() != 32 {
            return Ok(false);
        }

        let mut pk_arr = [0u8; 32];
        pk_arr.copy_from_slice(&pubkey_bytes);

        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);

        Ok(crate::signing::verify_signature(&pk_arr, &sig_arr, msg_bytes))
    }

    async fn build_dormancy_evidence(
        &self,
        request: DormancyEvidenceRequest,
    ) -> Result<DormancyEvidence> {
        let current_height = match request.current_height {
            Some(h) => h,
            None => self.adapter.latest_height().await?,
        };

        let activity = self.get_address_activity(&request.address).await?;

        let source_address_hash = format!(
            "0x{}",
            hex::encode(sha2::Sha256::digest(request.address.as_bytes()))
        );

        let dormancy_seconds = activity
            .last_seen_timestamp
            .map(|ts| std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs().saturating_sub(ts))
                .unwrap_or(0))
            .unwrap_or(0);

        let confidence_tier = match self.source_type {
            EvidenceSourceType::FullNode => 3,
            EvidenceSourceType::PrunedNode => 3,
            EvidenceSourceType::LightClient => 3,
            EvidenceSourceType::PublicRpc => 4,
            EvidenceSourceType::OfficialExplorerApi => 5,
            EvidenceSourceType::ThirdPartyExplorerApi => 6,
            EvidenceSourceType::MultiSource => 2,
            EvidenceSourceType::ZkVmProof => 2,
            EvidenceSourceType::ManualReview => 7,
        };

        let confidence_score = match confidence_tier {
            1 => 100,
            2 => 90,
            3 => 80,
            4 => 60,
            5 => 40,
            6 => 30,
            _ => 0,
        };

        let evidence_hash = format!(
            "0x{}",
            hex::encode(sha2::Sha256::digest(
                format!("{}:{}:{}:{}", self.chain_id, request.address, current_height, dormancy_seconds).as_bytes()
            ))
        );

        Ok(DormancyEvidence {
            version: "chrononode:evidence:v1".to_string(),
            chain_id: self.chain_id.clone(),
            source_type: self.source_type,
            source_count: 1,
            source_address_hash,
            evm_wallet: request.evm_wallet.unwrap_or_default(),
            last_seen_tx: activity.last_seen_tx,
            last_seen_block: activity.last_seen_block,
            last_seen_timestamp: activity.last_seen_timestamp,
            current_height,
            checked_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            dormancy_seconds,
            confidence_tier,
            confidence_score,
            evidence_hash,
            raw_evidence_pointer: None,
            zk_proof: None,
            public_inputs: None,
            attester_pubkey: String::new(),
            attester_signature: String::new(),
        })
    }

    async fn verify_evidence(
        &self,
        evidence: &DormancyEvidence,
    ) -> Result<bool> {
        if evidence.chain_id != self.chain_id {
            return Ok(false);
        }

        if evidence.dormancy_seconds == 0 {
            return Ok(false);
        }

        let expected_hash = format!(
            "0x{}",
            hex::encode(sha2::Sha256::digest(
                format!("{}:{}:{}:{}", evidence.chain_id, evidence.source_address_hash, evidence.current_height, evidence.dormancy_seconds).as_bytes()
            ))
        );

        if evidence.evidence_hash != expected_hash {
            return Ok(false);
        }

        if !evidence.attester_pubkey.is_empty() && !evidence.attester_signature.is_empty() {
            let sig_bytes = match hex::decode(evidence.attester_signature.trim_start_matches("0x")) {
                Ok(b) => b,
                Err(_) => return Ok(false),
            };
            let pubkey_bytes = match hex::decode(evidence.attester_pubkey.trim_start_matches("0x")) {
                Ok(b) => b,
                Err(_) => return Ok(false),
            };

            if sig_bytes.len() != 64 || pubkey_bytes.len() != 32 {
                return Ok(false);
            }

            let mut pk_arr = [0u8; 32];
            pk_arr.copy_from_slice(&pubkey_bytes);
            let mut sig_arr = [0u8; 64];
            sig_arr.copy_from_slice(&sig_bytes);

            let message = format!(
                "{}:{}:{}:{}",
                evidence.chain_id, evidence.source_address_hash, evidence.current_height, evidence.dormancy_seconds
            );

            if !crate::signing::verify_signature(&pk_arr, &sig_arr, message.as_bytes()) {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::BlockModel;
    use crate::block::ChronoBlock;

    struct MockChainAdapter {
        chain_id: String,
    }

    #[async_trait]
    impl ChainAdapter for MockChainAdapter {
        fn chain_id(&self) -> &str {
            &self.chain_id
        }
        fn display_name(&self) -> &str {
            "Mock"
        }
        fn block_model(&self) -> BlockModel {
            BlockModel::Account
        }
        async fn latest_height(&self) -> Result<u64> {
            Ok(100)
        }
        async fn fetch_block(&self, _height: u64) -> Result<ChronoBlock> {
            unimplemented!()
        }
        async fn fetch_block_by_hash(&self, _hash: &[u8]) -> Result<ChronoBlock> {
            unimplemented!()
        }
    }

    struct MockTxLookup {
        txs: std::collections::HashMap<String, serde_json::Value>,
    }

    #[async_trait]
    impl TxLookupBackend for MockTxLookup {
        async fn get_transaction_by_hash(
            &self,
            _chain_id: &str,
            tx_hash_hex: &str,
        ) -> Result<Option<serde_json::Value>> {
            Ok(self.txs.get(tx_hash_hex).cloned())
        }
    }

    #[tokio::test]
    async fn test_verify_transfer_claim() {
        let chain_id = "bitcoin".to_string();
        let adapter = Arc::new(MockChainAdapter { chain_id: chain_id.clone() });

        let mut txs = std::collections::HashMap::new();
        let recipient_str = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let recipient_hex = hex::encode(recipient_str.as_bytes());
        let sender_str = "1BiVoe3zW7vdn";
        let sender_hex = hex::encode(sender_str.as_bytes());

        txs.insert(
            "tx123".to_string(),
            serde_json::json!({
                "sender": sender_hex,
                "recipient": recipient_hex,
                "amount": "500000000",
                "block_height": 50u64,
            }),
        );

        let tx_lookup = Arc::new(MockTxLookup { txs });
        let evidence_adapter = DefaultChainEvidenceAdapter::new(
            chain_id,
            EvidenceSourceType::PublicRpc,
            adapter,
            tx_lookup,
        );

        // Test successful verification
        let res = evidence_adapter
            .verify_transfer_claim("tx123", Some(sender_str), recipient_str, Some(100000000))
            .await
            .unwrap();
        assert!(res.verified);
        assert_eq!(res.amount, 500000000);
        assert_eq!(res.from_address, sender_str);
        assert_eq!(res.to_address, recipient_str);

        // Test mismatched sender
        let res_wrong_sender = evidence_adapter
            .verify_transfer_claim("tx123", Some("wrong_sender"), recipient_str, Some(100000000))
            .await
            .unwrap();
        assert!(!res_wrong_sender.verified);

        // Test mismatched recipient
        let res_wrong_recipient = evidence_adapter
            .verify_transfer_claim("tx123", Some(sender_str), "wrong_recipient", Some(100000000))
            .await
            .unwrap();
        assert!(!res_wrong_recipient.verified);

        // Test insufficient amount
        let res_low_amount = evidence_adapter
            .verify_transfer_claim("tx123", Some(sender_str), recipient_str, Some(1000000000))
            .await
            .unwrap();
        assert!(!res_low_amount.verified);
    }
}
