use async_trait::async_trait;
use chrononode_core::{CoreError, Result, StorageBackend, StorageHealth, StoragePointer};
use reqwest::multipart;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::Instant;

pub struct IpfsBackend {
    api_url: String,
    client: reqwest::Client,
}

impl IpfsBackend {
    pub fn new(api_url: &str) -> Self {
        Self {
            api_url: api_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.api_url, path.trim_start_matches('/'))
    }

    fn split_pointer_key(key: &str) -> (Option<&str>, &str) {
        match key.split_once(':') {
            Some((sha256_hex, cid)) if sha256_hex.len() == 64 && !cid.is_empty() => {
                (Some(sha256_hex), cid)
            }
            _ => (None, key),
        }
    }

    fn hash_hex(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn pointer_for(bytes: &[u8], cid: &str) -> StoragePointer {
        StoragePointer::new("ipfs", format!("{}:{}", Self::hash_hex(bytes), cid))
    }

    fn cid_from_pointer(pointer: &StoragePointer) -> &str {
        let (_maybe_hash, cid) = Self::split_pointer_key(&pointer.key);
        cid
    }

    fn verify_bytes(pointer: &StoragePointer, bytes: &[u8]) -> Result<()> {
        let (expected_hash, _cid) = Self::split_pointer_key(&pointer.key);
        if let Some(expected_hash) = expected_hash {
            let got = Self::hash_hex(bytes);
            if got != expected_hash {
                return Err(CoreError::Storage(format!(
                    "IPFS content hash mismatch for pointer {}: expected {} got {}",
                    pointer.key, expected_hash, got
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct IpfsAddResponse {
    #[serde(rename = "Hash")]
    hash: String,
}

#[async_trait]
impl StorageBackend for IpfsBackend {
    async fn put(&self, bytes: &[u8]) -> Result<StoragePointer> {
        let form = multipart::Form::new().part(
            "file",
            multipart::Part::bytes(bytes.to_vec()).file_name("chronoblock.bin"),
        );
        let response = self
            .client
            .post(self.endpoint("/api/v0/add?pin=true"))
            .multipart(form)
            .send()
            .await
            .map_err(|e| CoreError::Storage(format!("IPFS add request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(CoreError::Storage(format!(
                "IPFS add failed with {}: {}",
                status, body
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| CoreError::Storage(format!("IPFS add response read failed: {}", e)))?;
        let line = body
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .ok_or_else(|| CoreError::Storage("IPFS add returned empty body".to_string()))?;
        let parsed: IpfsAddResponse = serde_json::from_str(line)
            .map_err(|e| CoreError::Storage(format!("IPFS add parse failed: {}", e)))?;
        if parsed.hash.is_empty() {
            return Err(CoreError::Storage(
                "IPFS add returned empty CID".to_string(),
            ));
        }

        Ok(Self::pointer_for(bytes, &parsed.hash))
    }

    async fn get(&self, pointer: &StoragePointer) -> Result<Vec<u8>> {
        let cid = Self::cid_from_pointer(pointer);
        let url = format!("{}/api/v0/cat?arg={}", self.api_url, cid);
        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| CoreError::Storage(format!("IPFS cat request failed: {}", e)))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(CoreError::Storage(format!(
                "IPFS cat failed with {} for {}: {}",
                status, cid, body
            )));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| CoreError::Storage(format!("IPFS cat body read failed: {}", e)))?
            .to_vec();
        Self::verify_bytes(pointer, &bytes)?;
        Ok(bytes)
    }

    async fn pin(&self, pointer: &StoragePointer) -> Result<()> {
        let cid = Self::cid_from_pointer(pointer);
        let url = format!("{}/api/v0/pin/add?arg={}", self.api_url, cid);
        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| CoreError::Storage(format!("IPFS pin request failed: {}", e)))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(CoreError::Storage(format!(
                "IPFS pin failed with {} for {}: {}",
                status, cid, body
            )));
        }
        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealth> {
        let start = Instant::now();
        let response = self
            .client
            .post(self.endpoint("/api/v0/version"))
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
                message: format!("IPFS responded with {}", resp.status()),
            }),
            Err(err) => Ok(StorageHealth {
                healthy: false,
                latency_ms: elapsed,
                message: format!("IPFS request failed: {}", err),
            }),
        }
    }
}
