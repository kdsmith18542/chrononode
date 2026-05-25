use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairPolicy {
    Skip,
    #[default]
    Refetch,
    RefetchAndReplace,
}

impl RepairPolicy {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "skip" => Self::Skip,
            "refetch-and-replace" => Self::RefetchAndReplace,
            _ => Self::Refetch,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PruningMode {
    #[default]
    None,
    Height,
    Age,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PruningConfig {
    pub mode: PruningMode,
    pub keep_blocks: u64,
    pub keep_duration_secs: u64,
    pub prune_utxos: bool,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            mode: PruningMode::None,
            keep_blocks: 1000,
            keep_duration_secs: 2592000, // 30 days
            prune_utxos: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepairConfig {
    pub policy: RepairPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DormancyConfig {
    /// Default threshold in blocks before an address is considered dormant
    #[serde(default = "default_dormancy_threshold")]
    pub default_threshold_blocks: u64,
    /// Per-chain override thresholds, e.g.  {"bitcoin": 26280, "dogecoin": 2103840}
    #[serde(default)]
    pub chain_thresholds: HashMap<String, u64>,
}

fn default_dormancy_threshold() -> u64 {
    26280 // ~5 years at 10-min blocks (BTC default)
}

impl Default for DormancyConfig {
    fn default() -> Self {
        Self {
            default_threshold_blocks: default_dormancy_threshold(),
            chain_thresholds: HashMap::new(),
        }
    }
}

impl DormancyConfig {
    pub fn threshold_for(&self, chain_id: &str) -> u64 {
        self.chain_thresholds
            .get(chain_id)
            .copied()
            .unwrap_or(self.default_threshold_blocks)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationConfig {
    /// BaaLS API URL for submitting attestation transactions
    pub baals_api_url: Option<String>,
    /// Path to the BaaLS account signing key file (32 bytes raw ed25519 seed)
    pub baals_key_path: Option<String>,
    /// Skip TLS certificate verification for BaaLS connections (for self-signed certs on localhost)
    #[serde(default)]
    pub baals_tls_skip_verify: bool,
    /// Auto-submit attestations when new dormant addresses are detected
    #[serde(default = "default_auto_submit")]
    pub auto_submit: bool,
    /// EVM RPC URL for submitting dormancy proofs to Resurgence contract
    pub evm_rpc_url: Option<String>,
    /// Resurgence RewardDistributor contract address
    pub evm_contract_address: Option<String>,
    /// Gas limit for EVM submission (default: 1_000_000)
    #[serde(default = "default_evm_gas_limit")]
    pub evm_gas_limit: u64,
    /// Hex-encoded secp256k1 private key for signing EVM transactions (no 0x prefix)
    pub evm_private_key: Option<String>,
    /// EVM chain ID — defaults to 421614 (Arbitrum Sepolia)
    pub evm_chain_id: Option<u64>,
}

fn default_auto_submit() -> bool {
    true
}

fn default_evm_gas_limit() -> u64 {
    1_000_000
}

impl Default for AttestationConfig {
    fn default() -> Self {
        Self {
            baals_api_url: None,
            baals_key_path: None,
            baals_tls_skip_verify: false,
            auto_submit: true,
            evm_rpc_url: None,
            evm_contract_address: None,
            evm_gas_limit: default_evm_gas_limit(),
            evm_private_key: None,
            evm_chain_id: None,
        }
    }
}

fn default_checkpoint_size() -> u64 {
    1000
}
fn default_hash_algorithm() -> String {
    "sha256".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    #[serde(default = "default_checkpoint_size")]
    pub checkpoint_size: u64,
    #[serde(default = "default_hash_algorithm")]
    pub hash_algorithm: String,
    #[serde(default)]
    pub repair: RepairConfig,
    #[serde(default)]
    pub pruning: PruningConfig,
    #[serde(default)]
    pub compression: bool,
    #[serde(default)]
    pub dormancy: DormancyConfig,
    #[serde(default)]
    pub attestation: AttestationConfig,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            checkpoint_size: 1000,
            hash_algorithm: "sha256".to_string(),
            repair: RepairConfig::default(),
            pruning: PruningConfig::default(),
            compression: true,
            dormancy: DormancyConfig::default(),
            attestation: AttestationConfig::default(),
        }
    }
}
