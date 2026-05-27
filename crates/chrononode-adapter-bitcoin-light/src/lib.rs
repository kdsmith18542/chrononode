use async_trait::async_trait;
use chrononode_adapter_sdk::retry::retry_with_backoff_predicate;
use chrononode_core::{BlockModel, ChainAdapter, ChronoBlock, ChronoTx, CoreError, Result};
use chrononode_core::address_evidence::{
    AddressEvidenceAdapter, AddressSummary, AddressTx, AddressLastActivity,
    AddressTransferEvidence, TxMerkleProof, UtxoEntry, UtxoStatus,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BitcoinProviderMode {
    Esplora,
    JsonRpc,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<serde_json::Value>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BitcoinRpcBlock {
    hash: String,
    height: u64,
    time: u64,
    previousblockhash: Option<String>,
    #[serde(default)]
    tx: Vec<BitcoinRpcTx>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BitcoinRpcTx {
    txid: String,
    #[serde(default)]
    vin: Vec<BitcoinRpcVin>,
    #[serde(default)]
    vout: Vec<BitcoinRpcVout>,
    #[serde(default)]
    locktime: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BitcoinRpcVin {
    txid: Option<String>,
    vout: Option<u64>,
    coinbase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BitcoinRpcVout {
    value: serde_json::Value,
    #[serde(rename = "scriptPubKey")]
    script_pub_key: BitcoinRpcScriptPubKey,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BitcoinRpcScriptPubKey {
    address: Option<String>,
    addresses: Option<Vec<String>>,
    hex: Option<String>,
}

pub struct BitcoinLightAdapter {
    chain_id: String,
    client: reqwest::Client,
    mode: BitcoinProviderMode,
    api_urls: Vec<String>,
    rpc_url: Option<String>,
    rpc_username: Option<String>,
    rpc_password: Option<String>,
    rpc_api_key_header: Option<String>,
    rpc_api_key: Option<String>,
}

impl BitcoinLightAdapter {
    pub fn new(api_url: &str) -> Self {
        Self::new_with_fallbacks(vec![api_url.to_string()])
    }

    pub fn new_with_fallbacks(api_urls: Vec<String>) -> Self {
        Self {
            chain_id: "bitcoin".to_string(),
            client: reqwest::Client::builder()
                .user_agent("chrononode/0.1")
                .build()
                .unwrap_or_default(),
            mode: BitcoinProviderMode::Esplora,
            api_urls: Self::normalize_api_urls(api_urls),
            rpc_url: None,
            rpc_username: None,
            rpc_password: None,
            rpc_api_key_header: None,
            rpc_api_key: None,
        }
    }

    pub fn new_rpc(
        rpc_url: &str,
        rpc_username: Option<String>,
        rpc_password: Option<String>,
        rpc_api_key: Option<String>,
        rpc_api_key_header: Option<String>,
    ) -> Self {
        Self {
            chain_id: "bitcoin".to_string(),
            client: reqwest::Client::builder()
                .user_agent("chrononode/0.1")
                .build()
                .unwrap_or_default(),
            mode: BitcoinProviderMode::JsonRpc,
            api_urls: Vec::new(),
            rpc_url: Some(rpc_url.trim().trim_end_matches('/').to_string()),
            rpc_username: rpc_username
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            rpc_password,
            rpc_api_key_header: rpc_api_key_header
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            rpc_api_key: rpc_api_key
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        }
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let mut last_err: Option<CoreError> = None;
        for base_url in &self.api_urls {
            let url = format!("{}{}", base_url, path);
            match self.get_single(&url).await {
                Ok(value) => return Ok(value),
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            CoreError::Adapter("no bitcoin-light API URLs configured".to_string())
        }))
    }

    async fn get_text(&self, path: &str) -> Result<String> {
        let mut last_err: Option<CoreError> = None;
        for base_url in &self.api_urls {
            let url = format!("{}{}", base_url, path);
            match self.get_text_single(&url).await {
                Ok(value) => return Ok(value),
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            CoreError::Adapter("no bitcoin-light API URLs configured".to_string())
        }))
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

    fn normalize_api_urls(api_urls: Vec<String>) -> Vec<String> {
        let mut out = Vec::new();
        for url in api_urls {
            let trimmed = url.trim().trim_end_matches('/').to_string();
            if !trimmed.is_empty() && !out.contains(&trimmed) {
                out.push(trimmed);
            }
        }
        if out.is_empty() {
            out.push("https://mempool.space".to_string());
            out.push("https://blockstream.info".to_string());
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
        if let Some(prefix_pos) = out.find("go.getblock.io/") {
            let segment_start = prefix_pos + "go.getblock.io/".len();
            let segment_end = out[segment_start..]
                .find('/')
                .map(|i| segment_start + i)
                .unwrap_or(out.len());
            if segment_end > segment_start {
                out.replace_range(segment_start..segment_end, "[redacted]");
            }
        }
        out
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

    async fn get_text_single(&self, url: &str) -> Result<String> {
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

    fn rpc_endpoint(&self) -> Result<&str> {
        self.rpc_url
            .as_deref()
            .ok_or_else(|| CoreError::Adapter("missing bitcoin-light rpc_url".to_string()))
    }

    async fn rpc_call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T> {
        let rpc_url = self.rpc_endpoint()?.to_string();
        let display_url = Self::redact_url(&rpc_url);
        let username = self.rpc_username.clone();
        let password = self.rpc_password.clone();
        let api_key = self.rpc_api_key.clone();
        let api_key_header = self
            .rpc_api_key_header
            .clone()
            .unwrap_or_else(|| "x-api-key".to_string());
        let client = self.client.clone();
        let method_name = method.to_string();

        retry_with_backoff_predicate(
            MAX_RETRIES,
            1000,
            || {
                let client = client.clone();
                let rpc_url = rpc_url.clone();
                let display_url = display_url.clone();
                let username = username.clone();
                let password = password.clone();
                let api_key = api_key.clone();
                let api_key_header = api_key_header.clone();
                let params = params.clone();
                let method_name = method_name.clone();
                async move {
                    let body = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": "chrononode",
                        "method": method_name,
                        "params": params,
                    });

                    let mut req = client
                        .post(&rpc_url)
                        .header(reqwest::header::CONTENT_TYPE, "application/json")
                        .json(&body);
                    if let Some(user) = username.as_deref() {
                        req = req.basic_auth(user, password.as_deref());
                    }
                    if let Some(key) = api_key.as_deref() {
                        req = req.header(&api_key_header, key);
                    }

                    let resp = req.send().await.map_err(|e| {
                        FetchError::Retryable(format!("POST {} failed: {}", display_url, e))
                    })?;

                    if Self::is_retryable_status(resp.status()) {
                        return Err(FetchError::Retryable(format!(
                            "POST {} returned {} (retryable)",
                            display_url,
                            resp.status()
                        )));
                    }
                    if !resp.status().is_success() {
                        return Err(FetchError::Fatal(format!(
                            "POST {} returned {}",
                            display_url,
                            resp.status()
                        )));
                    }

                    let payload: JsonRpcResponse<T> = resp
                        .json()
                        .await
                        .map_err(|e| FetchError::Fatal(format!("JSON parse failed: {}", e)))?;

                    if let Some(result) = payload.result {
                        Ok(result)
                    } else {
                        Err(FetchError::Fatal(format!(
                            "RPC {} error: {:?}",
                            method_name, payload.error
                        )))
                    }
                }
            },
            |e: &FetchError| matches!(e, FetchError::Retryable(_)),
        )
        .await
        .map_err(Into::into)
    }

    fn rpc_value_to_sats(value: &serde_json::Value) -> u64 {
        let btc = if let Some(f) = value.as_f64() {
            f
        } else if let Some(s) = value.as_str() {
            s.parse::<f64>().unwrap_or(0.0)
        } else if let Some(i) = value.as_i64() {
            i as f64
        } else if let Some(u) = value.as_u64() {
            u as f64
        } else {
            0.0
        };

        if btc <= 0.0 || !btc.is_finite() {
            0
        } else {
            (btc * 100_000_000.0).round() as u64
        }
    }

    fn parse_block_rpc(&self, block: &BitcoinRpcBlock) -> ChronoBlock {
        let transactions: Vec<ChronoTx> = block
            .tx
            .iter()
            .map(|tx| {
                let sender = if let Some(first_in) = tx.vin.first() {
                    if first_in.coinbase.is_some() {
                        b"coinbase".to_vec()
                    } else if let Some(txid) = &first_in.txid {
                        format!("{}:{}", txid, first_in.vout.unwrap_or(0)).into_bytes()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                let recipient = if let Some(first_out) = tx.vout.first() {
                    if let Some(addr) = &first_out.script_pub_key.address {
                        addr.as_bytes().to_vec()
                    } else if let Some(addrs) = &first_out.script_pub_key.addresses {
                        addrs
                            .first()
                            .map(|a| a.as_bytes().to_vec())
                            .unwrap_or_default()
                    } else {
                        Self::decode_hex_safe(first_out.script_pub_key.hex.as_deref().unwrap_or(""))
                    }
                } else {
                    vec![]
                };

                let total_sats: u64 = tx
                    .vout
                    .iter()
                    .map(|o| Self::rpc_value_to_sats(&o.value))
                    .sum();

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

    fn parse_block_esplora(&self, block: &BlockstreamBlock, txs: &[BlockstreamTx]) -> ChronoBlock {
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

    async fn fetch_block_by_hash_str_esplora(&self, hash_hex: &str) -> Result<ChronoBlock> {
        let block_json = self.get(&format!("/api/block/{}", hash_hex)).await?;
        let block: BlockstreamBlock = serde_json::from_value(block_json)
            .map_err(|e| CoreError::Adapter(format!("failed to parse block: {}", e)))?;
        let txs = self.fetch_block_txs(hash_hex).await?;
        Ok(self.parse_block_esplora(&block, &txs))
    }
}

#[async_trait]
impl ChainAdapter for BitcoinLightAdapter {
    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn display_name(&self) -> &str {
        match self.mode {
            BitcoinProviderMode::Esplora => "Bitcoin Light (Esplora)",
            BitcoinProviderMode::JsonRpc => "Bitcoin Light (JSON-RPC)",
        }
    }

    fn block_model(&self) -> BlockModel {
        BlockModel::Utxo
    }

    async fn latest_height(&self) -> Result<u64> {
        match self.mode {
            BitcoinProviderMode::Esplora => {
                let text = self.get_text("/api/blocks/tip/height").await?;
                text.trim()
                    .parse::<u64>()
                    .map_err(|e| CoreError::Adapter(format!("invalid tip height response: {}", e)))
            }
            BitcoinProviderMode::JsonRpc => {
                self.rpc_call("getblockcount", serde_json::json!([])).await
            }
        }
    }

    async fn fetch_block(&self, height: u64) -> Result<ChronoBlock> {
        match self.mode {
            BitcoinProviderMode::Esplora => {
                let hash = self
                    .get_text(&format!("/api/block-height/{}", height))
                    .await?;
                let hash = hash.trim().to_string();
                self.fetch_block_by_hash_str_esplora(&hash).await
            }
            BitcoinProviderMode::JsonRpc => {
                let hash: String = self
                    .rpc_call("getblockhash", serde_json::json!([height]))
                    .await?;
                let block: BitcoinRpcBlock = self
                    .rpc_call("getblock", serde_json::json!([hash, 2]))
                    .await?;
                Ok(self.parse_block_rpc(&block))
            }
        }
    }

    async fn fetch_block_by_hash(&self, hash: &[u8]) -> Result<ChronoBlock> {
        let hash_hex = hex::encode(hash);
        match self.mode {
            BitcoinProviderMode::Esplora => self.fetch_block_by_hash_str_esplora(&hash_hex).await,
            BitcoinProviderMode::JsonRpc => {
                let block: BitcoinRpcBlock = self
                    .rpc_call("getblock", serde_json::json!([hash_hex, 2]))
                    .await?;
                Ok(self.parse_block_rpc(&block))
            }
        }
    }
}

pub fn init() {
    chrononode_adapter_sdk::registry::register(
        "bitcoin-light",
        "Bitcoin Light (Blockstream)",
        |config| {
            let mode = config
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("esplora");

            if mode.eq_ignore_ascii_case("rpc") || mode.eq_ignore_ascii_case("json-rpc") {
                let rpc_url = config
                    .get("rpc_url")
                    .and_then(|v| v.as_str())
                    .or_else(|| config.get("api_url").and_then(|v| v.as_str()))
                    .ok_or_else(|| "bitcoin-light adapter mode=rpc requires rpc_url".to_string())?;

                let rpc_username = config
                    .get("rpc_username")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| std::env::var("CHRONONODE_BTC_RPC_USERNAME").ok())
                    .or_else(|| std::env::var("CHRONONODE_BITCOIN_LIGHT_RPC_USERNAME").ok());
                let rpc_password = config
                    .get("rpc_password")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| std::env::var("CHRONONODE_BTC_RPC_PASSWORD").ok())
                    .or_else(|| std::env::var("CHRONONODE_BITCOIN_LIGHT_RPC_PASSWORD").ok());
                let rpc_api_key = config
                    .get("rpc_api_key")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| std::env::var("CHRONONODE_BTC_RPC_API_KEY").ok())
                    .or_else(|| std::env::var("CHRONONODE_BITCOIN_LIGHT_RPC_API_KEY").ok());
                let rpc_api_key_header = config
                    .get("rpc_api_key_header")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| std::env::var("CHRONONODE_BTC_RPC_API_KEY_HEADER").ok())
                    .or_else(|| std::env::var("CHRONONODE_BITCOIN_LIGHT_RPC_API_KEY_HEADER").ok());

                return Ok(Arc::new(BitcoinLightAdapter::new_rpc(
                    rpc_url,
                    rpc_username,
                    rpc_password,
                    rpc_api_key,
                    rpc_api_key_header,
                )));
            }

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

            Ok(Arc::new(BitcoinLightAdapter::new_with_fallbacks(api_urls)))
        },
    );
}

// ── AddressEvidenceAdapter (Esplora) ──────────────────────────────────────

#[async_trait]
impl AddressEvidenceAdapter for BitcoinLightAdapter {
    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    async fn get_address_summary(&self, address: &str) -> Result<AddressSummary> {
        let data = self.get(&format!("/address/{}", address)).await?;

        let funded = data["chain_stats"]["funded_txo_sum"].as_u64().unwrap_or(0);
        let spent = data["chain_stats"]["spent_txo_sum"].as_u64().unwrap_or(0);
        let balance = funded.saturating_sub(spent);
        let tx_count = data["chain_stats"]["tx_count"].as_u64().unwrap_or(0);

        Ok(AddressSummary {
            address: address.to_string(),
            chain_id: self.chain_id.clone(),
            balance_satoshis: balance,
            tx_count,
            unconfirmed_tx_count: data["mempool_stats"]["tx_count"].as_u64().unwrap_or(0),
            last_seen_timestamp: None,
            last_seen_block: None,
            last_txid: None,
            appears_dormant: balance > 0 && tx_count > 0 && spent == 0,
        })
    }

    async fn get_address_txs(&self, address: &str, limit: usize) -> Result<Vec<AddressTx>> {
        let data = self.get(&format!("/address/{}/txs", address)).await?;
        let txs = data.as_array().ok_or_else(|| CoreError::Adapter("expected array".into()))?;

        let mut result = Vec::new();
        for tx in txs.iter().take(limit) {
            let vin_sum: i64 = tx["vin"].as_array().map(|vins| {
                vins.iter().filter_map(|vin| vin["prevout"]["value"].as_u64()).sum::<u64>() as i64
            }).unwrap_or(0);

            let vout_sum: i64 = tx["vout"].as_array().map(|vouts| {
                vouts.iter().filter_map(|vout| vout["value"].as_u64()).sum::<u64>() as i64
            }).unwrap_or(0);

            let is_outgoing = tx["vin"].as_array().map(|vins| {
                vins.iter().any(|vin| vin["prevout"]["scriptpubkey_address"].as_str() == Some(address))
            }).unwrap_or(false);

            let mut peers = Vec::new();
            for vin in tx["vin"].as_array().unwrap_or(&vec![]) {
                if let Some(addr) = vin["prevout"]["scriptpubkey_address"].as_str() {
                    peers.push(addr.to_string());
                }
            }
            for vout in tx["vout"].as_array().unwrap_or(&vec![]) {
                if let Some(addr) = vout["scriptpubkey_address"].as_str() {
                    peers.push(addr.to_string());
                }
            }

            result.push(AddressTx {
                txid: tx["txid"].as_str().unwrap_or("").to_string(),
                block_height: tx["status"]["block_height"].as_u64(),
                timestamp: tx["status"]["block_time"].as_u64(),
                confirmed: tx["status"]["confirmed"].as_bool().unwrap_or(false),
                value_satoshis: if is_outgoing { -vin_sum } else { vout_sum },
                peers,
            });
        }
        Ok(result)
    }

    async fn get_activity_after(&self, address: &str, after_txid: &str) -> Result<Option<AddressLastActivity>> {
        let data = self.get(&format!("/address/{}/txs/chain/{}", address, after_txid)).await?;
        let txs = data.as_array().ok_or_else(|| CoreError::Adapter("expected array".into()))?;

        if txs.is_empty() {
            return Ok(None);
        }
        let last = &txs[0];
        let block_time = last["status"]["block_time"].as_u64();
        let block_height = last["status"]["block_height"].as_u64();
        let current = self.current_height().await?;

        Ok(Some(AddressLastActivity {
            address: address.to_string(),
            chain_id: self.chain_id.clone(),
            last_txid: last["txid"].as_str().map(|s| s.to_string()),
            last_seen_block: block_height,
            last_seen_timestamp: block_time,
            dormancy_seconds: 0,
            current_height: current,
            is_dormant: true,
        }))
    }

    async fn get_last_activity(&self, address: &str) -> Result<Option<AddressLastActivity>> {
        let txs = self.get_address_txs(address, 1).await?;
        if txs.is_empty() { return Ok(None); }
        let tx = &txs[0];
        let current = self.current_height().await?;
        Ok(Some(AddressLastActivity {
            address: address.to_string(),
            chain_id: self.chain_id.clone(),
            last_txid: Some(tx.txid.clone()),
            last_seen_block: tx.block_height,
            last_seen_timestamp: tx.timestamp,
            dormancy_seconds: 0,
            current_height: current,
            is_dormant: true,
        }))
    }

    async fn verify_transfer_tx(&self, txid: &str, expected_to: &str) -> Result<AddressTransferEvidence> {
        let data = self.get(&format!("/tx/{}", txid)).await?;

        let confirmed = data["status"]["confirmed"].as_bool().unwrap_or(false);
        let block_height = data["status"]["block_height"].as_u64();
        let block_time = data["status"]["block_time"].as_u64();

        let mut from_addr = String::new();
        let mut to_addrs = Vec::new();
        let mut amount_sats: u64 = 0;

        for vin in data["vin"].as_array().unwrap_or(&vec![]) {
            if let Some(addr) = vin["prevout"]["scriptpubkey_address"].as_str() {
                if from_addr.is_empty() { from_addr = addr.to_string(); }
            }
        }
        for vout in data["vout"].as_array().unwrap_or(&vec![]) {
            if let Some(addr) = vout["scriptpubkey_address"].as_str() {
                to_addrs.push(addr.to_string());
                amount_sats += vout["value"].as_u64().unwrap_or(0);
            }
        }

        Ok(AddressTransferEvidence {
            txid: txid.to_string(),
            from_address: from_addr,
            to_address: to_addrs.join(","),
            amount_satoshis: amount_sats,
            block_height,
            timestamp: block_time,
            confirmed,
            matched_expected_to: to_addrs.contains(&expected_to.to_string()),
        })
    }

    async fn current_height(&self) -> Result<u64> {
        let data = self.get("/blocks/tip/height").await?;
        data.as_u64().ok_or_else(|| CoreError::Adapter("invalid tip height".into()))
    }

    async fn get_merkle_proof(&self, txid: &str) -> Result<TxMerkleProof> {
        let data = self.get(&format!("/tx/{}/merkle-proof", txid)).await?;
        Ok(TxMerkleProof {
            txid: txid.to_string(),
            block_height: data["block_height"].as_u64().unwrap_or(0),
            block_hash: data["block_hash"].as_str().unwrap_or("").to_string(),
            merkle_branch: data["merkle"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            tx_index_in_block: data["pos"].as_u64().unwrap_or(0),
        })
    }

    async fn get_utxos(&self, address: &str) -> Result<Vec<UtxoEntry>> {
        let data = self.get(&format!("/address/{}/utxo", address)).await?;
        let utxos = data.as_array().ok_or_else(|| CoreError::Adapter("expected array".into()))?;
        utxos.iter().map(|u| {
            Ok(UtxoEntry {
                txid: u["txid"].as_str().unwrap_or("").to_string(),
                vout: u["vout"].as_u64().unwrap_or(0) as u32,
                value_satoshis: u["value"].as_u64().unwrap_or(0),
                block_height: u["status"]["block_height"].as_u64(),
                status: UtxoStatus {
                    confirmed: u["status"]["confirmed"].as_bool().unwrap_or(false),
                    block_height: u["status"]["block_height"].as_u64(),
                },
            })
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_url_hides_getblock_path_token() {
        let redacted = BitcoinLightAdapter::redact_url("https://go.getblock.io/secret123");
        assert!(redacted.contains("go.getblock.io/[redacted]"));
        assert!(!redacted.contains("secret123"));
    }

    #[test]
    fn test_rpc_value_to_sats() {
        assert_eq!(
            BitcoinLightAdapter::rpc_value_to_sats(&serde_json::json!(0.1)),
            10_000_000
        );
        assert_eq!(
            BitcoinLightAdapter::rpc_value_to_sats(&serde_json::json!("0.00000001")),
            1
        );
    }
}
