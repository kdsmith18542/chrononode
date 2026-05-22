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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepairConfig {
    pub policy: RepairPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    pub checkpoint_size: u64,
    pub hash_algorithm: String,
    pub repair: RepairConfig,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            checkpoint_size: 1000,
            hash_algorithm: "sha256".to_string(),
            repair: RepairConfig::default(),
        }
    }
}
