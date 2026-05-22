use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use chrononode_core::proof::{
    verify_proof_json, CheckpointJson, ProofJson, ProofSiblingJson,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    pub chain_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockResponse {
    pub chain_id: String,
    pub height: u64,
    pub block_hash: String,
    pub timestamp: u64,
    pub tx_count: usize,
    pub event_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointResponse {
    pub checkpoint_id: String,
    pub chain_id: String,
    pub start_height: i64,
    pub end_height: i64,
    pub root_hash: String,
    pub signer_pubkey: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofResponse {
    pub proof: ProofJson,
}

#[derive(Debug, Error)]
pub enum SdkError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error response (status {status}): {message}")]
    Api { status: reqwest::StatusCode, message: String },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct ChronoNodeClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl ChronoNodeClient {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    fn apply_headers(&self, mut req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref key) = self.api_key {
            req = req.header("X-API-Key", key);
        }
        req
    }

    async fn check_status(&self, res: reqwest::Response) -> Result<reqwest::Response, SdkError> {
        let status = res.status();
        if !status.is_success() {
            let message = res.text().await.unwrap_or_default();
            return Err(SdkError::Api { status, message });
        }
        Ok(res)
    }

    pub async fn health(&self) -> Result<HealthResponse, SdkError> {
        let url = format!("{}/health", self.base_url);
        let req = self.client.get(&url);
        let res = self.apply_headers(req).send().await?;
        let res = self.check_status(res).await?;
        Ok(res.json().await?)
    }

    pub async fn list_chains(&self) -> Result<Vec<ChainInfo>, SdkError> {
        let url = format!("{}/v1/chains", self.base_url);
        let req = self.client.get(&url);
        let res = self.apply_headers(req).send().await?;
        let res = self.check_status(res).await?;
        Ok(res.json().await?)
    }

    pub async fn get_block_by_height(&self, chain_id: &str, height: u64) -> Result<BlockResponse, SdkError> {
        let url = format!("{}/v1/chains/{}/blocks/{}", self.base_url, chain_id, height);
        let req = self.client.get(&url);
        let res = self.apply_headers(req).send().await?;
        let res = self.check_status(res).await?;
        Ok(res.json().await?)
    }

    pub async fn get_block_by_hash(&self, chain_id: &str, hash: &str) -> Result<BlockResponse, SdkError> {
        let url = format!("{}/v1/chains/{}/blocks/hash/{}", self.base_url, chain_id, hash);
        let req = self.client.get(&url);
        let res = self.apply_headers(req).send().await?;
        let res = self.check_status(res).await?;
        Ok(res.json().await?)
    }

    pub async fn get_block_range(&self, chain_id: &str, from: u64, to: u64, format: Option<&str>) -> Result<Vec<serde_json::Value>, SdkError> {
        let mut url = format!("{}/v1/chains/{}/blocks?from={}&to={}", self.base_url, chain_id, from, to);
        if let Some(fmt) = format {
            url.push_str(&format!("&format={}", fmt));
        }
        let req = self.client.get(&url);
        let res = self.apply_headers(req).send().await?;
        let res = self.check_status(res).await?;
        let text = res.text().await?;
        if format == Some("ndjson") {
            let mut results = Vec::new();
            for line in text.lines() {
                if !line.trim().is_empty() {
                    results.push(serde_json::from_str(line)?);
                }
            }
            Ok(results)
        } else {
            Ok(serde_json::from_str(&text)?)
        }
    }

    pub async fn get_block_proof(&self, chain_id: &str, height: u64) -> Result<ProofJson, SdkError> {
        let url = format!("{}/v1/chains/{}/proofs/block/{}", self.base_url, chain_id, height);
        let req = self.client.get(&url);
        let res = self.apply_headers(req).send().await?;
        let res = self.check_status(res).await?;
        let resp: ProofResponse = res.json().await?;
        Ok(resp.proof)
    }

    pub async fn get_checkpoint(&self, checkpoint_id: &str) -> Result<CheckpointResponse, SdkError> {
        let url = format!("{}/v1/checkpoints/{}", self.base_url, checkpoint_id);
        let req = self.client.get(&url);
        let res = self.apply_headers(req).send().await?;
        let res = self.check_status(res).await?;
        Ok(res.json().await?)
    }

    pub async fn verify_proof_api(&self, proof: &ProofJson) -> Result<bool, SdkError> {
        let url = format!("{}/v1/proofs/verify", self.base_url);
        let body = serde_json::json!({ "proof_json": proof });
        let req = self.client.post(&url).json(&body);
        let res = self.apply_headers(req).send().await?;
        let res = self.check_status(res).await?;
        #[derive(Deserialize)]
        struct VerifyResponse {
            valid: bool,
        }
        let resp: VerifyResponse = res.json().await?;
        Ok(resp.valid)
    }

    pub fn verify_proof_locally(&self, proof: &ProofJson) -> bool {
        verify_proof_json(proof)
    }
}
