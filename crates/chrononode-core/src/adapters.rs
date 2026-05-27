/// Phase 3 — Multi-Source Evidence Adapters.
///
/// Provides alternative evidence sources when full nodes are unavailable:
/// - PublicRpcAdapter: Queries public RPC endpoints
/// - OfficialExplorerAdapter: Queries official block explorer APIs
/// - MultiSourceAdapter: Aggregates and cross-checks multiple sources
use crate::chain::{
    AddressActivity, ChainEvidenceAdapter, DormancyEvidence, DormancyEvidenceRequest,
    TransferEvidence,
};
use crate::dormancy::EvidenceSourceType;
use crate::{CoreError, Result};
use async_trait::async_trait;
use sha2::Digest;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Configuration for an RPC endpoint.
#[derive(Debug, Clone)]
pub struct RpcEndpoint {
    pub url: String,
    pub chain_id: String,
    pub rate_limit_per_minute: u32,
}

/// Adapter that queries public RPC endpoints for evidence.
pub struct PublicRpcAdapter {
    endpoint: RpcEndpoint,
    client: reqwest::Client,
}

impl PublicRpcAdapter {
    pub fn new(endpoint: RpcEndpoint) -> Self {
        Self {
            endpoint,
            client: reqwest::Client::builder()
                .user_agent("chrononode/0.1")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    async fn rpc_call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1,
        });

        let resp = self
            .client
            .post(&self.endpoint.url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Adapter(format!("RPC request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(CoreError::Adapter(format!(
                "RPC returned status {}",
                resp.status()
            )));
        }

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Adapter(format!("RPC response parse failed: {}", e)))?;

        if let Some(err) = result.get("error") {
            return Err(CoreError::Adapter(format!("RPC error: {}", err)));
        }

        Ok(result["result"].clone())
    }
}

#[async_trait]
impl ChainEvidenceAdapter for PublicRpcAdapter {
    fn chain_id(&self) -> &str {
        &self.endpoint.chain_id
    }

    fn source_type(&self) -> EvidenceSourceType {
        EvidenceSourceType::PublicRpc
    }

    async fn latest_height(&self) -> Result<u64> {
        let result = self.rpc_call("eth_blockNumber", serde_json::json!([])).await?;
        let hex_str = result
            .as_str()
            .ok_or_else(|| CoreError::Adapter("blockNumber not a string".into()))?;
        u64::from_str_radix(hex_str.trim_start_matches("0x"), 16)
            .map_err(|e| CoreError::Adapter(format!("parse block number: {}", e)))
    }

    async fn get_address_activity(&self, address: &str) -> Result<AddressActivity> {
        let current_height = self.latest_height().await?;

        let tx_count = self
            .rpc_call(
                "eth_getTransactionCount",
                serde_json::json!([address, "latest"]),
            )
            .await?;

        let nonce = tx_count
            .as_str()
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0);

        Ok(AddressActivity {
            address: address.to_string(),
            chain_id: self.chain_id().to_string(),
            last_seen_tx: None,
            last_seen_block: None,
            last_seen_timestamp: None,
            current_height,
            is_dormant: nonce == 0,
            dormancy_blocks: if nonce == 0 { current_height } else { 0 },
        })
    }

    async fn verify_ownership_signature(
        &self,
        _address: &str,
        _message: &str,
        _signature: &str,
    ) -> Result<bool> {
        Ok(false)
    }

    async fn verify_transfer_claim(
        &self,
        tx_hash: &str,
        expected_from: Option<&str>,
        expected_to: &str,
        min_amount: Option<u128>,
    ) -> Result<TransferEvidence> {
        let tx = self
            .rpc_call(
                "eth_getTransactionByHash",
                serde_json::json!([tx_hash]),
            )
            .await?;

        if tx.is_null() {
            return Ok(TransferEvidence {
                tx_hash: tx_hash.to_string(),
                from_address: String::new(),
                to_address: expected_to.to_string(),
                amount: 0,
                block_height: 0,
                verified: false,
            });
        }

        let from = tx.get("from").and_then(|v| v.as_str()).unwrap_or("");
        let to = tx.get("to").and_then(|v| v.as_str()).unwrap_or("");
        let value_hex = tx.get("value").and_then(|v| v.as_str()).unwrap_or("0x0");
        let block_hex = tx
            .get("blockNumber")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0");

        let amount = u128::from_str_radix(value_hex.trim_start_matches("0x"), 16).unwrap_or(0);
        let block_height = u64::from_str_radix(block_hex.trim_start_matches("0x"), 16).unwrap_or(0);

        let mut verified = true;
        if let Some(expected_from) = expected_from {
            if from.to_lowercase() != expected_from.to_lowercase() {
                verified = false;
            }
        }
        if to.to_lowercase() != expected_to.to_lowercase() {
            verified = false;
        }
        if let Some(min_amt) = min_amount {
            if amount < min_amt {
                verified = false;
            }
        }

        Ok(TransferEvidence {
            tx_hash: tx_hash.to_string(),
            from_address: from.to_string(),
            to_address: to.to_string(),
            amount,
            block_height,
            verified,
        })
    }

    async fn build_dormancy_evidence(
        &self,
        request: DormancyEvidenceRequest,
    ) -> Result<DormancyEvidence> {
        let current_height = match request.current_height {
            Some(h) => h,
            None => self.latest_height().await?,
        };

        let activity = self.get_address_activity(&request.address).await?;

        let source_address_hash = format!(
            "0x{}",
            hex::encode(sha2::Sha256::digest(request.address.as_bytes()))
        );

        let evidence_hash = format!(
            "0x{}",
            hex::encode(sha2::Sha256::digest(
                format!(
                    "{}:{}:{}:{}",
                    self.chain_id(),
                    request.address,
                    current_height,
                    activity.dormancy_blocks
                )
                .as_bytes()
            ))
        );

        Ok(DormancyEvidence {
            version: "chrononode:evidence:v1".to_string(),
            chain_id: self.chain_id().to_string(),
            source_type: EvidenceSourceType::PublicRpc,
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
            dormancy_seconds: 0,
            confidence_tier: 4,
            confidence_score: 60,
            evidence_hash,
            raw_evidence_pointer: None,
            zk_proof: None,
            public_inputs: None,
            attester_pubkey: String::new(),
            attester_signature: String::new(),
        })
    }

    async fn verify_evidence(&self, evidence: &DormancyEvidence) -> Result<bool> {
        Ok(evidence.chain_id == self.chain_id() && evidence.confidence_tier <= 4)
    }
}

/// Configuration for an explorer API endpoint.
#[derive(Debug, Clone)]
pub struct ExplorerEndpoint {
    pub base_url: String,
    pub api_key: Option<String>,
    pub chain_id: String,
    pub is_official: bool,
}

/// Adapter that queries official block explorer APIs for evidence.
pub struct OfficialExplorerAdapter {
    endpoint: ExplorerEndpoint,
    client: reqwest::Client,
}

impl OfficialExplorerAdapter {
    pub fn new(endpoint: ExplorerEndpoint) -> Self {
        Self {
            endpoint,
            client: reqwest::Client::builder()
                .user_agent("chrononode/0.1")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    async fn api_get(&self, module: &str, action: &str, params: &[(&str, &str)]) -> Result<serde_json::Value> {
        let mut url = format!(
            "{}/api?module={}&action={}",
            self.endpoint.base_url, module, action
        );

        for (key, value) in params {
            url.push_str(&format!("&{}={}", key, value));
        }

        if let Some(api_key) = &self.endpoint.api_key {
            url.push_str(&format!("&apikey={}", api_key));
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CoreError::Adapter(format!("Explorer request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(CoreError::Adapter(format!(
                "Explorer returned status {}",
                resp.status()
            )));
        }

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Adapter(format!("Explorer response parse failed: {}", e)))?;

        if result.get("status").and_then(|v| v.as_str()) == Some("0") {
            return Err(CoreError::Adapter(format!(
                "Explorer API error: {}",
                result.get("message").and_then(|v| v.as_str()).unwrap_or("unknown")
            )));
        }

        Ok(result)
    }
}

#[async_trait]
impl ChainEvidenceAdapter for OfficialExplorerAdapter {
    fn chain_id(&self) -> &str {
        &self.endpoint.chain_id
    }

    fn source_type(&self) -> EvidenceSourceType {
        if self.endpoint.is_official {
            EvidenceSourceType::OfficialExplorerApi
        } else {
            EvidenceSourceType::ThirdPartyExplorerApi
        }
    }

    async fn latest_height(&self) -> Result<u64> {
        let result = self.api_get("proxy", "eth_blockNumber", &[]).await?;
        let hex_str = result
            .get("result")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CoreError::Adapter("blockNumber not found".into()))?;
        u64::from_str_radix(hex_str.trim_start_matches("0x"), 16)
            .map_err(|e| CoreError::Adapter(format!("parse block number: {}", e)))
    }

    async fn get_address_activity(&self, address: &str) -> Result<AddressActivity> {
        let current_height = self.latest_height().await?;

        let tx_list = self
            .api_get(
                "account",
                "txlist",
                &[
                    ("address", address),
                    ("startblock", "0"),
                    ("endblock", "99999999"),
                    ("page", "1"),
                    ("offset", "1"),
                    ("sort", "desc"),
                ],
            )
            .await?;

        let txs = tx_list.get("result").and_then(|v| v.as_array());

        let (last_seen_tx, last_seen_block, last_seen_timestamp) = match txs.and_then(|arr| arr.first()) {
            Some(tx) => (
                tx.get("hash").and_then(|v| v.as_str()).map(|s| s.to_string()),
                tx.get("blockNumber")
                    .and_then(|v| v.as_str())
                    .and_then(|s| u64::from_str_radix(s, 16).ok()),
                tx.get("timeStamp")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u64>().ok()),
            ),
            None => (None, None, None),
        };

        let dormancy_blocks = match last_seen_block {
            Some(b) => current_height.saturating_sub(b),
            None => current_height,
        };

        Ok(AddressActivity {
            address: address.to_string(),
            chain_id: self.chain_id().to_string(),
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
        _address: &str,
        _message: &str,
        _signature: &str,
    ) -> Result<bool> {
        Ok(false)
    }

    async fn verify_transfer_claim(
        &self,
        tx_hash: &str,
        expected_from: Option<&str>,
        expected_to: &str,
        min_amount: Option<u128>,
    ) -> Result<TransferEvidence> {
        let result = self
            .api_get("proxy", "eth_getTransactionByHash", &[("txhash", tx_hash)])
            .await?;

        let tx = result.get("result");
        if tx.is_none() || tx.unwrap().is_null() {
            return Ok(TransferEvidence {
                tx_hash: tx_hash.to_string(),
                from_address: String::new(),
                to_address: expected_to.to_string(),
                amount: 0,
                block_height: 0,
                verified: false,
            });
        }

        let tx = tx.unwrap();
        let from = tx.get("from").and_then(|v| v.as_str()).unwrap_or("");
        let to = tx.get("to").and_then(|v| v.as_str()).unwrap_or("");
        let value_hex = tx.get("value").and_then(|v| v.as_str()).unwrap_or("0x0");
        let block_hex = tx
            .get("blockNumber")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0");

        let amount = u128::from_str_radix(value_hex.trim_start_matches("0x"), 16).unwrap_or(0);
        let block_height = u64::from_str_radix(block_hex.trim_start_matches("0x"), 16).unwrap_or(0);

        let mut verified = true;
        if let Some(expected_from) = expected_from {
            if from.to_lowercase() != expected_from.to_lowercase() {
                verified = false;
            }
        }
        if to.to_lowercase() != expected_to.to_lowercase() {
            verified = false;
        }
        if let Some(min_amt) = min_amount {
            if amount < min_amt {
                verified = false;
            }
        }

        Ok(TransferEvidence {
            tx_hash: tx_hash.to_string(),
            from_address: from.to_string(),
            to_address: to.to_string(),
            amount,
            block_height,
            verified,
        })
    }

    async fn build_dormancy_evidence(
        &self,
        request: DormancyEvidenceRequest,
    ) -> Result<DormancyEvidence> {
        let current_height = match request.current_height {
            Some(h) => h,
            None => self.latest_height().await?,
        };

        let activity = self.get_address_activity(&request.address).await?;

        let source_address_hash = format!(
            "0x{}",
            hex::encode(sha2::Sha256::digest(request.address.as_bytes()))
        );

        let evidence_hash = format!(
            "0x{}",
            hex::encode(sha2::Sha256::digest(
                format!(
                    "{}:{}:{}:{}",
                    self.chain_id(),
                    request.address,
                    current_height,
                    activity.dormancy_blocks
                )
                .as_bytes()
            ))
        );

        let (confidence_tier, confidence_score) = if self.endpoint.is_official {
            (5, 40)
        } else {
            (6, 30)
        };

        Ok(DormancyEvidence {
            version: "chrononode:evidence:v1".to_string(),
            chain_id: self.chain_id().to_string(),
            source_type: self.source_type(),
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
            dormancy_seconds: activity.last_seen_timestamp.map(|ts| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs().saturating_sub(ts))
                    .unwrap_or(0)
            }).unwrap_or(0),
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

    async fn verify_evidence(&self, evidence: &DormancyEvidence) -> Result<bool> {
        Ok(evidence.chain_id == self.chain_id())
    }
}

/// Adapter that aggregates multiple evidence sources and cross-checks results.
pub struct MultiSourceAdapter {
    chain_id: String,
    sources: Arc<Mutex<Vec<Box<dyn ChainEvidenceAdapter>>>>,
}

impl MultiSourceAdapter {
    pub fn new(chain_id: String) -> Self {
        Self {
            chain_id,
            sources: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn add_source(&self, source: Box<dyn ChainEvidenceAdapter>) {
        self.sources.lock().await.push(source);
    }
}

#[async_trait]
impl ChainEvidenceAdapter for MultiSourceAdapter {
    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn source_type(&self) -> EvidenceSourceType {
        EvidenceSourceType::MultiSource
    }

    async fn latest_height(&self) -> Result<u64> {
        let sources = self.sources.lock().await;
        if sources.is_empty() {
            return Err(CoreError::Adapter("no sources configured".into()));
        }

        let mut heights = Vec::new();
        for source in sources.iter() {
            if let Ok(h) = source.latest_height().await {
                heights.push(h);
            }
        }
        drop(sources);

        if heights.is_empty() {
            return Err(CoreError::Adapter("all sources failed".into()));
        }

        heights.sort();
        Ok(heights[heights.len() / 2])
    }

    async fn get_address_activity(&self, address: &str) -> Result<AddressActivity> {
        let sources = self.sources.lock().await;
        if sources.is_empty() {
            return Err(CoreError::Adapter("no sources configured".into()));
        }

        let mut activities = Vec::new();
        for source in sources.iter() {
            if let Ok(a) = source.get_address_activity(address).await {
                activities.push(a);
            }
        }
        drop(sources);

        if activities.is_empty() {
            return Err(CoreError::Adapter("all sources failed".into()));
        }

        let current_height = activities.iter().map(|a| a.current_height).max().unwrap_or(0);
        let last_seen_block = activities.iter().filter_map(|a| a.last_seen_block).max();
        let last_seen_tx = activities.iter().find_map(|a| a.last_seen_tx.clone());
        let last_seen_timestamp = activities.iter().filter_map(|a| a.last_seen_timestamp).max();

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
        let sources = self.sources.lock().await;
        for source in sources.iter() {
            if let Ok(result) = source
                .verify_ownership_signature(address, message, signature)
                .await
            {
                if result {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    async fn verify_transfer_claim(
        &self,
        tx_hash: &str,
        expected_from: Option<&str>,
        expected_to: &str,
        min_amount: Option<u128>,
    ) -> Result<TransferEvidence> {
        let sources = self.sources.lock().await;
        let mut results = Vec::new();

        for source in sources.iter() {
            if let Ok(evidence) = source
                .verify_transfer_claim(tx_hash, expected_from, expected_to, min_amount)
                .await
            {
                results.push(evidence);
            }
        }
        drop(sources);

        if results.is_empty() {
            return Ok(TransferEvidence {
                tx_hash: tx_hash.to_string(),
                from_address: String::new(),
                to_address: expected_to.to_string(),
                amount: 0,
                block_height: 0,
                verified: false,
            });
        }

        let verified_count = results.iter().filter(|e| e.verified).count();
        let consensus = verified_count > results.len() / 2;

        let best = results
            .into_iter()
            .max_by_key(|e| if e.verified { 1 } else { 0 })
            .unwrap();

        Ok(TransferEvidence {
            verified: consensus,
            ..best
        })
    }

    async fn build_dormancy_evidence(
        &self,
        request: DormancyEvidenceRequest,
    ) -> Result<DormancyEvidence> {
        let sources = self.sources.lock().await;
        if sources.is_empty() {
            return Err(CoreError::Adapter("no sources configured".into()));
        }

        let mut evidences = Vec::new();
        for source in sources.iter() {
            if let Ok(e) = source.build_dormancy_evidence(request.clone()).await {
                evidences.push(e);
            }
        }
        drop(sources);

        if evidences.is_empty() {
            return Err(CoreError::Adapter("all sources failed".into()));
        }

        let best = evidences
            .into_iter()
            .min_by_key(|e| e.confidence_tier)
            .unwrap();

        Ok(best)
    }

    async fn verify_evidence(&self, evidence: &DormancyEvidence) -> Result<bool> {
        let sources = self.sources.lock().await;
        for source in sources.iter() {
            if let Ok(result) = source.verify_evidence(evidence).await {
                if result {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
