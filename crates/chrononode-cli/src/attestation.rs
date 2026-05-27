use chrononode_core::{CoreConfig, DormancyProof, Result};

pub struct BaalsSubmitter {
    client: reqwest::Client,
    api_url: String,
    signing_key: Option<ed25519_dalek::SigningKey>,
}

impl BaalsSubmitter {
    pub fn new(config: &CoreConfig) -> Self {
        let api_url = config
            .attestation
            .baals_api_url
            .clone()
            .unwrap_or_else(|| "http://localhost:18080".to_string());

        let signing_key = config.attestation.baals_key_path.as_ref().and_then(|path| {
            let bytes = std::fs::read(path).ok()?;
            let seed: [u8; 32] = bytes.try_into().ok()?;
            Some(ed25519_dalek::SigningKey::from_bytes(&seed))
        });

        let mut builder = reqwest::Client::builder().user_agent("chrononode/0.1");
        if config.attestation.baals_tls_skip_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }
        Self {
            client: builder.build().unwrap_or_default(),
            api_url: api_url.trim_end_matches('/').to_string(),
            signing_key,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.signing_key.is_some()
    }

    pub async fn submit_dormancy_proof(
        &self,
        proof: &DormancyProof,
        index: &dyn crate::index::IndexBackend,
    ) -> Result<Option<String>> {
        let chain_id = &proof.chain_id;
        let address = &proof.address;
        let dormant_since = proof.dormant_since_block;

        if index
            .attestation_exists(chain_id, address, dormant_since)
            .await?
        {
            tracing::info!(
                "Attestation already exists for {} on {} dormant since block {}",
                address,
                chain_id,
                dormant_since
            );
            return Ok(None);
        }

        let mut signed_proof = proof.clone();
        if signed_proof.signature.is_none() {
            let key = self.signing_key.as_ref().ok_or_else(|| {
                chrononode_core::CoreError::Internal("BaaLS signing key not configured".to_string())
            })?;
            let keypair = chrononode_core::signing::OperatorKeypair::from_seed(&key.to_bytes());
            signed_proof.sign(&keypair);
        }

        let url = format!("{}/api/v1/oracle/attest", self.api_url);

        let resp = self
            .client
            .post(&url)
            .json(&signed_proof)
            .send()
            .await
            .map_err(|e| {
                chrononode_core::CoreError::Adapter(format!("BaaLS submit failed: {}", e))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            index
                .record_attestation(chain_id, address, dormant_since, None, "failed")
                .await?;
            return Err(chrononode_core::CoreError::Adapter(format!(
                "BaaLS submit returned {}: {}",
                status, text
            )));
        }

        let response: serde_json::Value = resp.json().await.map_err(|e| {
            chrononode_core::CoreError::Adapter(format!("BaaLS response parse failed: {}", e))
        })?;

        let attestation = response.get("attestation").ok_or_else(|| {
            chrononode_core::CoreError::Adapter(
                "BaaLS response missing attestation field".to_string(),
            )
        })?;

        let baals_sig = attestation
            .get("baals_signature")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                chrononode_core::CoreError::Adapter(
                    "BaaLS attestation missing baals_signature".to_string(),
                )
            })?
            .to_string();

        index
            .record_attestation(
                chain_id,
                address,
                dormant_since,
                Some(&baals_sig),
                "submitted",
            )
            .await?;

        tracing::info!(
            "Submitted dormancy attestation for {} on {} (baals_sig: {})",
            address,
            chain_id,
            baals_sig
        );

        Ok(Some(baals_sig))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{self, IndexBackend};
    use chrononode_core::{CoreConfig, DormancyProof};
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_logging() {
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt().with_env_filter("off").try_init();
        });
    }

    async fn test_index() -> (Box<dyn IndexBackend>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let index = index::open_index(index::IndexKind::Sqlite, &db_path, "")
            .await
            .unwrap();
        (index, dir)
    }

    fn test_config() -> CoreConfig {
        CoreConfig::default()
    }

    fn test_proof() -> DormancyProof {
        DormancyProof {
            version: "chrononode:dormancy:v1".to_string(),
            chain_id: "bitcoin".to_string(),
            address: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".to_string(),
            dormant_since_block: 500_000,
            current_block: 526_280,
            threshold_blocks: 26_280,
            signer_pubkey: None,
            signature: None,
            evm_wallet: None,
            proof_type: "ed25519".to_string(),
            zk_proof: None,
            public_inputs: None,
            confidence_tier: 1,
        }
    }

    #[test]
    fn test_new_with_no_config() {
        let config = CoreConfig::default();
        let submitter = BaalsSubmitter::new(&config);
        assert!(!submitter.is_configured());
    }

    #[test]
    fn test_new_with_key_file() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("baals.key");
        let keypair = chrononode_core::OperatorKeypair::generate();
        let seed = keypair.signing_key_bytes();
        std::fs::write(&key_path, seed).unwrap();

        let mut config = CoreConfig::default();
        config.attestation.baals_key_path = Some(key_path.to_string_lossy().to_string());

        let submitter = BaalsSubmitter::new(&config);
        assert!(submitter.is_configured());
    }

    #[tokio::test]
    async fn test_submit_skips_when_attestation_exists() {
        init_logging();
        let config = test_config();
        let submitter = BaalsSubmitter::new(&config);
        let (index, _dir) = test_index().await;
        let proof = test_proof();

        index
            .record_attestation(
                &proof.chain_id,
                &proof.address,
                proof.dormant_since_block,
                Some("fake_tx"),
                "submitted",
            )
            .await
            .unwrap();

        let result = submitter
            .submit_dormancy_proof(&proof, index.as_ref())
            .await
            .unwrap();
        assert!(
            result.is_none(),
            "should skip when attestation already exists"
        );
    }

    #[tokio::test]
    async fn test_submit_fails_without_key() {
        init_logging();
        let config = test_config();
        let submitter = BaalsSubmitter::new(&config);
        let (index, _dir) = test_index().await;
        let proof = test_proof();

        let result = submitter
            .submit_dormancy_proof(&proof, index.as_ref())
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("signing key not configured"));
    }

    #[tokio::test]
    async fn test_submit_success() {
        init_logging();
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("baals.key");
        let keypair = chrononode_core::OperatorKeypair::generate();
        let seed = keypair.signing_key_bytes();
        std::fs::write(&key_path, seed).unwrap();

        let mut mock_server = mockito::Server::new_async().await;
        let mock = mock_server
            .mock("POST", "/api/v1/oracle/attest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"ok","attestation":{"baals_signature":"abc123def456"}}"#)
            .create_async()
            .await;

        let mut config = CoreConfig::default();
        config.attestation.baals_api_url = Some(mock_server.url());
        config.attestation.baals_key_path = Some(key_path.to_string_lossy().to_string());

        let submitter = BaalsSubmitter::new(&config);
        assert!(submitter.is_configured());

        let (index, _dir) = test_index().await;
        let proof = test_proof();

        let result = submitter
            .submit_dormancy_proof(&proof, index.as_ref())
            .await
            .unwrap();
        assert_eq!(result, Some("abc123def456".to_string()));

        mock.assert_async().await;

        let recorded = index.list_attestations(&proof.chain_id).await.unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, proof.address);
    }

    #[tokio::test]
    async fn test_submit_server_error() {
        init_logging();
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("baals.key");
        let keypair = chrononode_core::OperatorKeypair::generate();
        let seed = keypair.signing_key_bytes();
        std::fs::write(&key_path, seed).unwrap();

        let mut mock_server = mockito::Server::new_async().await;
        let mock = mock_server
            .mock("POST", "/api/v1/oracle/attest")
            .with_status(500)
            .with_body("internal error")
            .create_async()
            .await;

        let mut config = CoreConfig::default();
        config.attestation.baals_api_url = Some(mock_server.url());
        config.attestation.baals_key_path = Some(key_path.to_string_lossy().to_string());

        let submitter = BaalsSubmitter::new(&config);
        let (index, _dir) = test_index().await;
        let proof = test_proof();

        let result = submitter
            .submit_dormancy_proof(&proof, index.as_ref())
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("500"));

        mock.assert_async().await;

        let recorded = index.list_attestations(&proof.chain_id).await.unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].3, "failed");
    }
}
