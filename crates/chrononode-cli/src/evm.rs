use chrononode_core::{CoreConfig, DormancyProof, Result};
use k256::ecdsa::{RecoveryId, Signature, SigningKey};
use sha3::{Digest, Keccak256};
use std::sync::OnceLock;

const DEFAULT_CHAIN_ID: u64 = 421614; // Arbitrum Sepolia

pub struct EvmSubmitter {
    rpc_url: String,
    contract_address: String,
    gas_limit: u64,
    chain_id: u64,
    enabled: bool,
    private_key_bytes: Option<Vec<u8>>,
}

static SELECTOR: OnceLock<[u8; 4]> = OnceLock::new();

fn dormancy_proof_selector() -> [u8; 4] {
    *SELECTOR.get_or_init(|| {
        let hash = Keccak256::new()
            .chain_update(
                b"submitDormancyProof(bytes32,address,uint256,uint256,uint256,bytes32,bytes)",
            )
            .finalize();
        [hash[0], hash[1], hash[2], hash[3]]
    })
}

impl EvmSubmitter {
    pub fn new(config: &CoreConfig) -> Self {
        let private_key_bytes = config
            .attestation
            .evm_private_key
            .as_deref()
            .and_then(|k| hex::decode(k.strip_prefix("0x").unwrap_or(k)).ok());

        let enabled = config.attestation.evm_rpc_url.is_some()
            && config.attestation.evm_contract_address.is_some()
            && private_key_bytes.is_some();

        Self {
            rpc_url: config
                .attestation
                .evm_rpc_url
                .clone()
                .unwrap_or_else(|| "http://localhost:8545".to_string()),
            contract_address: config
                .attestation
                .evm_contract_address
                .clone()
                .unwrap_or_default(),
            gas_limit: config.attestation.evm_gas_limit,
            chain_id: config
                .attestation
                .evm_chain_id
                .unwrap_or(DEFAULT_CHAIN_ID),
            enabled,
            private_key_bytes,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn operator_address(&self) -> Option<String> {
        let key_bytes = self.private_key_bytes.as_ref()?;
        let signing_key = SigningKey::from_slice(key_bytes).ok()?;
        Some(derive_address(&signing_key))
    }

    pub async fn submit_proof(&self, proof: &DormancyProof) -> Result<String> {
        if !self.enabled {
            return Err(chrononode_core::CoreError::Internal(
                "EVM submitter not configured".to_string(),
            ));
        }

        let key_bytes = self.private_key_bytes.as_ref().unwrap();
        let signing_key = SigningKey::from_slice(key_bytes).map_err(|e| {
            chrononode_core::CoreError::Internal(format!("Invalid EVM private key: {}", e))
        })?;
        let sender = derive_address(&signing_key);

        let call_data = self.encode_dormancy_proof(proof);
        let client = reqwest::Client::new();

        let nonce = self.get_nonce(&client, &sender).await?;
        let gas_price = self.get_gas_price(&client).await?;

        let to_bytes = hex::decode(
            self.contract_address
                .strip_prefix("0x")
                .unwrap_or(&self.contract_address),
        )
        .map_err(|e| {
            chrononode_core::CoreError::Internal(format!("contract address decode: {}", e))
        })?;

        let raw_tx =
            self.build_signed_tx(&signing_key, nonce, gas_price, &to_bytes, &call_data)?;

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendRawTransaction",
            "params": [format!("0x{}", hex::encode(&raw_tx))],
            "id": 1,
        });

        let resp = client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| chrononode_core::CoreError::Adapter(format!("EVM RPC error: {}", e)))?;

        let json: serde_json::Value = resp.json().await.map_err(|e| {
            chrononode_core::CoreError::Adapter(format!("EVM response parse error: {}", e))
        })?;

        let tx_hash = json["result"]
            .as_str()
            .ok_or_else(|| {
                chrononode_core::CoreError::Adapter(format!(
                    "EVM RPC error result: {:?}",
                    json["error"]
                ))
            })?
            .to_string();

        Ok(tx_hash)
    }

    fn build_signed_tx(
        &self,
        signing_key: &SigningKey,
        nonce: u64,
        gas_price: u64,
        to: &[u8],
        data: &[u8],
    ) -> Result<Vec<u8>> {
        // EIP-155 signing payload: rlp([nonce, gasPrice, gasLimit, to, value, data, chainId, 0, 0])
        let signing_payload = rlp_list([
            rlp_uint(nonce),
            rlp_uint(gas_price),
            rlp_uint(self.gas_limit),
            rlp_bytes(to),
            rlp_uint(0u64), // value
            rlp_bytes(data),
            rlp_uint(self.chain_id),
            rlp_uint(0u64),
            rlp_uint(0u64),
        ]);

        let hash = Keccak256::digest(&signing_payload);

        let (sig, recid): (Signature, RecoveryId) = signing_key
            .sign_prehash_recoverable(&hash)
            .map_err(|e| {
                chrononode_core::CoreError::Internal(format!("EVM signing error: {}", e))
            })?;

        let sig_bytes = sig.to_bytes();
        let r = &sig_bytes[..32];
        let s = &sig_bytes[32..];
        let v = recid.to_byte() as u64 + self.chain_id * 2 + 35;

        let raw_tx = rlp_list([
            rlp_uint(nonce),
            rlp_uint(gas_price),
            rlp_uint(self.gas_limit),
            rlp_bytes(to),
            rlp_uint(0u64),
            rlp_bytes(data),
            rlp_uint(v),
            rlp_bigint(r),
            rlp_bigint(s),
        ]);

        Ok(raw_tx)
    }

    async fn get_nonce(&self, client: &reqwest::Client, address: &str) -> Result<u64> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionCount",
            "params": [address, "latest"],
            "id": 1,
        });
        let resp: serde_json::Value = client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| chrononode_core::CoreError::Adapter(format!("nonce RPC error: {}", e)))?
            .json()
            .await
            .map_err(|e| {
                chrononode_core::CoreError::Adapter(format!("nonce parse error: {}", e))
            })?;
        let hex_nonce = resp["result"].as_str().ok_or_else(|| {
            chrononode_core::CoreError::Adapter(format!("nonce error: {:?}", resp["error"]))
        })?;
        u64::from_str_radix(hex_nonce.strip_prefix("0x").unwrap_or(hex_nonce), 16).map_err(|e| {
            chrononode_core::CoreError::Adapter(format!("nonce hex parse: {}", e))
        })
    }

    async fn get_gas_price(&self, client: &reqwest::Client) -> Result<u64> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_gasPrice",
            "params": [],
            "id": 1,
        });
        let resp: serde_json::Value = client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                chrononode_core::CoreError::Adapter(format!("gasPrice RPC error: {}", e))
            })?
            .json()
            .await
            .map_err(|e| {
                chrononode_core::CoreError::Adapter(format!("gasPrice parse error: {}", e))
            })?;
        let hex_price = resp["result"].as_str().ok_or_else(|| {
            chrononode_core::CoreError::Adapter(format!("gasPrice error: {:?}", resp["error"]))
        })?;
        u64::from_str_radix(hex_price.strip_prefix("0x").unwrap_or(hex_price), 16).map_err(|e| {
            chrononode_core::CoreError::Adapter(format!("gasPrice hex parse: {}", e))
        })
    }

    fn encode_dormancy_proof(&self, proof: &DormancyProof) -> Vec<u8> {
        let chain_id = Self::pad_bytes32(proof.chain_id.as_bytes());
        let wallet = Self::pad_address(&proof.address);
        let signer_pubkey = Self::hex_or_zero32(&proof.signer_pubkey);
        let signature = Self::hex_bytes_or_empty(&proof.signature);

        let mut data = Vec::new();

        data.extend_from_slice(&dormancy_proof_selector());
        data.extend_from_slice(&chain_id);
        data.extend_from_slice(&wallet);
        Self::push_uint256(&mut data, proof.dormant_since_block);
        Self::push_uint256(&mut data, proof.current_block);
        Self::push_uint256(&mut data, proof.threshold_blocks);
        data.extend_from_slice(&signer_pubkey);

        // Dynamic: uint256 offset to signature data (7 * 32 = 224 bytes)
        Self::push_uint256(&mut data, 224);

        // Signature: uint256 length + bytes (padded to 32-byte boundary)
        Self::push_uint256(&mut data, signature.len() as u64);
        data.extend_from_slice(&signature);
        while data.len() % 32 != 0 {
            data.push(0);
        }

        data
    }

    fn pad_bytes32(input: &[u8]) -> [u8; 32] {
        let mut buf = [0u8; 32];
        let copy_len = input.len().min(32);
        buf[..copy_len].copy_from_slice(&input[..copy_len]);
        buf
    }

    fn pad_address(address: &str) -> [u8; 32] {
        let addr_hex = address.strip_prefix("0x").unwrap_or(address);
        let addr_bytes = hex::decode(addr_hex).unwrap_or_else(|_| {
            let mut hasher = Keccak256::new();
            hasher.update(address.as_bytes());
            let hash = hasher.finalize();
            hash[..20].to_vec()
        });

        let mut buf = [0u8; 32];
        if addr_bytes.len() >= 20 {
            buf[12..32].copy_from_slice(&addr_bytes[addr_bytes.len() - 20..]);
        } else {
            let offset = 32 - addr_bytes.len();
            buf[offset..32].copy_from_slice(&addr_bytes);
        }
        buf
    }

    fn push_uint256(data: &mut Vec<u8>, value: u64) {
        let be_bytes = value.to_be_bytes();
        let pad = [0u8; 24];
        data.extend_from_slice(&pad);
        data.extend_from_slice(&be_bytes);
    }

    fn hex_or_zero32(hex_opt: &Option<String>) -> [u8; 32] {
        match hex_opt {
            Some(h) => {
                let mut buf = [0u8; 32];
                if let Ok(bytes) = hex::decode(h) {
                    let copy_len = bytes.len().min(32);
                    buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
                }
                buf
            }
            None => [0u8; 32],
        }
    }

    fn hex_bytes_or_empty(hex_opt: &Option<String>) -> Vec<u8> {
        match hex_opt {
            Some(h) => hex::decode(h).unwrap_or_default(),
            None => vec![],
        }
    }
}

fn derive_address(signing_key: &SigningKey) -> String {
    let verifying_key = signing_key.verifying_key();
    let encoded = verifying_key.to_encoded_point(false); // uncompressed: 04 || x || y
    let pk_bytes = &encoded.as_bytes()[1..]; // skip 0x04 prefix → 64 bytes
    let hash = Keccak256::digest(pk_bytes);
    format!("0x{}", hex::encode(&hash[12..]))
}

// ── Minimal RLP encoding ────────────────────────────────────────────────────

fn rlp_uint(n: u64) -> Vec<u8> {
    if n == 0 {
        return vec![0x80];
    }
    let be = n.to_be_bytes();
    let skip = be.iter().take_while(|&&b| b == 0).count();
    rlp_bytes(&be[skip..])
}

// Big integer: strip leading zeros before encoding (for r, s signature values)
fn rlp_bigint(data: &[u8]) -> Vec<u8> {
    let skip = data.iter().take_while(|&&b| b == 0).count();
    rlp_bytes(&data[skip..])
}

fn rlp_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        return data.to_vec();
    }
    let mut out = Vec::new();
    if data.len() <= 55 {
        out.push(0x80 + data.len() as u8);
    } else {
        let len_enc = rlp_length_bytes(data.len());
        out.push(0xb7 + len_enc.len() as u8);
        out.extend_from_slice(&len_enc);
    }
    out.extend_from_slice(data);
    out
}

fn rlp_list(items: impl IntoIterator<Item = Vec<u8>>) -> Vec<u8> {
    let payload: Vec<u8> = items.into_iter().flatten().collect();
    let mut out = Vec::new();
    if payload.len() <= 55 {
        out.push(0xc0 + payload.len() as u8);
    } else {
        let len_enc = rlp_length_bytes(payload.len());
        out.push(0xf7 + len_enc.len() as u8);
        out.extend_from_slice(&len_enc);
    }
    out.extend_from_slice(&payload);
    out
}

fn rlp_length_bytes(len: usize) -> Vec<u8> {
    let be = len.to_be_bytes();
    let skip = be.iter().take_while(|&&b| b == 0).count();
    be[skip..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrononode_core::{CoreConfig, DormancyProof};
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_logging() {
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt().with_env_filter("off").try_init();
        });
    }

    fn test_proof() -> DormancyProof {
        DormancyProof {
            version: "chrononode:dormancy:v1".to_string(),
            chain_id: "bitcoin".to_string(),
            address: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".to_string(),
            dormant_since_block: 500_000,
            current_block: 526_280,
            threshold_blocks: 26_280,
            signer_pubkey: Some("ab".repeat(32)),
            signature: Some("cd".repeat(64)),
        }
    }

    fn config_with_key() -> CoreConfig {
        let mut config = CoreConfig::default();
        config.attestation.evm_rpc_url = Some("http://localhost:8545".to_string());
        config.attestation.evm_contract_address =
            Some("0x1234567890123456789012345678901234567890".to_string());
        // well-known test private key (not used on mainnet)
        config.attestation.evm_private_key = Some(
            "4c0883a69102937d6231471b5dbb6e538eba2ef4ac23d7a843dfd94820ee0b59".to_string(),
        );
        config
    }

    #[test]
    fn test_function_selector() {
        assert_ne!(dormancy_proof_selector(), [0u8; 4]);
        assert!(dormancy_proof_selector().iter().any(|b| *b != 0));
    }

    #[test]
    fn test_pad_bytes32() {
        let result = EvmSubmitter::pad_bytes32(b"bitcoin");
        assert_eq!(&result[..7], b"bitcoin");
        assert!(result[7..].iter().all(|b| *b == 0));
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_push_uint256() {
        let mut data = Vec::new();
        EvmSubmitter::push_uint256(&mut data, 500_000);
        assert_eq!(data.len(), 32);
        assert_eq!(data[..24], [0u8; 24]);
        assert_eq!(data[24..32], 500_000u64.to_be_bytes());
    }

    #[test]
    fn test_abi_encode_has_correct_length() {
        let config = config_with_key();
        let submitter = EvmSubmitter::new(&config);
        let proof = test_proof();

        let data = submitter.encode_dormancy_proof(&proof);

        assert!(data.len() > 4 + 7 * 32, "minimum ABI-encoded size");
        assert_eq!(data.len() % 32, 0, "ABI data must be 32-byte aligned");
        assert_eq!(
            &data[..4],
            &dormancy_proof_selector(),
            "starts with function selector"
        );
    }

    #[test]
    fn test_abi_encode_contains_chain_id() {
        let config = config_with_key();
        let submitter = EvmSubmitter::new(&config);
        let proof = test_proof();

        let data = submitter.encode_dormancy_proof(&proof);
        let chain_id_slot = &data[4..36];
        assert_eq!(&chain_id_slot[..7], b"bitcoin");
    }

    #[test]
    fn test_abi_encodes_dormant_since_block() {
        let config = config_with_key();
        let submitter = EvmSubmitter::new(&config);
        let proof = test_proof();

        let data = submitter.encode_dormancy_proof(&proof);
        // dormantSinceBlock slot: after selector(4) + chainId(32) + wallet(32) = offset 68
        let slot = &data[68..100];
        assert_eq!(slot[24..32], 500_000u64.to_be_bytes());
    }

    #[test]
    fn test_new_not_enabled_by_default() {
        let config = CoreConfig::default();
        let submitter = EvmSubmitter::new(&config);
        assert!(!submitter.is_enabled());
    }

    #[test]
    fn test_new_enabled_with_full_config() {
        let config = config_with_key();
        let submitter = EvmSubmitter::new(&config);
        assert!(submitter.is_enabled());
    }

    #[test]
    fn test_not_enabled_missing_private_key() {
        let mut config = CoreConfig::default();
        config.attestation.evm_rpc_url = Some("http://localhost:8545".to_string());
        config.attestation.evm_contract_address =
            Some("0x1234567890123456789012345678901234567890".to_string());
        let submitter = EvmSubmitter::new(&config);
        assert!(!submitter.is_enabled());
    }

    #[test]
    fn test_operator_address_derivation() {
        let config = config_with_key();
        let submitter = EvmSubmitter::new(&config);
        let addr = submitter.operator_address().unwrap();
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42);
    }

    #[test]
    fn test_rlp_uint_zero() {
        assert_eq!(rlp_uint(0), vec![0x80]);
    }

    #[test]
    fn test_rlp_uint_small() {
        // 0x01 < 0x80 → single byte
        assert_eq!(rlp_uint(1), vec![0x01]);
    }

    #[test]
    fn test_rlp_uint_large() {
        // 500000 = 0x07A120 → 3 bytes → 0x83 prefix
        let enc = rlp_uint(500_000);
        assert_eq!(enc[0], 0x83);
        assert_eq!(enc.len(), 4);
    }

    #[test]
    fn test_build_signed_tx_structure() {
        let config = config_with_key();
        let submitter = EvmSubmitter::new(&config);
        let key_bytes = submitter.private_key_bytes.as_ref().unwrap();
        let signing_key = SigningKey::from_slice(key_bytes).unwrap();
        let to = hex::decode("1234567890123456789012345678901234567890").unwrap();
        let data = b"test calldata";

        let raw_tx = submitter
            .build_signed_tx(&signing_key, 0, 1_000_000_000, &to, data)
            .unwrap();

        // Must start with RLP list prefix
        assert!(raw_tx[0] >= 0xc0);
        // Must be non-empty
        assert!(raw_tx.len() > 50);
    }

    #[tokio::test]
    async fn test_submit_proof_when_not_enabled() {
        init_logging();
        let config = CoreConfig::default();
        let submitter = EvmSubmitter::new(&config);
        let proof = test_proof();

        let result = submitter.submit_proof(&proof).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not configured"));
    }

    #[tokio::test]
    async fn test_submit_proof_success() {
        init_logging();
        let mut mock_server = mockito::Server::new_async().await;

        // nonce response
        let _mock_nonce = mock_server
            .mock("POST", "/")
            .match_body(mockito::Matcher::PartialJsonString(
                r#"{"method":"eth_getTransactionCount"}"#.to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"jsonrpc":"2.0","result":"0x0","id":1}"#)
            .create_async()
            .await;

        // gasPrice response
        let _mock_gas = mock_server
            .mock("POST", "/")
            .match_body(mockito::Matcher::PartialJsonString(
                r#"{"method":"eth_gasPrice"}"#.to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"jsonrpc":"2.0","result":"0x3b9aca00","id":1}"#)
            .create_async()
            .await;

        // sendRawTransaction response
        let mock_send = mock_server
            .mock("POST", "/")
            .match_body(mockito::Matcher::PartialJsonString(
                r#"{"method":"eth_sendRawTransaction"}"#.to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"jsonrpc":"2.0","result":"0xabcd1234","id":1}"#)
            .create_async()
            .await;

        let mut config = config_with_key();
        config.attestation.evm_rpc_url = Some(mock_server.url());

        let submitter = EvmSubmitter::new(&config);
        assert!(submitter.is_enabled());

        let proof = test_proof();
        let result = submitter.submit_proof(&proof).await.unwrap();
        assert_eq!(result, "0xabcd1234");

        mock_send.assert_async().await;
    }

    #[tokio::test]
    async fn test_submit_proof_rpc_error() {
        init_logging();
        let mut mock_server = mockito::Server::new_async().await;

        let _mock_nonce = mock_server
            .mock("POST", "/")
            .match_body(mockito::Matcher::PartialJsonString(
                r#"{"method":"eth_getTransactionCount"}"#.to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"jsonrpc":"2.0","result":"0x1","id":1}"#)
            .create_async()
            .await;

        let _mock_gas = mock_server
            .mock("POST", "/")
            .match_body(mockito::Matcher::PartialJsonString(
                r#"{"method":"eth_gasPrice"}"#.to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"jsonrpc":"2.0","result":"0x3b9aca00","id":1}"#)
            .create_async()
            .await;

        let mock_send = mock_server
            .mock("POST", "/")
            .match_body(mockito::Matcher::PartialJsonString(
                r#"{"method":"eth_sendRawTransaction"}"#.to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"jsonrpc":"2.0","error":{"code":-32000,"message":"execution reverted"},"id":1}"#,
            )
            .create_async()
            .await;

        let mut config = config_with_key();
        config.attestation.evm_rpc_url = Some(mock_server.url());

        let submitter = EvmSubmitter::new(&config);
        let proof = test_proof();

        let result = submitter.submit_proof(&proof).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("execution reverted"));

        mock_send.assert_async().await;
    }
}
