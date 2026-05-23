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
    // BlockCypher returns ISO 8601 strings: "2021-04-23T09:24:36Z"
    time: String,
    received_time: String,
    size: u64,
    prev_block: Option<String>,
    mrkl_root: String,
    txids: Vec<String>,
    nonce: u64,
    // BlockCypher returns bits as an integer, not a hex string
    bits: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct BlockCypherTx {
    // BlockCypher uses "hash", not "tx_hash"
    hash: String,
    // BlockCypher uses "inputs"/"outputs", not "vin"/"vout"
    inputs: Vec<BlockCypherVin>,
    outputs: Vec<BlockCypherVout>,
    #[serde(default)]
    lock_time: u64,
    total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockCypherVin {
    // Previous tx hash — absent for coinbase inputs
    prev_hash: Option<String>,
    // -1 signals a coinbase input; otherwise the spent output index
    output_index: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlockCypherVout {
    value: u64,
    // BlockCypher uses "addresses", not "scriptpubkey_addresses"
    addresses: Option<Vec<String>>,
}

pub struct DogeAdapter {
    chain_id: String,
    client: reqwest::Client,
    api_urls: Vec<String>,
    api_token: Option<String>,
}

impl DogeAdapter {
    pub fn new(api_url: &str) -> Self {
        Self::new_with_options(vec![api_url.to_string()], None)
    }

    pub fn new_with_options(api_urls: Vec<String>, api_token: Option<String>) -> Self {
        Self {
            chain_id: "dogecoin".to_string(),
            client: reqwest::Client::builder()
                .user_agent("chrononode/0.1")
                .build()
                .unwrap_or_default(),
            api_urls: Self::normalize_api_urls(api_urls),
            api_token: api_token
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty()),
        }
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let mut last_err: Option<CoreError> = None;

        for base_url in &self.api_urls {
            let url = self.build_url(base_url, path);
            match self.get_single(&url).await {
                Ok(value) => return Ok(value),
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        Err(last_err
            .unwrap_or_else(|| CoreError::Adapter("no dogecoin API URLs configured".to_string())))
    }

    fn decode_hex_safe(hex_str: &str) -> Vec<u8> {
        hex::decode(hex_str.trim_start_matches("0x")).unwrap_or_default()
    }

    fn normalize_api_urls(api_urls: Vec<String>) -> Vec<String> {
        let mut out = Vec::new();
        for url in api_urls {
            let trimmed = url.trim().trim_end_matches('/').to_string();
            if !trimmed.is_empty() && !out.contains(&trimmed) {
                out.push(trimmed);
            }
        }
        if out.is_empty() {
            out.push("https://api.blockcypher.com".to_string());
        }
        out
    }

    fn is_retryable_status(status: reqwest::StatusCode) -> bool {
        status.is_server_error()
            || status == reqwest::StatusCode::TOO_MANY_REQUESTS
            || status == reqwest::StatusCode::REQUEST_TIMEOUT
    }

    fn redact_url(url: &str) -> String {
        let mut out = url.to_string();
        if let Some(token_pos) = out.find("token=") {
            let value_start = token_pos + "token=".len();
            let value_end = out[value_start..]
                .find('&')
                .map(|i| value_start + i)
                .unwrap_or(out.len());
            out.replace_range(value_start..value_end, "[redacted]");
        }
        out
    }

    fn build_url(&self, base_url: &str, path: &str) -> String {
        let mut url = format!("{}{}", base_url, path);
        if let Some(token) = &self.api_token {
            if url.contains('?') {
                url.push('&');
            } else {
                url.push('?');
            }
            url.push_str("token=");
            url.push_str(token);
        }
        url
    }

    async fn get_single(&self, url: &str) -> Result<serde_json::Value> {
        let url = url.to_string();
        let display_url = Self::redact_url(&url);
        let client = self.client.clone();

        retry_with_backoff_predicate(
            MAX_RETRIES,
            1000,
            || {
                let url = url.clone();
                let display_url = display_url.clone();
                let client = client.clone();
                async move {
                    let resp = client.get(&url).send().await.map_err(|e| {
                        FetchError::Retryable(format!("GET {} failed: {}", display_url, e))
                    })?;

                    if Self::is_retryable_status(resp.status()) {
                        return Err(FetchError::Retryable(format!(
                            "GET {} returned {} (retryable)",
                            display_url,
                            resp.status()
                        )));
                    }
                    if !resp.status().is_success() {
                        return Err(FetchError::Fatal(format!(
                            "GET {} returned {}",
                            display_url,
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

    /// Parse ISO 8601 timestamp ("2021-04-23T09:24:36Z") to Unix seconds.
    /// Avoids adding a chrono dep to this crate.
    fn parse_timestamp(s: &str) -> u64 {
        let s = s.trim_end_matches('Z');
        let parts: Vec<&str> = s.splitn(2, 'T').collect();
        if parts.len() != 2 {
            return 0;
        }
        let date: Vec<u64> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
        let time: Vec<u64> = parts[1].split(':').filter_map(|p| p.parse().ok()).collect();
        if date.len() != 3 || time.len() != 3 {
            return 0;
        }
        let (y, m, d) = (date[0], date[1], date[2]);
        let (h, mi, sec) = (time[0], time[1], time[2]);
        // Civil-time to days-since-epoch (Gregorian calendar)
        let m_adj = if m <= 2 { m + 9 } else { m - 3 };
        let y_adj = if m <= 2 { y - 1 } else { y };
        let era = y_adj / 400;
        let yoe = y_adj - era * 400;
        let doy = (153 * m_adj + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146097 + doe;
        let epoch_days = days.saturating_sub(719468);
        epoch_days * 86400 + h * 3600 + mi * 60 + sec
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
                let sender = if let Some(first_in) = tx.inputs.first() {
                    let is_coinbase = first_in.output_index == Some(-1);
                    if is_coinbase {
                        b"coinbase".to_vec()
                    } else if let Some(prev_hash) = &first_in.prev_hash {
                        let vout = first_in.output_index.unwrap_or(0).max(0) as u64;
                        format!("{}:{}", prev_hash, vout).into_bytes()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                let recipient = if let Some(first_out) = tx.outputs.first() {
                    if let Some(addrs) = &first_out.addresses {
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
                    "inputs": tx.inputs,
                    "outputs": tx.outputs,
                }))
                .unwrap_or_default();

                ChronoTx {
                    tx_hash: Self::decode_hex_safe(&tx.hash),
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
            timestamp: Self::parse_timestamp(&block.time),
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
        let mut api_urls = config
            .get("api_urls")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if api_urls.is_empty() {
            if let Some(url) = config.get("api_url").and_then(|v| v.as_str()) {
                api_urls.push(url.to_string());
            }
        }

        let api_token = config
            .get("api_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| std::env::var("CHRONONODE_DOGE_API_TOKEN").ok());

        Ok(Arc::new(DogeAdapter::new_with_options(api_urls, api_token)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_timestamp_known_date() {
        // 2021-04-23T09:24:36Z = Unix 1619169876 (verified externally)
        assert_eq!(
            DogeAdapter::parse_timestamp("2021-04-23T09:24:36Z"),
            1619169876
        );
    }

    #[test]
    fn test_parse_timestamp_epoch() {
        assert_eq!(DogeAdapter::parse_timestamp("1970-01-01T00:00:00Z"), 0);
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        assert_eq!(DogeAdapter::parse_timestamp("not-a-date"), 0);
    }

    #[test]
    fn test_block_deserialize_real_shape() {
        let json = serde_json::json!({
            "hash": "d0f0af23aadcf6b8d4a681ee930e39d1e64aca967187fa8a0c655c6dacfa22ce",
            "height": 3700000u64,
            "chain": "DOGE.main",
            "time": "2021-04-23T09:24:36Z",
            "received_time": "2021-04-23T09:24:36Z",
            "size": 11506u64,
            "prev_block": "25f2a076e37d8d16a3def4187507b8084159e7198cad44d5ba3577d3426fa8f5",
            "mrkl_root": "7f95a44b575df8fcc58fe19d8c35e1d43c208f10c2a61f5902fcfd7a97dafeaf",
            "txids": ["e8b1d033b222c3c5a104d3ef1a8c931363bfb881a869b8bc57ab02504e30a141"],
            "nonce": 0u64,
            "bits": 436482088u64,
        });
        let block: BlockCypherBlock = serde_json::from_value(json).unwrap();
        assert_eq!(block.height, 3700000);
        assert_eq!(block.bits, 436482088);
        assert_eq!(DogeAdapter::parse_timestamp(&block.time), 1619169876);
    }

    #[test]
    fn test_tx_deserialize_coinbase() {
        let json = serde_json::json!({
            "hash": "e8b1d033b222c3c5a104d3ef1a8c931363bfb881a869b8bc57ab02504e30a141",
            "inputs": [{ "output_index": -1 }],
            "outputs": [{
                "value": 1008861401632u64,
                "addresses": ["D5gKqqDSirsdVpNA9efWKaBmsGD7TcckQ9"]
            }],
            "total": 1008861401632u64,
        });
        let tx: BlockCypherTx = serde_json::from_value(json).unwrap();
        assert_eq!(
            tx.hash,
            "e8b1d033b222c3c5a104d3ef1a8c931363bfb881a869b8bc57ab02504e30a141"
        );
        assert_eq!(tx.inputs[0].output_index, Some(-1));
        assert_eq!(
            tx.outputs[0].addresses.as_ref().unwrap()[0],
            "D5gKqqDSirsdVpNA9efWKaBmsGD7TcckQ9"
        );
    }

    #[test]
    fn test_tx_deserialize_regular() {
        let json = serde_json::json!({
            "hash": "abcd1234",
            "inputs": [{
                "prev_hash": "deadbeef",
                "output_index": 0
            }],
            "outputs": [{
                "value": 500000000u64,
                "addresses": ["DAddr123"]
            }],
            "total": 500000000u64,
        });
        let tx: BlockCypherTx = serde_json::from_value(json).unwrap();
        assert_eq!(tx.inputs[0].prev_hash.as_deref(), Some("deadbeef"));
        assert_eq!(tx.inputs[0].output_index, Some(0));
    }

    #[test]
    fn test_redact_url_hides_token_value() {
        let redacted =
            DogeAdapter::redact_url("https://api.blockcypher.com/v1/doge/main?token=abc123");
        assert!(redacted.contains("token=[redacted]"));
        assert!(!redacted.contains("abc123"));
    }
}
