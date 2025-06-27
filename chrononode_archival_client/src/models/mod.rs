//! Core data structures for ChronoNode's blockchain data model.
//! 
//! This module defines the fundamental data structures used throughout the ChronoNode
//! system for representing blockchain data in a chain-agnostic way.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a blockchain-agnostic block header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Block height
    pub height: u64,
    /// Block hash
    pub hash: String,
    /// Previous block hash
    pub prev_hash: String,
    /// Block timestamp in seconds since epoch
    pub timestamp: u64,
    /// Chain-specific additional fields
    pub extra: HashMap<String, String>,
}

/// Represents a blockchain transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Transaction hash/ID
    pub txid: String,
    /// Block hash this transaction is included in
    pub block_hash: String,
    /// Transaction index in the block
    pub index: u32,
    /// Sender address (if applicable)
    pub from: Option<String>,
    /// Recipient address (if applicable)
    pub to: Option<String>,
    /// Transaction value/amount
    pub value: String,
    /// Transaction fee
    pub fee: String,
    /// Transaction status (confirmed, pending, etc.)
    pub status: TransactionStatus,
    /// Chain-specific transaction data
    pub extra: HashMap<String, String>,
}

/// Transaction status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Confirmed(u64), // Confirmed with number of confirmations
    Failed(String), // Failed with reason
}

/// Represents a state change in the blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    /// Type of state change
    pub change_type: StateChangeType,
    /// Block height when the change occurred
    pub block_height: u64,
    /// Transaction that triggered this change (if any)
    pub txid: Option<String>,
    /// Address affected by this change
    pub address: String,
    /// Previous state (serialized as JSON string)
    pub previous_state: Option<String>,
    /// New state (serialized as JSON string)
    pub new_state: String,
}

/// Types of state changes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StateChangeType {
    /// Account balance update
    BalanceUpdate,
    /// Smart contract storage change
    StorageUpdate,
    /// Code deployment
    CodeUpdate,
    /// Custom state change
    Custom(String),
}

/// Represents a blockchain event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainEvent {
    /// Event type (e.g., "NewBlock", "NewTransaction")
    pub event_type: String,
    /// Block height this event is associated with
    pub block_height: u64,
    /// Transaction hash this event is associated with (if any)
    pub txid: Option<String>,
    /// Event data (serialized as JSON string)
    pub data: String,
}

/// Trait for converting chain-specific types to our common data model
pub trait ToCommonType<T> {
    /// Convert to the common type
    fn to_common(&self) -> T;
}

/// Error type for model conversions
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] bincode::Error),
    
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

// Implementations for common conversions

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_block_header_serialization() {
        let mut extra = HashMap::new();
        extra.insert("difficulty".to_string(), "12345".to_string());
        
        let header = BlockHeader {
            height: 12345,
            hash: "0000000000000000000abc123".to_string(),
            prev_hash: "0000000000000000000def456".to_string(),
            timestamp: 1630000000,
            extra,
        };
        
        let serialized = serde_json::to_string(&header).unwrap();
        let deserialized: BlockHeader = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(header.height, deserialized.height);
        assert_eq!(header.hash, deserialized.hash);
    }
    
    #[test]
    fn test_transaction_status_serialization() {
        let pending = TransactionStatus::Pending;
        let confirmed = TransactionStatus::Confirmed(6);
        let failed = TransactionStatus::Failed("Out of gas".to_string());
        
        assert_eq!(
            serde_json::to_string(&pending).unwrap(),
            "\"Pending\""
        );
        
        assert_eq!(
            serde_json::to_string(&confirmed).unwrap(),
            "{\"Confirmed\":6}"
        );
        
        assert_eq!(
            serde_json::to_string(&failed).unwrap(),
            "{\"Failed\":\"Out of gas\"}"
        );
    }
}
