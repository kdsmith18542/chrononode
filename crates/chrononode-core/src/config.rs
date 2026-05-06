use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    pub checkpoint_size: u64,
    pub hash_algorithm: String,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            checkpoint_size: 1000,
            hash_algorithm: "sha256".to_string(),
        }
    }
}
