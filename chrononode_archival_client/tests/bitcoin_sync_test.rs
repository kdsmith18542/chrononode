//! Integration tests for Bitcoin synchronization functionality

use chrononode_archival_client::{
    bitcoin_rpc::BitcoinRpcClient,
    bitcoin_sync::{BitcoinBlock, BitcoinSyncClient, BitcoinSyncError, SyncState},
    config::{BitcoinConfig, Config, LoggingConfig},
};
use bitcoin::{
    blockdata::block::BlockHeader, consensus::deserialize, hashes::Hash, BlockHash, BlockHash as Hash,
};
use mockito::{mock, Server};
use rocksdb::DB;
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_bitcoin_sync_basic() -> Result<(), Box<dyn std::error::Error>> {
    // Set up test environment
    let temp_dir = tempdir()?;
    let db_path = temp_dir.path().join("bitcoin_blocks");
    
    // Create a mock Bitcoin RPC server
    let mut server = Server::new_async().await;
    
    // Mock getblockcount response
    let _m1 = mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": 1000
        }).to_string())
        .create_async()
        .await;
    
    // Mock getblockhash response
    let _m2 = mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": "0000000000000000000000000000000000000000000000000000000000000001"
        }).to_string())
        .create_async()
        .await;
    
    // Create config
    let config = Config {
        bitcoin: BitcoinConfig {
            rpc_url: server.url(),
            rpc_username: "test".to_string(),
            rpc_password: "test".to_string(),
            data_dir: db_path.clone(),
            parallel_blocks: 4,
            batch_size: 100,
        },
        logging: LoggingConfig {
            level: "debug".to_string(),
            log_file: None,
        },
    };
    
    // Initialize the sync client
    let mut client = BitcoinSyncClient::new(config).await?;
    
    // Test initial state
    assert_eq!(client.state().height, 0);
    assert_eq!(
        client.state().best_block_hash,
        Hash::from_hex("0000000000000000000000000000000000000000000000000000000000000000").unwrap()
    );
    
    // Test stopping the client
    client.stop();
    assert!(!client.running);
    
    Ok(())
}

#[tokio::test]
async fn test_bitcoin_rpc_client() -> Result<(), Box<dyn std::error::Error>> {
    // Create a mock server
    let mut server = Server::new_async().await;
    
    // Mock getblockcount response
    let _m1 = mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": 1000
        }).to_string())
        .create_async()
        .await;
    
    // Create RPC client
    let client = BitcoinRpcClient::new(&server.url()).await?;
    
    // Test get_block_count
    let count = client.get_block_count().await?;
    assert_eq!(count, 1000);
    
    Ok(())
}

#[test]
fn test_sync_state_serialization() -> Result<(), Box<dyn std::error::Error>> {
    // Test serialization/deserialization of SyncState
    let state = SyncState {
        height: 1000,
        best_block_hash: Hash::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )?,
        total_size: 123456789,
        tx_count: 5000000,
    };
    
    // Serialize
    let serialized = bincode::serialize(&state)?;
    
    // Deserialize
    let deserialized: SyncState = bincode::deserialize(&serialized)?;
    
    // Verify
    assert_eq!(deserialized.height, state.height);
    assert_eq!(deserialized.best_block_hash, state.best_block_hash);
    assert_eq!(deserialized.total_size, state.total_size);
    assert_eq!(deserialized.tx_count, state.tx_count);
    
    Ok(())
}

// Helper function to create a test block
fn create_test_block(height: u64) -> BitcoinBlock {
    let mut header = BlockHeader::default();
    header.version = 0x20000000;
    header.time = 1234567890 + height * 600; // 10 min apart
    
    BitcoinBlock {
        header,
        transactions: vec![],
        height,
        hash: Hash::hash(&height.to_be_bytes()),
    }
}
