use serde::{Deserialize, Serialize};

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
pub struct CoreConfig {
    pub checkpoint_size: u64,
    pub hash_algorithm: String,
    pub repair: RepairConfig,
    #[serde(default)]
    pub pruning: PruningConfig,
    #[serde(default)]
    pub compression: bool,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            checkpoint_size: 1000,
            hash_algorithm: "sha256".to_string(),
            repair: RepairConfig::default(),
            pruning: PruningConfig::default(),
            compression: true,
        }
    }
}
