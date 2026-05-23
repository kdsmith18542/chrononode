use async_trait::async_trait;
use chrononode_adapter_sdk::retry::retry_with_backoff_predicate;
use chrononode_core::{
    BlockModel, ChainAdapter, ChronoBlock, ChronoEvent, ChronoTx, CoreError, Result,
};
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: &'static str,
    method: &'static str,
    params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
struct EthereumBlockJson {
    number: String,
    hash: String,
    #[serde(rename = "parentHash")]
    parent_hash: String,
    timestamp: String,
    #[serde(rename = "stateRoot")]
    state_root: String,
    #[serde(rename = "transactionsRoot")]
    transactions_root: String,
    #[serde(rename = "receiptsRoot")]
    receipts_root: String,
    transactions: Vec<EthereumTxJson>,
}

#[derive(Debug, Clone, Deserialize)]
struct EthereumTxJson {
    hash: String,
    from: String,
    to: Option<String>,
    value: String,
    nonce: String,
    input: String,
    gas: String,
}

#[derive(Debug, Clone, Deserialize)]
struct EthereumReceiptJson {
    #[serde(rename = "transactionHash")]
    transaction_hash: String,
    #[serde(rename = "transactionIndex")]
    _transaction_index: String,
    #[serde(rename = "gasUsed")]
    gas_used: String,
    #[serde(rename = "contractAddress")]
    contract_address: Option<String>,
    logs: Vec<EthereumLogJson>,
}

#[derive(Debug, Clone, Deserialize)]
struct EthereumLogJson {
    address: String,
    topics: Vec<String>,
    data: String,
    #[serde(rename = "transactionIndex")]
    transaction_index: String,
    #[serde(rename = "logIndex")]
    log_index: String,
}

#[derive(Serialize, Deserialize)]
struct EthereumExtraData {
    #[serde(rename = "stateRoot")]
    state_root: String,
    #[serde(rename = "transactionsRoot")]
    transactions_root: String,
    #[serde(rename = "receiptsRoot")]
    receipts_root: String,
}

#[derive(Clone)]
pub struct EthereumAdapter {
    chain_id: String,
    client: reqwest::Client,
    api_url: String,
}

impl EthereumAdapter {
    pub fn new(api_url: &str) -> Self {
        Self {
            chain_id: "ethereum".to_string(),
            client: reqwest::Client::new(),
            api_url: api_url.trim_end_matches('/').to_string(),
        }
    }

    async fn call_rpc<T: serde::de::DeserializeOwned>(
        &self,
        method: &'static str,
        params: serde_json::Value,
    ) -> Result<T> {
        let payload = JsonRpcRequest {
            jsonrpc: "2.0",
            id: "chrononode",
            method,
            params,
        };

        let client = self.client.clone();
        let url = self.api_url.clone();

        let response_body: JsonRpcResponse<T> = retry_with_backoff_predicate(
            MAX_RETRIES,
            500,
            || {
                let client = client.clone();
                let url = url.clone();
                let payload = &payload;
                async move {
                    let resp = client.post(&url).json(payload).send().await.map_err(|e| {
                        FetchError::Retryable(format!("RPC connection failed to {}: {}", url, e))
                    })?;

                    if resp.status().is_server_error() {
                        return Err(FetchError::Retryable(format!(
                            "RPC server returned status {} (retryable)",
                            resp.status()
                        )));
                    }

                    if !resp.status().is_success()
                        && resp.status() != reqwest::StatusCode::BAD_REQUEST
                    {
                        return Err(FetchError::Fatal(format!(
                            "RPC request returned status {}",
                            resp.status()
                        )));
                    }

                    resp.json::<JsonRpcResponse<T>>().await.map_err(|e| {
                        FetchError::Fatal(format!("Failed to parse RPC response: {}", e))
                    })
                }
            },
            |e: &FetchError| matches!(e, FetchError::Retryable(_)),
        )
        .await?;

        if let Some(err) = response_body.error {
            return Err(CoreError::Adapter(format!(
                "RPC error {}: {}",
                err.code, err.message
            )));
        }

        response_body
            .result
            .ok_or_else(|| CoreError::NotFound(format!("RPC result is null for method {}", method)))
    }

    fn decode_hex_safe(hex_str: &str) -> Vec<u8> {
        hex::decode(hex_str.trim_start_matches("0x")).unwrap_or_default()
    }

    fn parse_hex_u64(hex_str: &str) -> u64 {
        let hex_clean = hex_str.trim_start_matches("0x");
        u64::from_str_radix(hex_clean, 16).unwrap_or(0)
    }

    fn divide_by_u32(bytes: &[u8], divisor: u32) -> (Vec<u8>, u32) {
        let mut quotient = Vec::with_capacity(bytes.len());
        let mut remainder = 0u64;
        for &byte in bytes {
            let current = (remainder << 8) | byte as u64;
            let q = current / divisor as u64;
            remainder = current % divisor as u64;
            if !quotient.is_empty() || q > 0 {
                quotient.push(q as u8);
            }
        }
        (quotient, remainder as u32)
    }

    fn parse_hex_u256_to_gwei_u64(hex_str: &str) -> u64 {
        let hex_clean = hex_str.trim_start_matches("0x");
        if hex_clean.is_empty() {
            return 0;
        }
        let hex_even = if hex_clean.len() % 2 == 1 {
            format!("0{}", hex_clean)
        } else {
            hex_clean.to_string()
        };
        let bytes = match hex::decode(&hex_even) {
            Ok(b) => b,
            Err(_) => return 0,
        };
        // Divide by 10^9 to convert Wei to Gwei
        let (quotient_bytes, _) = Self::divide_by_u32(&bytes, 1_000_000_000);
        let mut val = 0u64;
        for &byte in &quotient_bytes {
            if let Some(next_val) = val.checked_mul(256) {
                if let Some(sum_val) = next_val.checked_add(byte as u64) {
                    val = sum_val;
                } else {
                    return u64::MAX;
                }
            } else {
                return u64::MAX;
            }
        }
        val
    }

    fn u64_to_hex_quantity(val: u64) -> String {
        format!("0x{:x}", val)
    }

    async fn fetch_receipts_parallel(
        &self,
        tx_hashes: &[String],
    ) -> Result<HashMap<String, EthereumReceiptJson>> {
        if tx_hashes.is_empty() {
            return Ok(HashMap::new());
        }

        let self_clone = self.clone();
        let receipts: Vec<EthereumReceiptJson> = futures::stream::iter(tx_hashes.to_vec())
            .map(move |tx_hash| {
                let adapter = self_clone.clone();
                async move {
                    adapter
                        .call_rpc::<EthereumReceiptJson>(
                            "eth_getTransactionReceipt",
                            serde_json::json!([tx_hash]),
                        )
                        .await
                }
            })
            .buffer_unordered(10)
            .try_collect::<Vec<_>>()
            .await?;

        let mut receipt_map = HashMap::with_capacity(receipts.len());
        for receipt in receipts {
            receipt_map.insert(receipt.transaction_hash.clone(), receipt);
        }
        Ok(receipt_map)
    }

    fn normalize_block(
        &self,
        block: &EthereumBlockJson,
        receipts: &HashMap<String, EthereumReceiptJson>,
    ) -> ChronoBlock {
        let mut transactions = Vec::with_capacity(block.transactions.len());
        let mut events = Vec::new();

        for tx in &block.transactions {
            let receipt = receipts.get(&tx.hash);

            let recipient = if let Some(to_addr) = &tx.to {
                Self::decode_hex_safe(to_addr)
            } else if let Some(rec) = receipt {
                rec.contract_address
                    .as_ref()
                    .map(|addr| Self::decode_hex_safe(addr))
                    .unwrap_or_default()
            } else {
                vec![]
            };

            let gas_used = receipt
                .map(|r| Self::parse_hex_u64(&r.gas_used))
                .unwrap_or(0);

            transactions.push(ChronoTx {
                tx_hash: Self::decode_hex_safe(&tx.hash),
                sender: Self::decode_hex_safe(&tx.from),
                recipient,
                amount: Self::parse_hex_u256_to_gwei_u64(&tx.value),
                nonce: Self::parse_hex_u64(&tx.nonce),
                payload: Self::decode_hex_safe(&tx.input),
                gas_limit: Self::parse_hex_u64(&tx.gas),
                gas_used,
                extra_data: vec![],
            });

            if let Some(r) = receipt {
                for log in &r.logs {
                    let event_type = if let Some(first_topic) = log.topics.first() {
                        first_topic.trim_start_matches("0x").to_string()
                    } else {
                        "log".to_string()
                    };

                    events.push(ChronoEvent {
                        event_type,
                        emitter: Self::decode_hex_safe(&log.address),
                        tx_index: Self::parse_hex_u64(&log.transaction_index),
                        event_index: Self::parse_hex_u64(&log.log_index),
                        payload: Self::decode_hex_safe(&log.data),
                    });
                }
            }
        }

        // Sort events by event_index to ensure deterministic ordering
        events.sort_by_key(|e| e.event_index);

        let extra_data = serde_json::to_vec(&EthereumExtraData {
            state_root: block.state_root.clone(),
            transactions_root: block.transactions_root.clone(),
            receipts_root: block.receipts_root.clone(),
        })
        .unwrap_or_default();

        ChronoBlock {
            schema_version: 1,
            chain_id: self.chain_id.clone(),
            height: Self::parse_hex_u64(&block.number),
            block_hash: Self::decode_hex_safe(&block.hash),
            prev_hash: Self::decode_hex_safe(&block.parent_hash),
            timestamp: Self::parse_hex_u64(&block.timestamp),
            block_model: "Account".to_string(),
            hash_algorithm: "keccak256".to_string(),
            transactions,
            events,
            extra_data,
        }
    }
}

#[async_trait]
impl ChainAdapter for EthereumAdapter {
    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn display_name(&self) -> &str {
        "Ethereum Network"
    }

    fn block_model(&self) -> BlockModel {
        BlockModel::Account
    }

    async fn latest_height(&self) -> Result<u64> {
        let height_hex: String = self
            .call_rpc("eth_blockNumber", serde_json::Value::Array(vec![]))
            .await?;
        Ok(Self::parse_hex_u64(&height_hex))
    }

    async fn fetch_block(&self, height: u64) -> Result<ChronoBlock> {
        let height_hex = Self::u64_to_hex_quantity(height);
        let block_json: EthereumBlockJson = self
            .call_rpc(
                "eth_getBlockByNumber",
                serde_json::json!([height_hex, true]),
            )
            .await?;

        let tx_hashes: Vec<String> = block_json
            .transactions
            .iter()
            .map(|tx| tx.hash.clone())
            .collect();
        let receipts = self.fetch_receipts_parallel(&tx_hashes).await?;

        Ok(self.normalize_block(&block_json, &receipts))
    }

    async fn fetch_block_by_hash(&self, hash: &[u8]) -> Result<ChronoBlock> {
        let hash_hex = format!("0x{}", hex::encode(hash));
        let block_json: EthereumBlockJson = self
            .call_rpc("eth_getBlockByHash", serde_json::json!([hash_hex, true]))
            .await?;

        let tx_hashes: Vec<String> = block_json
            .transactions
            .iter()
            .map(|tx| tx.hash.clone())
            .collect();
        let receipts = self.fetch_receipts_parallel(&tx_hashes).await?;

        Ok(self.normalize_block(&block_json, &receipts))
    }
}

pub fn init() {
    chrononode_adapter_sdk::registry::register("ethereum", "Ethereum Network", |config| {
        let url = config
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:8545");
        Ok(Arc::new(EthereumAdapter::new(url)))
    });
}
