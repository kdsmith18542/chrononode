use async_trait::async_trait;
use chrononode_adapter_sdk::retry::retry_with_backoff_predicate;
use chrononode_core::{
    BlockModel, ChainAdapter, ChronoBlock, ChronoEvent, ChronoTx, CoreError, Result,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const MAX_RETRIES: u32 = 5;

#[derive(Debug)]
enum FetchError {
    Retryable(String),
    Fatal(String),
}

impl From<FetchError> for CoreError {
    fn from(e: FetchError) -> CoreError {
        match e {
            FetchError::Retryable(msg) | FetchError::Fatal(msg) => CoreError::Adapter(msg),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BaalsBlock {
    index: u64,
    timestamp: u64,
    prev_hash: String,
    state_root: String,
    hash: String,
    nonce: u64,
    transactions: Vec<BaalsTransaction>,
    #[serde(default)]
    total_gas_used: u64,
    #[serde(default)]
    signer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BaalsTransaction {
    hash: String,
    sender: String,
    nonce: u64,
    timestamp: u64,
    recipient: String,
    payload: BaalsPayload,
    gas_limit: u64,
    gas_price: u64,
    #[serde(default)]
    priority: u8,
    #[serde(default)]
    chain_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BaalsPayload {
    Transfer {
        amount: u64,
    },
    ContractDeploy {
        wasm_bytes: String,
        init_payload: Option<String>,
    },
    ContractCall {
        method: String,
        args: Vec<String>,
        value: Option<u64>,
    },
    Data {
        data: String,
    },
    ValidatorSetChange {
        added: Vec<String>,
        removed: Vec<String>,
        effective_height: u64,
    },
}

pub struct BaalsAdapter {
    chain_id: String,
    client: reqwest::Client,
    api_url: String,
}

impl BaalsAdapter {
    pub fn new(api_url: &str) -> Self {
        Self {
            chain_id: "baals".to_string(),
            client: reqwest::Client::new(),
            api_url: api_url.trim_end_matches('/').to_string(),
        }
    }

    async fn get_url(&self, path: &str) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.api_url, path);
        let client = self.client.clone();

        retry_with_backoff_predicate(
            MAX_RETRIES,
            500,
            || {
                let url = url.clone();
                let client = client.clone();
                async move {
                    let resp =
                        client.get(&url).send().await.map_err(|e| {
                            FetchError::Retryable(format!("GET {} failed: {}", url, e))
                        })?;
                    if resp.status().is_server_error() {
                        return Err(FetchError::Retryable(format!(
                            "GET {} returned {} (retryable)",
                            url,
                            resp.status()
                        )));
                    }
                    if !resp.status().is_success() {
                        return Err(FetchError::Fatal(format!(
                            "GET {} returned {}",
                            url,
                            resp.status()
                        )));
                    }
                    resp.json()
                        .await
                        .map_err(|e| FetchError::Fatal(format!("JSON parse failed: {}", e)))
                }
            },
            |e: &FetchError| matches!(e, FetchError::Retryable(_)),
        )
        .await
        .map_err(Into::into)
    }

    fn decode_hex_safe(hex_str: &str) -> Vec<u8> {
        hex::decode(hex_str.trim_start_matches("0x")).unwrap_or_default()
    }

    fn parse_block(&self, block: &BaalsBlock) -> ChronoBlock {
        let transactions: Vec<ChronoTx> = block
            .transactions
            .iter()
            .map(|tx| {
                let amount = match &tx.payload {
                    BaalsPayload::Transfer { amount } => *amount,
                    _ => 0,
                };

                let payload_bytes = match &tx.payload {
                    BaalsPayload::ContractDeploy { wasm_bytes, .. } => {
                        Self::decode_hex_safe(wasm_bytes)
                    }
                    BaalsPayload::ContractCall { method, args, .. } => {
                        format!("{}:{}", method, args.join(",")).into_bytes()
                    }
                    BaalsPayload::Data { data } => Self::decode_hex_safe(data),
                    _ => vec![],
                };

                ChronoTx {
                    tx_hash: Self::decode_hex_safe(&tx.hash),
                    sender: Self::decode_hex_safe(&tx.sender),
                    recipient: Self::decode_hex_safe(&tx.recipient),
                    amount,
                    nonce: tx.nonce,
                    payload: payload_bytes,
                    gas_limit: tx.gas_limit,
                    gas_used: tx.gas_limit.saturating_mul(tx.gas_price),
                    extra_data: vec![],
                }
            })
            .collect();

        let events: Vec<ChronoEvent> = block
            .transactions
            .iter()
            .enumerate()
            .flat_map(|(tx_idx, tx)| match &tx.payload {
                BaalsPayload::ContractCall { method, .. } => {
                    vec![ChronoEvent {
                        event_type: "contract_call".to_string(),
                        emitter: Self::decode_hex_safe(&tx.recipient),
                        tx_index: tx_idx as u64,
                        event_index: 0,
                        payload: method.as_bytes().to_vec(),
                    }]
                }
                BaalsPayload::ContractDeploy { .. } => {
                    vec![ChronoEvent {
                        event_type: "contract_deploy".to_string(),
                        emitter: Self::decode_hex_safe(&tx.sender),
                        tx_index: tx_idx as u64,
                        event_index: 0,
                        payload: vec![],
                    }]
                }
                _ => vec![],
            })
            .collect();

        ChronoBlock {
            schema_version: 1,
            chain_id: self.chain_id.clone(),
            height: block.index,
            block_hash: Self::decode_hex_safe(&block.hash),
            prev_hash: Self::decode_hex_safe(&block.prev_hash),
            timestamp: block.timestamp,
            block_model: "Account".to_string(),
            hash_algorithm: "sha256".to_string(),
            transactions,
            events,
            extra_data: block
                .signer
                .as_ref()
                .map(|s| s.as_bytes().to_vec())
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl ChainAdapter for BaalsAdapter {
    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn display_name(&self) -> &str {
        "BaaLS Network"
    }

    fn block_model(&self) -> BlockModel {
        BlockModel::Account
    }

    async fn latest_height(&self) -> Result<u64> {
        let result = self.get_url("/api/v1/chain/head").await?;
        result
            .get("latest_block_index")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                CoreError::Adapter("missing latest_block_index in chain head response".to_string())
            })
    }

    async fn fetch_block(&self, height: u64) -> Result<ChronoBlock> {
        let result = self.get_url(&format!("/api/v1/blocks/{}", height)).await?;
        let block: BaalsBlock = serde_json::from_value(result)
            .map_err(|e| CoreError::Adapter(format!("failed to parse block: {}", e)))?;
        Ok(self.parse_block(&block))
    }

    async fn fetch_block_by_hash(&self, hash: &[u8]) -> Result<ChronoBlock> {
        let hash_hex = hex::encode(hash);
        let result = self
            .get_url(&format!("/api/v1/blocks/by_hash/{}", hash_hex))
            .await?;
        let block: BaalsBlock = serde_json::from_value(result)
            .map_err(|e| CoreError::Adapter(format!("failed to parse block: {}", e)))?;
        Ok(self.parse_block(&block))
    }
}

pub fn init() {
    chrononode_adapter_sdk::registry::register("baals", "BaaLS Network", |config| {
        let url = config
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:8080");
        Ok(Arc::new(BaalsAdapter::new(url)))
    });
}
