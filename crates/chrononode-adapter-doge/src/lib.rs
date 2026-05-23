use async_trait::async_trait;
use chrononode_adapter_sdk::retry::retry_with_backoff_predicate;
use chrononode_core::{BlockModel, ChainAdapter, ChronoBlock, ChronoTx, CoreError, Result};
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

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct BlockCypherChain {
    height: u64,
    hash: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct BlockCypherBlock {
    hash: String,
    height: u64,
    chain: String,
    time: u64,
    received_time: u64,
    size: u64,
    prev_block: Option<String>,
    mrkl_root: String,
    txids: Vec<String>,
    nonce: u64,
    bits: String,
}

#[derive(Debug, Clone, Deserialize)]
struct BlockCypherTx {
    tx_hash: String,
    vin: Vec<BlockCypherVin>,
    vout: Vec<BlockCypherVout>,
    lock_time: u64,
    total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockCypherVin {
    tx_hash: Option<String>,
    vout_index: Option<u64>,
    coinbase: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockCypherVout {
    value: u64,
    scriptpubkey_addresses: Option<Vec<String>>,
}

pub struct DogeAdapter {
    chain_id: String,
    client: reqwest::Client,
    api_url: String,
}

impl DogeAdapter {
    pub fn new(api_url: &str) -> Self {
        Self {
            chain_id: "dogecoin".to_string(),
            client: reqwest::Client::builder()
                .user_agent("chrononode/0.1")
                .build()
                .unwrap_or_default(),
            api_url: api_url.trim_end_matches('/').to_string(),
        }
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.api_url, path);
        let client = self.client.clone();

        retry_with_backoff_predicate(
            MAX_RETRIES,
            1000,
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

    async fn fetch_tx(&self, txid: &str) -> Result<BlockCypherTx> {
        let json = self.get(&format!("/v1/doge/main/txs/{}", txid)).await?;
        serde_json::from_value(json)
            .map_err(|e| CoreError::Adapter(format!("failed to parse tx {}: {}", txid, e)))
    }

    fn parse_block(&self, block: &BlockCypherBlock, txs: &[BlockCypherTx]) -> ChronoBlock {
        let transactions: Vec<ChronoTx> = txs
            .iter()
            .map(|tx| {
                let sender = if let Some(first_in) = tx.vin.first() {
                    if first_in.coinbase.unwrap_or(false) {
                        b"coinbase".to_vec()
                    } else if let Some(tx_hash) = &first_in.tx_hash {
                        let vout = first_in.vout_index.unwrap_or(0);
                        format!("{}:{}", tx_hash, vout).into_bytes()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                let recipient = if let Some(first_out) = tx.vout.first() {
                    if let Some(addrs) = &first_out.scriptpubkey_addresses {
                        addrs
                            .first()
                            .map(|a| a.as_bytes().to_vec())
                            .unwrap_or_default()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                let extra_data = serde_json::to_vec(&serde_json::json!({
                    "vin": tx.vin,
                    "vout": tx.vout,
                }))
                .unwrap_or_default();

                ChronoTx {
                    tx_hash: Self::decode_hex_safe(&tx.tx_hash),
                    sender,
                    recipient,
                    amount: tx.total,
                    nonce: tx.lock_time,
                    payload: vec![],
                    gas_limit: 0,
                    gas_used: 0,
                    extra_data,
                }
            })
            .collect();

        let prev_hash = match &block.prev_block {
            Some(h) => Self::decode_hex_safe(h),
            None => vec![0u8; 32],
        };

        ChronoBlock {
            schema_version: 1,
            chain_id: self.chain_id.clone(),
            height: block.height,
            block_hash: Self::decode_hex_safe(&block.hash),
            prev_hash,
            timestamp: block.time,
            block_model: "Utxo".to_string(),
            hash_algorithm: "scrypt".to_string(),
            transactions,
            events: vec![],
            extra_data: vec![],
        }
    }
}

#[async_trait]
impl ChainAdapter for DogeAdapter {
    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn display_name(&self) -> &str {
        "Dogecoin (BlockCypher)"
    }

    fn block_model(&self) -> BlockModel {
        BlockModel::Utxo
    }

    async fn latest_height(&self) -> Result<u64> {
        let json = self.get("/v1/doge/main").await?;
        let chain: BlockCypherChain = serde_json::from_value(json)
            .map_err(|e| CoreError::Adapter(format!("failed to parse chain info: {}", e)))?;
        Ok(chain.height)
    }

    async fn fetch_block(&self, height: u64) -> Result<ChronoBlock> {
        let json = self
            .get(&format!("/v1/doge/main/blocks/{}", height))
            .await?;
        let block: BlockCypherBlock = serde_json::from_value(json)
            .map_err(|e| CoreError::Adapter(format!("failed to parse block: {}", e)))?;
        let mut txs = Vec::with_capacity(block.txids.len());
        for txid in &block.txids {
            let tx = self.fetch_tx(txid).await?;
            txs.push(tx);
        }
        Ok(self.parse_block(&block, &txs))
    }

    async fn fetch_block_by_hash(&self, hash: &[u8]) -> Result<ChronoBlock> {
        let hash_hex = hex::encode(hash);
        let json = self
            .get(&format!("/v1/doge/main/blocks/{}", hash_hex))
            .await?;
        let block: BlockCypherBlock = serde_json::from_value(json)
            .map_err(|e| CoreError::Adapter(format!("failed to parse block: {}", e)))?;
        let mut txs = Vec::with_capacity(block.txids.len());
        for txid in &block.txids {
            let tx = self.fetch_tx(txid).await?;
            txs.push(tx);
        }
        Ok(self.parse_block(&block, &txs))
    }
}

pub fn init() {
    chrononode_adapter_sdk::registry::register("dogecoin", "Dogecoin (BlockCypher)", |config| {
        let url = config
            .get("api_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://api.blockcypher.com");
        Ok(Arc::new(DogeAdapter::new(url)))
    });
}
