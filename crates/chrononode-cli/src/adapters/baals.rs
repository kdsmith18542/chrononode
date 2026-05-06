use async_trait::async_trait;
use chrononode_core::{BlockModel, ChainAdapter, ChronoBlock, ChronoEvent, ChronoTx, Result, CoreError};
use serde::{Deserialize, Serialize};

pub struct BaalsAdapter {
    chain_id: String,
    client: reqwest::Client,
    rpc_url: String,
}

impl BaalsAdapter {
    pub fn new(rpc_url: &str) -> Self {
        Self {
            chain_id: "baals".to_string(),
            client: reqwest::Client::new(),
            rpc_url: rpc_url.to_string(),
        }
    }

    async fn rpc_call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let resp = self
            .client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Adapter(format!("RPC request failed: {}", e)))?;
        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Adapter(format!("RPC parse failed: {}", e)))?;
        if let Some(err) = result.get("error") {
            return Err(CoreError::Adapter(format!("RPC error: {}", err)));
        }
        result
            .get("result")
            .cloned()
            .ok_or_else(|| CoreError::Adapter("RPC response missing result".to_string()))
    }

    async fn get_block_rpc(&self, height: u64) -> Result<ChronoBlock> {
        let result = self
            .rpc_call("baals_getBlockByHeight", serde_json::json!([height]))
            .await?;

        let block_hash = hex::decode(
            result
                .get("hash")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
        )
        .unwrap_or_default();
        let prev_hash = hex::decode(
            result
                .get("parentHash")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
        )
        .unwrap_or_default();
        let timestamp = result
            .get("timestamp")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let txs = result
            .get("transactions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|tx| {
                        let tx_hash = hex::decode(
                            tx.get("hash").and_then(|v| v.as_str()).unwrap_or(""),
                        )
                        .unwrap_or_default();
                        let sender = hex::decode(
                            tx.get("from").and_then(|v| v.as_str()).unwrap_or(""),
                        )
                        .unwrap_or_default();
                        let recipient = hex::decode(
                            tx.get("to").and_then(|v| v.as_str()).unwrap_or(""),
                        )
                        .unwrap_or_default();
                        let amount = tx
                            .get("value")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(0);
                        ChronoTx {
                            tx_hash,
                            sender,
                            recipient,
                            amount,
                            nonce: tx.get("nonce").and_then(|v| v.as_u64()).unwrap_or(0),
                            payload: vec![],
                            gas_limit: tx.get("gas").and_then(|v| v.as_u64()).unwrap_or(0),
                            gas_used: tx.get("gasUsed").and_then(|v| v.as_u64()).unwrap_or(0),
                            extra_data: vec![],
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(ChronoBlock {
            schema_version: 1,
            chain_id: self.chain_id.clone(),
            height,
            block_hash,
            prev_hash,
            timestamp,
            block_model: "EventLedger".to_string(),
            hash_algorithm: "sha256".to_string(),
            transactions: txs,
            events: vec![],
            extra_data: vec![],
        })
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
        BlockModel::EventLedger
    }

    async fn latest_height(&self) -> Result<u64> {
        let result = self.rpc_call("baals_blockNumber", serde_json::json!([])).await?;
        let hex = result.as_str().unwrap_or("0x0");
        let without_prefix = hex.trim_start_matches("0x");
        u64::from_str_radix(without_prefix, 16)
            .map_err(|e| CoreError::Adapter(format!("invalid block number: {}", e)))
    }

    async fn fetch_block(&self, height: u64) -> Result<ChronoBlock> {
        self.get_block_rpc(height).await
    }

    async fn fetch_block_by_hash(&self, hash: &[u8]) -> Result<ChronoBlock> {
        let hash_hex = hex::encode(hash);
        let result = self
            .rpc_call("baals_getBlockByHash", serde_json::json!([hash_hex]))
            .await?;
        let height = result
            .get("number")
            .and_then(|v| v.as_str())
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0);
        self.get_block_rpc(height).await
    }
}
