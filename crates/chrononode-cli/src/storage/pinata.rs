use async_trait::async_trait;
use chrononode_core::{CoreError, Result, StorageBackend, StorageHealth, StoragePointer};
use reqwest::multipart;
use sha2::{Digest, Sha256};
use std::time::Instant;

pub struct PinataBackend {
    api_base: String,
    gateway_base: String,
    jwt: Option<String>,
    client: reqwest::Client,
}

impl PinataBackend {
    pub fn new(api_base: &str, gateway_base: &str, jwt: Option<String>) -> Self {
        Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            gateway_base: gateway_base.trim_end_matches('/').to_string(),
            jwt,
            client: reqwest::Client::new(),
        }
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
        StoragePointer::new("pinata", format!("{}:{}", Self::hash_hex(bytes), cid))
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
                    "Pinata content hash mismatch for pointer {}: expected {} got {}",
                    pointer.key, expected_hash, got
                )));
            }
        }
        Ok(())
    }

    fn bearer(&self) -> Result<&str> {
        self.jwt.as_deref().ok_or_else(|| {
            CoreError::Storage("Pinata JWT missing; set CHRONONODE_PINATA_JWT".to_string())
        })
    }
}

#[async_trait]
impl StorageBackend for PinataBackend {
    async fn put(&self, bytes: &[u8]) -> Result<StoragePointer> {
        let jwt = self.bearer()?;
        let form = multipart::Form::new().part(
            "file",
            multipart::Part::bytes(bytes.to_vec()).file_name("chronoblock.bin"),
        );
        let response = self
            .client
            .post(format!("{}/pinning/pinFileToIPFS", self.api_base))
            .header("Authorization", format!("Bearer {}", jwt))
            .multipart(form)
            .send()
            .await
            .map_err(|e| CoreError::Storage(format!("Pinata upload request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(CoreError::Storage(format!(
                "Pinata upload failed with {}: {}",
                status, body
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| CoreError::Storage(format!("Pinata upload parse failed: {}", e)))?;
        let cid = value
            .get("IpfsHash")
            .and_then(|v| v.as_str())
            .or_else(|| value.get("cid").and_then(|v| v.as_str()))
            .or_else(|| {
                value
                    .get("data")
                    .and_then(|d| d.get("cid"))
                    .and_then(|v| v.as_str())
            })
            .ok_or_else(|| CoreError::Storage("Pinata upload response missing CID".to_string()))?;
        Ok(Self::pointer_for(bytes, cid))
    }

    async fn get(&self, pointer: &StoragePointer) -> Result<Vec<u8>> {
        let cid = Self::cid_from_pointer(pointer);
        let mut request = self
            .client
            .get(format!("{}/ipfs/{}", self.gateway_base, cid));
        if let Some(jwt) = self.jwt.as_deref() {
            request = request.header("Authorization", format!("Bearer {}", jwt));
        }
        let response = request
            .send()
            .await
            .map_err(|e| CoreError::Storage(format!("Pinata gateway request failed: {}", e)))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(CoreError::Storage(format!(
                "Pinata gateway fetch failed with {} for {}: {}",
                status, cid, body
            )));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| CoreError::Storage(format!("Pinata gateway body read failed: {}", e)))?
            .to_vec();
        Self::verify_bytes(pointer, &bytes)?;
        Ok(bytes)
    }

    async fn pin(&self, pointer: &StoragePointer) -> Result<()> {
        let jwt = self.bearer()?;
        let cid = Self::cid_from_pointer(pointer);
        let body = serde_json::json!({ "hashToPin": cid });
        let response = self
            .client
            .post(format!("{}/pinning/pinByHash", self.api_base))
            .header("Authorization", format!("Bearer {}", jwt))
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Storage(format!("Pinata pin request failed: {}", e)))?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(CoreError::Storage(format!(
                "Pinata pin failed with {} for {}: {}",
                status, cid, text
            )));
        }
        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealth> {
        let Some(jwt) = self.jwt.as_deref() else {
            return Ok(StorageHealth {
                healthy: false,
                latency_ms: 0,
                message: "Pinata JWT missing; set CHRONONODE_PINATA_JWT".to_string(),
            });
        };
        let start = Instant::now();
        let response = self
            .client
            .get(format!("{}/data/testAuthentication", self.api_base))
            .header("Authorization", format!("Bearer {}", jwt))
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
                message: format!("Pinata responded with {}", resp.status()),
            }),
            Err(err) => Ok(StorageHealth {
                healthy: false,
                latency_ms: elapsed,
                message: format!("Pinata request failed: {}", err),
            }),
        }
    }
}
