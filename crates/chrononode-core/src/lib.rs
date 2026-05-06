pub mod block;
pub mod chain;
pub mod config;
pub mod error;
pub mod proof;

pub use block::{ChronoBlock, ChronoEvent, ChronoTx};
pub use chain::{BlockModel, ChainAdapter, StorageBackend, StoragePointer, StorageHealth};
pub use config::CoreConfig;
pub use error::CoreError;

pub type Result<T> = std::result::Result<T, CoreError>;
