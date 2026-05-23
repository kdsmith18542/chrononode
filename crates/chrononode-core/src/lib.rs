pub mod block;
pub mod chain;
pub mod config;
pub mod error;
pub mod proof;
pub mod signing;

pub use block::{ChronoBlock, ChronoEvent, ChronoTx};
pub use chain::{BlockModel, ChainAdapter, StorageBackend, StorageHealth, StoragePointer};
pub use config::{CoreConfig, RepairPolicy, PruningConfig, PruningMode};
pub use error::CoreError;
pub use proof::MerkleLeaf;
pub use signing::{verify_signature, OperatorKeypair};

pub type Result<T> = std::result::Result<T, CoreError>;
