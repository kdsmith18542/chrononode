//! Blockchain data validation logic for ChronoNode.
//! 
//! This module provides validation functions for blocks, transactions, and other
//! blockchain data structures to ensure data integrity and consensus compliance.

use crate::models::{BlockHeader, Transaction, StateChange, ModelError};
use anyhow::{Context, Result};
use sha2::{Sha256, Sha512, Digest};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

/// Validates a block header against basic consensus rules
pub fn validate_block_header(header: &BlockHeader) -> Result<()> {
    // Check timestamp is not in the future (with 2-hour grace period for clock skew)
    let max_future_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ModelError::InvalidData("System time is before Unix epoch".into()))?
        .as_secs() + 7200; // 2 hours in seconds
    
    if header.timestamp > max_future_time {
        return Err(ModelError::InvalidData(
            format!("Block timestamp {} is too far in the future", header.timestamp)
        ).into());
    }

    // Check block hash format (basic validation, chain-specific validation happens in chain modules)
    if header.hash.is_empty() {
        return Err(ModelError::InvalidData("Block hash cannot be empty".into()).into());
    }

    // Check previous hash format
    if header.prev_hash.is_empty() && header.height != 0 {
        return Err(ModelError::InvalidData(
            "Non-genesis block must have a previous block hash".into()
        ).into());
    }

    // Additional chain-specific validations can be added via the extra fields
    // These would be implemented in the specific chain validation modules

    Ok(())
}

/// Validates a transaction against basic rules
pub fn validate_transaction(tx: &Transaction) -> Result<()> {
    // Basic transaction ID validation
    if tx.txid.is_empty() {
        return Err(ModelError::InvalidData("Transaction ID cannot be empty".into()).into());
    }

    // Block hash must be set for confirmed transactions
    if tx.status != crate::models::TransactionStatus::Pending && tx.block_hash.is_empty() {
        return Err(ModelError::InvalidData(
            "Confirmed transaction must have a block hash".into()
        ).into());
    }

    // Basic value validation
    if let Err(e) = tx.value.parse::<u128>() {
        return Err(ModelError::InvalidData(
            format!("Invalid transaction value: {}", e)
        ).into());
    }

    // Basic fee validation
    if let Err(e) = tx.fee.parse::<u128>() {
        return Err(ModelError::InvalidData(
            format!("Invalid transaction fee: {}", e)
        ).into());
    }

    Ok(())
}

/// Validates a state change
pub fn validate_state_change(change: &StateChange) -> Result<()> {
    // Address validation
    if change.address.is_empty() {
        return Err(ModelError::InvalidData("State change address cannot be empty".into()).into());
    }

    // New state must be set
    if change.new_state.is_empty() {
        return Err(ModelError::InvalidData("New state cannot be empty".into()).into());
    }

    // If this is an update (not creation), previous state must be set
    if change.change_type != crate::models::StateChangeType::CodeUpdate && 
       change.previous_state.is_none() {
        return Err(ModelError::InvalidData(
            "State update must have previous state".into()
        ).into());
    }

    Ok(())
}

/// Validates a cryptographic hash against an expected format
pub fn validate_hash(hash: &str, expected_length: usize, algorithm: &str) -> bool {
    if hash.len() != expected_length * 2 { // *2 because hex encoding
        return false;
    }

    // Check if all characters are valid hex
    if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }

    // Additional algorithm-specific validation could be added here
    match algorithm.to_lowercase().as_str() {
        "sha256" => hash.len() == 64,  // 32 bytes
        "keccak256" => hash.len() == 64, // 32 bytes
        "sha512" => hash.len() == 128,  // 64 bytes
        _ => true, // Unknown algorithm, just do basic validation
    }
}

/// Validates a blockchain address format
pub fn validate_address(address: &str, chain: &str) -> bool {
    match chain.to_lowercase().as_str() {
        "bitcoin" => {
            // Basic Bitcoin address validation (supports legacy, segwit, native segwit)
            address.starts_with('1') || address.starts_with('3') || 
            address.starts_with("bc1")
        },
        "ethereum" => {
            // Ethereum address validation (0x + 40 hex chars, case-insensitive checksum)
            if !address.starts_with("0x") || address.len() != 42 {
                return false;
            }
            address[2..].chars().all(|c| c.is_ascii_hexdigit())
        },
        "solana" => {
            // Solana address validation (base58, 32-44 chars)
            if address.len() < 32 || address.len() > 44 {
                return false;
            }
            address.chars().all(|c| {
                c.is_ascii_alphanumeric() && 
                !"0OIl".contains(c) // Base58 excludes these characters
            })
        },
        _ => true, // Unknown chain, minimal validation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{TransactionStatus, StateChangeType};

    #[test]
    fn test_validate_block_header() {
        let valid_header = BlockHeader {
            height: 1,
            hash: "0000000000000000000abc123".to_string(),
            prev_hash: "0000000000000000000def456".to_string(),
            timestamp: 1630000000,
            extra: Default::default(),
        };
        
        assert!(validate_block_header(&valid_header).is_ok());

        let future_header = BlockHeader {
            timestamp: 2000000000, // Far in the future
            ..valid_header.clone()
        };
        assert!(validate_block_header(&future_header).is_err());
    }

    #[test]
    fn test_validate_transaction() {
        let valid_tx = Transaction {
            txid: "tx123".to_string(),
            block_hash: "block123".to_string(),
            index: 0,
            from: Some("from_addr".to_string()),
            to: Some("to_addr".to_string()),
            value: "1000".to_string(),
            fee: "10".to_string(),
            status: TransactionStatus::Confirmed(1),
            extra: Default::default(),
        };
        
        assert!(validate_transaction(&valid_tx).is_ok());

        let invalid_tx = Transaction {
            txid: "".to_string(), // Empty txid
            ..valid_tx
        };
        assert!(validate_transaction(&invalid_tx).is_err());
    }

    #[test]
    fn test_validate_state_change() {
        let valid_change = StateChange {
            change_type: StateChangeType::BalanceUpdate,
            block_height: 1,
            txid: Some("tx123".to_string()),
            address: "addr1".to_string(),
            previous_state: Some("{}".to_string()),
            new_state: "{\"balance\": 100}".to_string(),
        };
        
        assert!(validate_state_change(&valid_change).is_ok());

        let invalid_change = StateChange {
            address: "".to_string(), // Empty address
            ..valid_change
        };
        assert!(validate_state_change(&invalid_change).is_err());
    }

    #[test]
    fn test_validate_hash() {
        // Valid SHA-256 hash
        assert!(validate_hash(
            "5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8",
            32,
            "sha256"
        ));

        // Invalid length
        assert!(!validate_hash("1234", 32, "sha256"));
        
        // Invalid characters
        assert!(!validate_hash("g000000000000000000000000000000000000000000000000000000000000000", 32, "sha256"));
    }

    #[test]
    fn test_validate_address() {
        // Bitcoin addresses
        assert!(validate_address("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", "bitcoin"));
        assert!(!validate_address("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa!", "bitcoin"));
        
        // Ethereum addresses
        assert!(validate_address("0x742d35Cc6634C0532925a3b844Bc454e4438f44e", "ethereum"));
        assert!(!validate_address("0x742d35Cc6634C0532925a3b844Bc454e4438f44", "ethereum")); // Too short
        
        // Solana addresses
        assert!(validate_address("HN5Hx1J3uHpsa1pWqMhp4sHDBkHWj4cY8VexFHG9WJ1k", "solana"));
        assert!(!validate_address("HN5Hx1J3uHpsa1pWqMhp4sHDBkHWj4cY8VexFHG9WJ1k!", "solana"));
    }
}
