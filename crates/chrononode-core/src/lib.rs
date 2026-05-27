pub mod block;
pub mod chain;
pub mod config;
pub mod dormancy;
pub mod error;
pub mod proof;
pub mod signing;
pub mod zkvm;
pub mod evidence;
pub mod adapters;
pub mod policy;
pub mod address_evidence;

pub use block::{ChronoBlock, ChronoEvent, ChronoTx};
pub use chain::{BlockModel, ChainAdapter, StorageBackend, StorageHealth, StoragePointer, ChainEvidenceAdapter, TransferEvidence, AddressActivity, DormancyEvidenceRequest, DormancyEvidence};
pub use config::{
    AttestationConfig, CoreConfig, DormancyConfig, PruningConfig, PruningMode, RepairPolicy,
};
pub use dormancy::{DormancyProof, DormancyStatus};
pub use error::CoreError;
pub use proof::MerkleLeaf;
pub use signing::{verify_signature, OperatorKeypair};
pub use evidence::{DefaultChainEvidenceAdapter, TxLookupBackend};

pub type Result<T> = std::result::Result<T, CoreError>;
