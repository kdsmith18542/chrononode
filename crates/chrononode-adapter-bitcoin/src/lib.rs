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
struct BitcoinBlockJson {
    hash: String,
    height: u64,
    time: u64,
    previousblockhash: Option<String>,
    tx: Vec<BitcoinTxJson>,
}

#[derive(Debug, Clone, Deserialize)]
struct BitcoinTxJson {
    txid: String,
    locktime: u64,
    vin: Vec<BitcoinVinJson>,
    vout: Vec<BitcoinVoutJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BitcoinVinJson {
    coinbase: Option<String>,
    txid: Option<String>,
    vout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BitcoinVoutJson {
    value: f64,
    #[serde(rename = "scriptPubKey")]
    script_pub_key: BitcoinScriptPubKeyJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BitcoinScriptPubKeyJson {
    hex: String,
    address: Option<String>,
}

pub struct BitcoinAdapter {
    chain_id: String,
    client: reqwest::Client,
    api_url: String,
    username: Option<String>,
    password: Option<String>,
}

impl BitcoinAdapter {
    pub fn new(
        api_url: &str,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        Self {
            chain_id: "bitcoin".to_string(),
            client: reqwest::Client::new(),
            api_url: api_url.trim_end_matches('/').to_string(),
            username,
            password,
        }
    }

    async fn call_rpc<T: serde::de::DeserializeOwned>(&self, method: &'static str, params: serde_json::Value) -> Result<T> {
        let payload = JsonRpcRequest {
            jsonrpc: "1.0",
            id: "chrononode",
            method,
            params,
        };

        let client = self.client.clone();
        let url = self.api_url.clone();
        let username = self.username.clone();
        let password = self.password.clone();

        let response_body: JsonRpcResponse<T> = retry_with_backoff_predicate(
            MAX_RETRIES,
            500,
            || {
                let client = client.clone();
                let url = url.clone();
                let payload = &payload;
                let username = username.clone();
                let password = password.clone();
                async move {
                    let mut req = client.post(&url).json(payload);
                    if let (Some(u), Some(p)) = (username, password) {
                        req = req.basic_auth(u, Some(p));
                    }
                    
                    let resp = req.send().await.map_err(|e| {
                        FetchError::Retryable(format!("RPC connection failed to {}: {}", url, e))
                    })?;

                    if resp.status().is_server_error() {
                        return Err(FetchError::Retryable(format!(
                            "RPC server returned status {} (retryable)",
                            resp.status()
                        )));
                    }

                    if !resp.status().is_success() && resp.status() != reqwest::StatusCode::BAD_REQUEST {
                        return Err(FetchError::Fatal(format!(
                            "RPC request returned status {}",
                            resp.status()
                        )));
                    }

                    resp.json::<JsonRpcResponse<T>>()
                        .await
                        .map_err(|e| FetchError::Fatal(format!("Failed to parse RPC response: {}", e)))
                }
            },
            |e: &FetchError| matches!(e, FetchError::Retryable(_)),
        )
        .await?;

        if let Some(err) = response_body.error {
            if err.code == -5 {
                return Err(CoreError::NotFound(err.message));
            }
            return Err(CoreError::Adapter(format!("RPC error {}: {}", err.code, err.message)));
        }

        response_body.result.ok_or_else(|| {
            CoreError::Adapter("RPC response missing both result and error".to_string())
        })
    }

    fn decode_hex_safe(hex_str: &str) -> Vec<u8> {
        hex::decode(hex_str.trim_start_matches("0x")).unwrap_or_default()
    }

    fn parse_block(&self, block: &BitcoinBlockJson) -> ChronoBlock {
        let transactions: Vec<ChronoTx> = block
            .tx
            .iter()
            .map(|tx| {
                let sender = if let Some(first_in) = tx.vin.first() {
                    if first_in.coinbase.is_some() {
                        b"coinbase".to_vec()
                    } else if let (Some(txid), Some(vout)) = (&first_in.txid, first_in.vout) {
                        format!("{}:{}", txid, vout).into_bytes()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                let recipient = if let Some(first_out) = tx.vout.first() {
                    if let Some(addr) = &first_out.script_pub_key.address {
                        addr.as_bytes().to_vec()
                    } else {
                        Self::decode_hex_safe(&first_out.script_pub_key.hex)
                    }
                } else {
                    vec![]
                };

                // Sum outputs values and convert BTC to Satoshis
                let total_btc: f64 = tx.vout.iter().map(|o| o.value).sum();
                let amount_sats = (total_btc * 100_000_000.0).round() as u64;

                let extra_data = serde_json::to_vec(&serde_json::json!({
                    "vin": tx.vin,
                    "vout": tx.vout,
                })).unwrap_or_default();

                ChronoTx {
                    tx_hash: Self::decode_hex_safe(&tx.txid),
                    sender,
                    recipient,
                    amount: amount_sats,
                    nonce: tx.locktime,
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
            block_hash: Self::decode_hex_safe(&block.hash),
            prev_hash,
            timestamp: block.time,
            block_model: "Utxo".to_string(),
            hash_algorithm: "sha256d".to_string(),
            transactions,
            events: vec![],
            extra_data: vec![],
        }
    }
}

#[async_trait]
impl ChainAdapter for BitcoinAdapter {
    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn display_name(&self) -> &str {
        "Bitcoin Network"
    }

    fn block_model(&self) -> BlockModel {
        BlockModel::Utxo
    }

    async fn latest_height(&self) -> Result<u64> {
        self.call_rpc("getblockcount", serde_json::Value::Array(vec![])).await
    }

    async fn fetch_block(&self, height: u64) -> Result<ChronoBlock> {
        let hash_hex: String = self.call_rpc("getblockhash", serde_json::json!([height])).await?;
        let block_json: BitcoinBlockJson = self.call_rpc("getblock", serde_json::json!([hash_hex, 2])).await?;
        Ok(self.parse_block(&block_json))
    }

    async fn fetch_block_by_hash(&self, hash: &[u8]) -> Result<ChronoBlock> {
        let hash_hex = hex::encode(hash);
        let block_json: BitcoinBlockJson = self.call_rpc("getblock", serde_json::json!([hash_hex, 2])).await?;
        Ok(self.parse_block(&block_json))
    }
}

pub fn init() {
    chrononode_adapter_sdk::registry::register("bitcoin", "Bitcoin Network", |config| {
        let url = config
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:8332");
        let username = config
            .get("username")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let password = config
            .get("password")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Ok(Arc::new(BitcoinAdapter::new(url, username, password)))
    });
}
