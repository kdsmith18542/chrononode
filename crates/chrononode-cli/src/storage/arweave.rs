use async_trait::async_trait;
use chrononode_core::{CoreError, Result, StorageBackend, StorageHealth, StoragePointer};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::time::Instant;

pub struct ArweaveBackend {
    gateway_base: String,
    bundler_url: String,
    client: Client,
}

impl ArweaveBackend {
    pub fn new(gateway_base: &str, bundler_url: &str) -> Self {
        Self {
            gateway_base: gateway_base.trim_end_matches('/').to_string(),
            bundler_url: bundler_url.trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }

    fn split_pointer_key(key: &str) -> (Option<&str>, &str) {
        match key.split_once(':') {
            Some((sha256_hex, txid)) if sha256_hex.len() == 64 && !txid.is_empty() => {
                (Some(sha256_hex), txid)
            }
            _ => (None, key),
        }
    }

    fn hash_hex(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn pointer_for(bytes: &[u8], txid: &str) -> StoragePointer {
        StoragePointer::new("arweave", format!("{}:{}", Self::hash_hex(bytes), txid))
    }

    fn txid_from_pointer(pointer: &StoragePointer) -> &str {
        let (_maybe_hash, txid) = Self::split_pointer_key(&pointer.key);
        txid
    }

    fn verify_bytes(pointer: &StoragePointer, bytes: &[u8]) -> Result<()> {
        let (expected_hash, _txid) = Self::split_pointer_key(&pointer.key);
        if let Some(expected_hash) = expected_hash {
            let got = Self::hash_hex(bytes);
            if got != expected_hash {
                return Err(CoreError::Storage(format!(
                    "Arweave content hash mismatch for pointer {}: expected {} got {}",
                    pointer.key, expected_hash, got
                )));
            }
        }
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for ArweaveBackend {
    async fn put(&self, bytes: &[u8]) -> Result<StoragePointer> {
        let url = format!("{}/tx", self.bundler_url);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/octet-stream")
            .body(bytes.to_vec())
            .send()
            .await
            .map_err(|e| {
                CoreError::Storage(format!("Arweave bundler upload request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(CoreError::Storage(format!(
                "Arweave bundler upload failed with {}: {}",
                status, body
            )));
        }

        let body: serde_json::Value = response.json().await.map_err(|e| {
            CoreError::Storage(format!("Arweave bundler response parse failed: {}", e))
        })?;

        let txid = body["id"]
            .as_str()
            .or_else(|| body["tx_id"].as_str())
            .or_else(|| body["transactionId"].as_str())
            .ok_or_else(|| {
                CoreError::Storage(format!("Arweave bundler response missing tx id: {}", body))
            })?;

        Ok(Self::pointer_for(bytes, txid))
    }

    async fn get(&self, pointer: &StoragePointer) -> Result<Vec<u8>> {
        let txid = Self::txid_from_pointer(pointer);
        let url = format!("{}/{}", self.gateway_base, txid);
        let response =
            self.client.get(&url).send().await.map_err(|e| {
                CoreError::Storage(format!("Arweave gateway request failed: {}", e))
            })?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(CoreError::Storage(format!(
                "Arweave gateway fetch failed with {} for {}: {}",
                status, txid, body
            )));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| CoreError::Storage(format!("Arweave gateway body read failed: {}", e)))?
            .to_vec();
        Self::verify_bytes(pointer, &bytes)?;
        Ok(bytes)
    }

    async fn pin(&self, _pointer: &StoragePointer) -> Result<()> {
        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealth> {
        let start = Instant::now();
        let response = self
            .client
            .get(format!("{}/info", self.gateway_base))
            .send()
            .await;
        let elapsed = start.elapsed().as_millis() as u64;
        match response {
            Ok(resp) if resp.status().is_success() => Ok(StorageHealth {
                healthy: true,
                latency_ms: elapsed,
                message: "OK".to_string(),
            }),
            Ok(resp) => Ok(StorageHealth {
                healthy: false,
                latency_ms: elapsed,
                message: format!("Arweave responded with {}", resp.status()),
            }),
            Err(err) => Ok(StorageHealth {
                healthy: false,
                latency_ms: elapsed,
                message: format!("Arweave request failed: {}", err),
            }),
        }
    }
}
