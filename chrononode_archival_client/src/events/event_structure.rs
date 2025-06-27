//! Core event types and structures for the ChronoNode event system.

use crate::models::{BlockHeader, Transaction, StateChange};
use serde::{Serialize, Deserialize};
use std::fmt;

/// Types of blockchain events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EventType {
    /// A new block was added to the chain
    NewBlock,
    /// A new transaction was included in a block
    NewTransaction,
    /// A state change occurred (e.g., contract storage update)
    NewStateChange,
    /// The chain was reorganized (reorg)
    ChainReorg,
    /// Synchronization progress update
    SyncProgress,
    /// An error occurred
    Error,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventType::NewBlock => write!(f, "NewBlock"),
            EventType::NewTransaction => write!(f, "NewTransaction"),
            EventType::NewStateChange => write!(f, "NewStateChange"),
            EventType::ChainReorg => write!(f, "ChainReorg"),
            EventType::SyncProgress => write!(f, "SyncProgress"),
            EventType::Error => write!(f, "Error"),
        }
    }
}

/// Event payload that can contain different types of data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    /// A new block was added to the chain
    Block(BlockHeader),
    /// A new transaction was included in a block
    Transaction(Transaction),
    /// A state change occurred (e.g., contract storage update)
    StateChange(StateChange),
    /// The chain was reorganized (reorg)
    Reorg {
        /// Block number where the reorg occurred
        block_number: u64,
        /// Old block hashes that were removed
        old_blocks: Vec<String>,
        /// New block hashes that were added
        new_blocks: Vec<String>,
    },
    /// Synchronization progress update
    SyncProgress {
        /// Current block number being processed
        current_block: u64,
        /// Highest block number in the chain
        highest_block: u64,
        /// Number of blocks remaining to sync
        blocks_remaining: u64,
    },
    /// An error occurred
    Error {
        /// Error message
        message: String,
        /// Error code (if any)
        code: Option<i32>,
        /// Additional error details
        details: Option<serde_json::Value>,
    },
}

impl EventPayload {
    /// Get the event type for this payload
    pub fn event_type(&self) -> EventType {
        match self {
            EventPayload::Block(_) => EventType::NewBlock,
            EventPayload::Transaction(_) => EventType::NewTransaction,
            EventPayload::StateChange(_) => EventType::NewStateChange,
            EventPayload::Reorg { .. } => EventType::ChainReorg,
            EventPayload::SyncProgress { .. } => EventType::SyncProgress,
            EventPayload::Error { .. } => EventType::Error,
        }
    }
}

/// Event structure that gets published to subscribers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event ID
    pub id: String,
    /// Event type
    pub event_type: EventType,
    /// Source of the event (e.g., "bitcoin", "ethereum", "solana")
    pub source: String,
    /// Timestamp in milliseconds since epoch
    pub timestamp: u64,
    /// Event payload
    pub payload: EventPayload,
}

impl Event {
    /// Create a new event
    pub fn new(event_type: EventType, source: &str, payload: EventPayload) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event_type: event_type.clone(),
            source: source.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            payload,
        }
    }

    /// Create a new block event
    pub fn new_block(block: BlockHeader, source: &str) -> Self {
        Self::new(EventType::NewBlock, source, EventPayload::Block(block))
    }

    /// Create a new transaction event
    pub fn new_transaction(tx: Transaction, source: &str) -> Self {
        Self::new(EventType::NewTransaction, source, EventPayload::Transaction(tx))
    }

    /// Create a new state change event
    pub fn new_state_change(change: StateChange, source: &str) -> Self {
        Self::new(EventType::NewStateChange, source, EventPayload::StateChange(change))
    }

    /// Create a new chain reorg event
    pub fn new_reorg(
        block_number: u64,
        old_blocks: Vec<String>,
        new_blocks: Vec<String>,
        source: &str,
    ) -> Self {
        Self::new(
            EventType::ChainReorg,
            source,
            EventPayload::Reorg {
                block_number,
                old_blocks,
                new_blocks,
            },
        )
    }

    /// Create a new sync progress event
    pub fn new_sync_progress(
        current_block: u64,
        highest_block: u64,
        source: &str,
    ) -> Self {
        let blocks_remaining = highest_block.saturating_sub(current_block);
        Self::new(
            EventType::SyncProgress,
            source,
            EventPayload::SyncProgress {
                current_block,
                highest_block,
                blocks_remaining,
            },
        )
    }

    /// Create a new error event
    pub fn new_error(message: String, code: Option<i32>, details: Option<serde_json::Value>, source: &str) -> Self {
        Self::new(
            EventType::Error,
            source,
            EventPayload::Error {
                message,
                code,
                details,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::BlockHeader;
    use std::str::FromStr;

    #[test]
    fn test_event_creation() {
        let block = BlockHeader {
            hash: "0x1234".to_string(),
            number: 12345,
            parent_hash: "0x1233".to_string(),
            timestamp: 1625097600,
            ..Default::default()
        };

        let event = Event::new_block(block.clone(), "ethereum");
        assert_eq!(event.event_type, EventType::NewBlock);
        assert_eq!(event.source, "ethereum");
        assert!(event.timestamp > 0);

        if let EventPayload::Block(event_block) = event.payload {
            assert_eq!(event_block.hash, block.hash);
        } else {
            panic!("Expected Block payload");
        }
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(EventType::NewBlock.to_string(), "NewBlock");
        assert_eq!(EventType::NewTransaction.to_string(), "NewTransaction");
        assert_eq!(EventType::NewStateChange.to_string(), "NewStateChange");
        assert_eq!(EventType::ChainReorg.to_string(), "ChainReorg");
        assert_eq!(EventType::SyncProgress.to_string(), "SyncProgress");
        assert_eq!(EventType::Error.to_string(), "Error");
    }
}
