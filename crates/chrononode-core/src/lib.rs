pub mod block;
pub mod chain;
pub mod config;
pub mod dormancy;
pub mod error;
pub mod proof;
pub mod signing;
#[cfg(feature = "zkvm")]
pub mod zkvm;

pub use block::{ChronoBlock, ChronoEvent, ChronoTx};
pub use chain::{BlockModel, ChainAdapter, StorageBackend, StorageHealth, StoragePointer};
pub use config::{
    AttestationConfig, CoreConfig, DormancyConfig, PruningConfig, PruningMode, RepairPolicy,
};
pub use dormancy::{DormancyProof, DormancyStatus};
pub use error::CoreError;
pub use proof::MerkleLeaf;
pub use signing::{verify_signature, OperatorKeypair};

pub type Result<T> = std::result::Result<T, CoreError>;
