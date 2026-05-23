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
struct BlockstreamBlock {
    id: String,
    height: u64,
    version: i64,
    timestamp: u64,
    tx_count: u64,
    size: u64,
    weight: u64,
    merkle_root: String,
    previousblockhash: Option<String>,
    nonce: u64,
    bits: u64,
    difficulty: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct BlockstreamTx {
    txid: String,
    version: u32,
    locktime: u32,
    vin: Vec<BlockstreamVin>,
    vout: Vec<BlockstreamVout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockstreamVin {
    txid: Option<String>,
    vout: Option<u64>,
    is_coinbase: Option<bool>,
    scriptsig: Option<String>,
    inner_redeemscript_asm: Option<String>,
    inner_witnessscript_asm: Option<String>,
    sequence: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockstreamVout {
    value: u64,
    scriptpubkey_address: Option<String>,
    scriptpubkey_asm: Option<String>,
    scriptpubkey_hex: Option<String>,
    scriptpubkey_type: Option<String>,
}

pub struct BitcoinLightAdapter {
    chain_id: String,
    client: reqwest::Client,
    api_url: String,
}

impl BitcoinLightAdapter {
    pub fn new(api_url: &str) -> Self {
        Self {
            chain_id: "bitcoin".to_string(),
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

    async fn get_text(&self, path: &str) -> Result<String> {
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
                    resp.text()
                        .await
                        .map_err(|e| FetchError::Fatal(format!("text read failed: {}", e)))
                }
            },
            |e: &FetchError| matches!(e, FetchError::Retryable(_)),
        )
        .await
        .map_err(Into::into)
    }

    async fn fetch_block_txs(&self, block_hash: &str) -> Result<Vec<BlockstreamTx>> {
        let mut all_txs = Vec::new();
        let mut start_index = 0u64;
        loop {
            let path = format!("/api/block/{}/txs/{}", block_hash, start_index);
            let json = self.get(&path).await?;
            let txs: Vec<BlockstreamTx> = serde_json::from_value(json)
                .map_err(|e| CoreError::Adapter(format!("failed to parse txs: {}", e)))?;
            let count = txs.len() as u64;
            all_txs.extend(txs);
            if count < 25 {
                break;
            }
            start_index += 25;
        }
        Ok(all_txs)
    }

    fn decode_hex_safe(hex_str: &str) -> Vec<u8> {
        hex::decode(hex_str.trim_start_matches("0x")).unwrap_or_default()
    }

    fn parse_block(&self, block: &BlockstreamBlock, txs: &[BlockstreamTx]) -> ChronoBlock {
        let transactions: Vec<ChronoTx> = txs
            .iter()
            .map(|tx| {
                let sender = if let Some(first_in) = tx.vin.first() {
                    if first_in.is_coinbase.unwrap_or(false) {
                        b"coinbase".to_vec()
                    } else if let Some(txid) = &first_in.txid {
                        let vout = first_in.vout.unwrap_or(0);
                        format!("{}:{}", txid, vout).into_bytes()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                let recipient = if let Some(first_out) = tx.vout.first() {
                    if let Some(addr) = &first_out.scriptpubkey_address {
                        addr.as_bytes().to_vec()
                    } else {
                        Self::decode_hex_safe(first_out.scriptpubkey_hex.as_deref().unwrap_or(""))
                    }
                } else {
                    vec![]
                };

                let total_sats: u64 = tx.vout.iter().map(|o| o.value).sum();

                let extra_data = serde_json::to_vec(&serde_json::json!({
                    "vin": tx.vin,
                    "vout": tx.vout,
                }))
                .unwrap_or_default();

                ChronoTx {
                    tx_hash: Self::decode_hex_safe(&tx.txid),
                    sender,
                    recipient,
                    amount: total_sats,
                    nonce: tx.locktime as u64,
                    payload: vec![],
                    gas_limit: 0,
                    gas_used: 0,
                    extra_data,
                }
            })
            .collect();

        let prev_hash = match &block.previousblockhash {
            Some(h) => Self::decode_hex_safe(h),
            None => vec![0u8; 32],
        };

        ChronoBlock {
            schema_version: 1,
            chain_id: self.chain_id.clone(),
            height: block.height,
            block_hash: Self::decode_hex_safe(&block.id),
            prev_hash,
            timestamp: block.timestamp,
            block_model: "Utxo".to_string(),
            hash_algorithm: "sha256d".to_string(),
            transactions,
            events: vec![],
            extra_data: vec![],
        }
    }
}

#[async_trait]
impl ChainAdapter for BitcoinLightAdapter {
    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn display_name(&self) -> &str {
        "Bitcoin Light (Blockstream)"
    }

    fn block_model(&self) -> BlockModel {
        BlockModel::Utxo
    }

    async fn latest_height(&self) -> Result<u64> {
        let text = self.get_text("/api/blocks/tip/height").await?;
        text.trim()
            .parse::<u64>()
            .map_err(|e| CoreError::Adapter(format!("invalid tip height response: {}", e)))
    }

    async fn fetch_block(&self, height: u64) -> Result<ChronoBlock> {
        let hash = self
            .get_text(&format!("/api/block-height/{}", height))
            .await?;
        let hash = hash.trim().to_string();
        self.fetch_block_by_hash_str(&hash).await
    }

    async fn fetch_block_by_hash(&self, hash: &[u8]) -> Result<ChronoBlock> {
        let hash_hex = hex::encode(hash);
        self.fetch_block_by_hash_str(&hash_hex).await
    }
}

impl BitcoinLightAdapter {
    async fn fetch_block_by_hash_str(&self, hash_hex: &str) -> Result<ChronoBlock> {
        let block_json = self.get(&format!("/api/block/{}", hash_hex)).await?;
        let block: BlockstreamBlock = serde_json::from_value(block_json)
            .map_err(|e| CoreError::Adapter(format!("failed to parse block: {}", e)))?;
        let txs = self.fetch_block_txs(hash_hex).await?;
        Ok(self.parse_block(&block, &txs))
    }
}

pub fn init() {
    chrononode_adapter_sdk::registry::register(
        "bitcoin-light",
        "Bitcoin Light (Blockstream)",
        |config| {
            let url = config
                .get("api_url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://blockstream.info");
            Ok(Arc::new(BitcoinLightAdapter::new(url)))
        },
    );
}
