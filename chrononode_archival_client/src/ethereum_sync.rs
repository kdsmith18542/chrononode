//! Ethereum blockchain synchronization module for ChronoNode Archival Client.
//! Handles full node synchronization, block processing, and state management.

use crate::{
    config::Config,
    errors::BlockchainSyncError,
    events::{
        ethereum_events::EthereumEventHandler,
        EventBus,
    },
};
use ethers::{
    core::types::{
        Block, BlockNumber, Transaction, TransactionReceipt, H256, U64,
    },
    providers::{Http, Middleware, Provider, Ws},
    types::BlockId,
};
use futures_util::StreamExt;
use log::{debug, error, info, trace, warn};
use rocksdb::{DB, Options};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::mpsc;

/// Database column family names
const CF_BLOCKS: &str = "blocks";
const CF_TRANSACTIONS: &str = "transactions";
const CF_RECEIPTS: &str = "receipts";
const CF_STATE: &str = "state";

/// Ethereum synchronization state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumSyncState {
    /// Current block number
    pub block_number: u64,
    /// Block hash of the current head
    pub block_hash: H256,
    /// Total size of synced data in bytes
    pub total_size: u64,
    /// Total number of transactions processed
    pub tx_count: u64,
    /// Chain ID of the connected network
    pub chain_id: u64,
}

impl Default for EthereumSyncState {
    fn default() -> Self {
        Self {
            block_number: 0,
            block_hash: H256::zero(),
            total_size: 0,
            tx_count: 0,
            chain_id: 1, // Default to mainnet
        }
    }
}

/// Configuration for Ethereum synchronization
#[derive(Debug, Clone)]
pub struct EthereumSyncConfig {
    /// Chain ID of the Ethereum network
    pub chain_id: u64,
    /// Whether to enable event publishing
    pub enable_events: bool,
    /// Event bus for publishing blockchain events
    pub event_bus: Option<Arc<EventBus>>,
}

impl Default for EthereumSyncConfig {
    fn default() -> Self {
        Self {
            chain_id: 1, // Mainnet
            enable_events: true,
            event_bus: None,
        }
    }
}

/// Ethereum synchronization client
pub struct EthereumSyncClient {
    /// Ethereum RPC provider
    provider: Arc<Provider<Http>>,
    /// Database instance for storing blockchain data
    db: Arc<DB>,
    /// Current synchronization state
    state: EthereumSyncState,
    /// Configuration
    config: Config,
    /// Event handler for processing Ethereum events
    event_handler: Option<Arc<EthereumEventHandler>>,
    /// Flag to control the synchronization loop
    running: bool,
}

impl EthereumSyncClient {
    /// Create a new EthereumSyncClient with default configuration
    pub async fn new(config: Config) -> Result<Self, BlockchainSyncError> {
        Self::with_config(config, EthereumSyncConfig::default()).await
    }

    /// Create a new EthereumSyncClient with custom configuration
    pub async fn with_config(
        config: Config,
        sync_config: EthereumSyncConfig,
    ) -> Result<Self, BlockchainSyncError> {
        // Initialize RocksDB
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        
        // Create data directory if it doesn't exist
        if let Some(parent) = config.ethereum.data_dir.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        
        // Define column families
        let cfs = vec![CF_BLOCKS, CF_TRANSACTIONS, CF_RECEIPTS, CF_STATE];
        let db = DB::open_cf(&db_opts, &config.ethereum.data_dir, cfs)?;
        
        // Initialize Ethereum provider
        let provider = Provider::<Http>::try_from(&config.ethereum.rpc_url)
            .map_err(|e| BlockchainSyncError::ConnectionError(e.to_string()))?;
            
        // Load or initialize state
        let mut state = Self::load_state(&db).await?;
        
        // Initialize event handler if enabled
        let event_handler = if sync_config.enable_events {
            sync_config
                .event_bus
                .clone()
                .map(|bus| Arc::new(EthereumEventHandler::new(bus, sync_config.chain_id)))
        } else {
            None
        };
        
        // Update state with chain ID if not set
        if state.chain_id == 0 {
            state.chain_id = sync_config.chain_id;
        }
        
        // Get chain ID
        let chain_id = provider.get_chainid().await?.as_u64();
        
        info!("Initialized EthereumSyncClient for chain ID: {}", chain_id);
        info!("Current block: {}", state.block_number);
        
        Ok(Self {
            provider: Arc::new(provider),
            db: Arc::new(db),
            state,
            config,
            event_handler,
            running: true,
        })
    }
    
    /// Load synchronization state from the database
    async fn load_state(db: &DB) -> Result<EthereumSyncState, BlockchainSyncError> {
        let cf = db.cf_handle(CF_STATE).ok_or_else(|| {
            BlockchainSyncError::DatabaseError("State column family not found".into())
        })?;
        
        match db.get_cf(&cf, b"sync_state")? {
            Some(data) => {
                bincode::deserialize(&data).map_err(|e| {
                    BlockchainSyncError::SerializationError(e.to_string())
                })
            }
            None => Ok(EthereumSyncState::default()),
        }
    }
    
    /// Save synchronization state to the database
    async fn save_state(&self) -> Result<(), BlockchainSyncError> {
        let cf = self.db.cf_handle(CF_STATE).ok_or_else(|| {
            BlockchainSyncError::DatabaseError("State column family not found".into())
        })?;
        
        let data = bincode::serialize(&self.state)
            .map_err(|e| BlockchainSyncError::SerializationError(e.to_string()))?;
            
        self.db.put_cf(&cf, b"sync_state", data)?;
        Ok(())
    }
    
    /// Get the current block number from the Ethereum node
    async fn get_block_number(&self) -> Result<u64, BlockchainSyncError> {
        self.provider
            .get_block_number()
            .await
            .map(|n| n.as_u64())
            .map_err(|e| BlockchainSyncError::RpcError(e.to_string()))
    }
    
    /// Get a block by number
    async fn get_block(&self, block_number: u64) -> Result<Option<Block<H256>>, BlockchainSyncError> {
        self.provider
            .get_block(block_number)
            .await
            .map_err(|e| BlockchainSyncError::RpcError(e.to_string()))
    }
    
    /// Get transaction receipts for a block
    async fn get_transaction_receipts(
        &self,
        block_number: u64,
    ) -> Result<Vec<TransactionReceipt>, BlockchainSyncError> {
        let block = self
            .provider
            .get_block_with_txs(block_number)
            .await
            .map_err(|e| BlockchainSyncError::RpcError(e.to_string()))?
            .ok_or_else(|| BlockchainSyncError::BlockNotFound(block_number.to_string()))?;
            
        let mut receipts = Vec::with_capacity(block.transactions.len());
        
        for tx in block.transactions {
            let receipt = self
                .provider
                .get_transaction_receipt(tx.hash)
                .await
                .map_err(|e| BlockchainSyncError::RpcError(e.to_string()))?
                .ok_or_else(|| BlockchainSyncError::TransactionNotFound(tx.hash.to_string()))?;
                
            receipts.push(receipt);
        }
        
        Ok(receipts)
    }
    
    /// Process a single block and update the database
    async fn process_block(&mut self, block_number: u64) -> Result<(), BlockchainSyncError> {
        debug!("Processing block {}", block_number);

        // Get block data
        let block = self.get_block(block_number).await?
            .ok_or_else(|| BlockchainSyncError::BlockNotFound(block_number))?;

        // Get transaction receipts
        let receipts = self.get_transaction_receipts(block_number).await?;

        // Emit block event if handler is configured
        if let Some(handler) = &self.event_handler {
            if let Err(e) = handler.process_block(&block, &receipts).await {
                error!("Failed to process block event: {}", e);
                // Continue processing even if event emission fails
            }
        }

        // Store block data
        self.store_block(block, receipts).await?;

        // Update state
        self.state.block_number = block_number;
        self.state.block_hash = block.hash.unwrap_or_default();
        self.state.tx_count += block.transactions.len() as u64;
        
        // Save state
        self.save_state()?;

        debug!("Processed block {} with {} transactions", block_number, block.transactions.len());
        Ok(())
    }
    
    /// Store block data in the database
    async fn store_block(
        &self,
        block: Block<H256>,
        receipts: Vec<TransactionReceipt>,
    ) -> Result<(), BlockchainSyncError> {
        let block_number = block.number.unwrap().as_u64();
        let block_hash = block.hash.unwrap();
        
        let cf_blocks = self.db.cf_handle(CF_BLOCKS).ok_or_else(|| {
            BlockchainSyncError::DatabaseError("Blocks column family not found".into())
        })?;
        
        let cf_txs = self.db.cf_handle(CF_TRANSACTIONS).ok_or_else(|| {
            BlockchainSyncError::DatabaseError("Transactions column family not found".into())
        })?;
        
        let cf_receipts = self.db.cf_handle(CF_RECEIPTS).ok_or_else(|| {
            BlockchainSyncError::DatabaseError("Receipts column family not found".into())
        })?;
        
        // Start a batch write
        let mut batch = rocksdb::WriteBatch::default();
        
        // Serialize and store block
        let block_key = block_number.to_be_bytes();
        let block_data = bincode::serialize(&block)
            .map_err(|e| BlockchainSyncError::SerializationError(e.to_string()))?;
        batch.put_cf(&cf_blocks, block_key, block_data);
        
        // Store transactions and receipts
        for (i, tx) in block.transactions.iter().enumerate() {
            // Store transaction
            let tx_key = tx.hash.as_bytes();
            let tx_data = bincode::serialize(&tx)
                .map_err(|e| BlockchainSyncError::SerializationError(e.to_string()))?;
            batch.put_cf(&cf_txs, tx_key, tx_data);
            
            // Store receipt if available
            if let Some(receipt) = receipts.get(i) {
                let receipt_key = tx.hash.as_bytes();
                let receipt_data = bincode::serialize(&receipt)
                    .map_err(|e| BlockchainSyncError::SerializationError(e.to_string()))?;
                batch.put_cf(&cf_receipts, receipt_key, receipt_data);
            }
        }
        
        // Commit the batch
        self.db.write(batch)?;
        
        trace!("Stored block {} ({})", block_number, block_hash);
        
        Ok(())
    }
    
    /// Synchronize blocks from the current height to the latest
    pub async fn sync_blocks(&mut self) -> Result<(), BlockchainSyncError> {
        info!("Starting Ethereum blockchain synchronization...");
        info!("Current block: {}", self.state.block_number);
        
        while self.running {
            // Get the current block number from the node
            let node_block_number = self.get_block_number().await?;
            
            if self.state.block_number >= node_block_number {
                // We're in sync, wait for new blocks
                info!("Blockchain is synchronized up to block {}", self.state.block_number);
                tokio::time::sleep(Duration::from_secs(15)).await;
                continue;
            }
            
            // Process blocks in batches
            let end_block = std::cmp::min(
                self.state.block_number + self.config.ethereum.batch_size as u64,
                node_block_number
            );
            
            info!("Syncing blocks {} to {}...", self.state.block_number + 1, end_block);
            
            for block_number in (self.state.block_number + 1)..=end_block {
                if !self.running {
                    info!("Synchronization stopped by user");
                    return Ok(());
                }
                
                match self.process_block(block_number).await {
                    Ok(_) => {
                        // Progress reporting
                        if block_number % 1000 == 0 {
                            info!("Processed block {}/{}", block_number, node_block_number);
                        }
                    }
                    Err(e) => {
                        error!("Failed to process block {}: {}", block_number, e);
                        // Implement backoff and retry logic here
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                }
            }
            
            // Save state after each batch
            self.save_state().await?;
        }
        
        Ok(())
    }
    
    /// Subscribe to new blocks
    pub async fn subscribe_to_blocks(&self) -> Result<(), BlockchainSyncError> {
        // This would use WebSocket provider to subscribe to new blocks
        // Implementation depends on the specific requirements
        Ok(())
    }
    
    /// Get the current synchronization state
    pub fn state(&self) -> &EthereumSyncState {
        &self.state
    }
    
    /// Stop the synchronization process
    pub fn stop(&mut self) {
        self.running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::{
        core::types::{Block, Transaction, H256, U64},
        providers::Provider,
    };
    use mockito::mock;
    use std::convert::TryFrom;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_ethereum_sync() {
        // Set up test environment
        let temp_dir = tempdir().unwrap();
        let config = Config::default(); // You'll need to implement Default for Config
        
        // Create a mock server
        let mock_server = mockito::Server::new();
        
        // Mock the chain ID request
        let _m1 = mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"jsonrpc":"2.0","id":1,"result":"0x1"}"#)
            .create();
            
        // Mock the block number request
        let _m2 = mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"jsonrpc":"2.0","id":1,"result":"0x100"}"#)
            .create();
        
        // Create a client with the mock server
        let mut client = EthereumSyncClient::new(config).await.unwrap();
        
        // Test stopping the client
        client.stop();
        assert!(!client.running);
    }
}
