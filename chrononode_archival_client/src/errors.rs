//! Error types for the ChronoNode Archival Client

use std::fmt;
use thiserror::Error;

/// Main error type for blockchain synchronization
#[derive(Error, Debug)]
pub enum BlockchainSyncError {
    /// Error from the underlying database
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    /// Error from the blockchain RPC client
    #[error("RPC error: {0}")]
    RpcError(String),
    
    /// Error serializing or deserializing data
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    /// Block not found
    #[error("Block not found: {0}")]
    BlockNotFound(String),
    
    /// Transaction not found
    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),
    
    /// Invalid block data
    #[error("Invalid block: {0}")]
    InvalidBlock(String),
    
    /// Network connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    /// Verification failed
    #[error("Verification failed: {0}")]
    VerificationError(String),
    
    /// Other errors
    #[error("Error: {0}")]
    Other(String),
}

impl From<rocksdb::Error> for BlockchainSyncError {
    fn from(err: rocksdb::Error) -> Self {
        BlockchainSyncError::DatabaseError(err.to_string())
    }
}

impl From<std::io::Error> for BlockchainSyncError {
    fn from(err: std::io::Error) -> Self {
        BlockchainSyncError::Other(err.to_string())
    }
}

impl From<bincode::Error> for BlockchainSyncError {
    fn from(err: bincode::Error) -> Self {
        BlockchainSyncError::SerializationError(err.to_string())
    }
}

/// Result type for blockchain operations
pub type BlockchainSyncResult<T> = Result<T, BlockchainSyncError>;
