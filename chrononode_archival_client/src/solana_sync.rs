//! Solana blockchain synchronization module for ChronoNode Archival Client.
//! Handles full node synchronization, block processing, and state management.

use crate::{
    config::Config,
    errors::BlockchainSyncError,
    events::{
        solana_events::{SolanaCluster, SolanaEventHandler},
        EventBus,
    },
};
use log::{debug, error, info, trace, warn};
use solana_client::{
    client_error::ClientError,
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcBlockConfig, RpcBlockSubscribeConfig, RpcTransactionLogsFilter, RpcTransactionLogsConfig},
    rpc_response::{Response as RpcResponse, RpcBlockConfig, RpcBlockSubscribeConfig, RpcTransactionLogsFilter, RpcTransactionLogsConfig},
};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    hash::Hash,
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
};
use solana_transaction_status::{
    EncodedConfirmedBlock, EncodedTransactionWithStatusMeta, UiTransactionEncoding,
};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

/// Database column family names
const CF_BLOCKS: &str = "blocks";
const CF_TRANSACTIONS: &str = "transactions";
const CF_ACCOUNTS: &str = "accounts";
const CF_SLOT_METADATA: &str = "slot_metadata";

/// Configuration for Solana synchronization
#[derive(Debug, Clone)]
pub struct SolanaSyncConfig {
    /// Cluster to connect to (mainnet-beta, testnet, devnet)
    pub cluster: SolanaCluster,
    /// Whether to enable event publishing
    pub enable_events: bool,
    /// Event bus for publishing blockchain events
    pub event_bus: Option<Arc<EventBus>>,
}

impl Default for SolanaSyncConfig {
    fn default() -> Self {
        Self {
            cluster: SolanaCluster::MainnetBeta,
            enable_events: true,
            event_bus: None,
        }
    }
}

/// Solana synchronization state
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SolanaSyncState {
    /// Current slot
    pub slot: u64,
    /// Block hash of the current head
    pub block_hash: String,
    /// Total size of synced data in bytes
    pub total_size: u64,
    /// Total number of transactions processed
    pub tx_count: u64,
    /// Chain ID of the connected network
    pub chain_id: String,
}

impl Default for SolanaSyncState {
    fn default() -> Self {
        Self {
            slot: 0,
            block_hash: String::new(),
            total_size: 0,
            tx_count: 0,
            chain_id: "mainnet-beta".to_string(),
        }
    }
}

/// Solana synchronization client
pub struct SolanaSyncClient {
    /// Solana RPC client
    rpc_client: Arc<RpcClient>,
    /// Database instance for storing blockchain data
    db: Arc<rocksdb::DB>,
    /// Current synchronization state
    state: SolanaSyncState,
    /// Configuration
    config: Config,
    /// Event handler for processing Solana events
    event_handler: Option<Arc<SolanaEventHandler>>,
    /// Flag to control the synchronization loop
    running: bool,
}

impl SolanaSyncClient {
    /// Create a new SolanaSyncClient with default configuration
    pub async fn new(config: Config) -> Result<Self, BlockchainSyncError> {
        Self::with_config(config, SolanaSyncConfig::default()).await
    }

    /// Create a new SolanaSyncClient with custom configuration
    pub async fn with_config(
        config: Config,
        sync_config: SolanaSyncConfig,
    ) -> Result<Self, BlockchainSyncError> {
        // Initialize RocksDB
        let mut db_opts = rocksdb::Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        
        // Create data directory if it doesn't exist
        if let Some(parent) = config.solana.data_dir.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        
        // Define column families
        let cfs = vec![
            rocksdb::ColumnFamilyDescriptor::new(CF_BLOCKS, rocksdb::Options::default()),
            rocksdb::ColumnFamilyDescriptor::new(CF_TRANSACTIONS, rocksdb::Options::default()),
            rocksdb::ColumnFamilyDescriptor::new(CF_ACCOUNTS, rocksdb::Options::default()),
            rocksdb::ColumnFamilyDescriptor::new(CF_SLOT_METADATA, rocksdb::Options::default()),
        ];
        
        let db = Arc::new(
            rocksdb::DB::open_cf_descriptors(&db_opts, &config.solana.data_dir, cfs)?,
        );
        
        // Initialize RPC client
        let rpc_client = RpcClient::new_with_commitment(
            config.solana.rpc_url.clone(),
            CommitmentConfig::confirmed(),
        );
        
        // Load or initialize state
        let mut state = Self::load_state(&db).await?;
        
        // Initialize event handler if enabled
        let event_handler = if sync_config.enable_events {
            sync_config
                .event_bus
                .clone()
                .map(|bus| Arc::new(SolanaEventHandler::new(bus, sync_config.cluster)))
        } else {
            None
        };
        
        // Update state with chain ID if not set
        if state.chain_id.is_empty() {
            state.chain_id = sync_config.cluster.to_string();
        }
        
        // Get cluster info to verify connection
        let cluster_info = rpc_client.get_cluster_nodes().await?;
        info!("Connected to Solana cluster: {} nodes", cluster_info.len());
        
        // Get chain ID
        let genesis_hash = rpc_client.get_genesis_hash().await?;
        let chain_id = match config.solana.network.as_str() {
            "mainnet-beta" => "mainnet-beta",
            "testnet" => "testnet",
            "devnet" => "devnet",
            "localnet" => "localnet",
            _ => "custom",
        }.to_string();
        
        info!("Initialized SolanaSyncClient for chain: {}", chain_id);
        info!("Genesis hash: {}", genesis_hash);
        info!("Current slot: {}", state.slot);
        
        Ok(Self {
            rpc_client: Arc::new(rpc_client),
            db: Arc::new(db),
            state,
            config,
            event_handler,
            running: true,
        })
    }
    
    /// Load synchronization state from the database
    async fn load_state(db: &rocksdb::DB) -> Result<SolanaSyncState, BlockchainSyncError> {
        let cf = db.cf_handle(CF_SLOT_METADATA).ok_or_else(|| {
            BlockchainSyncError::DatabaseError("Slot metadata column family not found".into())
        })?;
        
        match db.get_cf(&cf, b"sync_state")? {
            Some(data) => {
                bincode::deserialize(&data).map_err(|e| {
                    BlockchainSyncError::SerializationError(e.to_string())
                })
            }
            None => Ok(SolanaSyncState::default()),
        }
    }
    
    /// Save synchronization state to the database
    async fn save_state(&self) -> Result<(), BlockchainSyncError> {
        let cf = self.db.cf_handle(CF_SLOT_METADATA).ok_or_else(|| {
            BlockchainSyncError::DatabaseError("Slot metadata column family not found".into())
        })?;
        
        let data = bincode::serialize(&self.state)
            .map_err(|e| BlockchainSyncError::SerializationError(e.to_string()))?;
            
        self.db.put_cf(&cf, b"sync_state", data)?;
        Ok(())
    }
    
    /// Get the current slot from the Solana node
    async fn get_slot(&self) -> Result<u64, BlockchainSyncError> {
        self.rpc_client
            .get_slot()
            .await
            .map_err(|e| BlockchainSyncError::RpcError(e.to_string()))
    }
    
    /// Get a block by slot
    async fn get_block(
        &self,
        slot: u64,
    ) -> Result<Option<EncodedConfirmedBlock>, BlockchainSyncError> {
        let config = RpcBlockConfig {
            encoding: Some(UiTransactionEncoding::JsonParsed),
            transaction_details: Some(solana_transaction_status::TransactionDetails::Full),
            rewards: Some(true),
            commitment: Some(CommitmentConfig::confirmed()),
            ..Default::default()
        };
        
        match self.rpc_client.get_block_with_config(slot, config).await {
            Ok(block) => Ok(Some(block)),
            Err(ClientError::Reqwest(e)) if e.is_timeout() => {
                warn!("Timeout fetching block {}", slot);
                Ok(None)
            }
            Err(ClientError::RpcError(e)) if e.code == -32004 => {
                // Skip slot
                debug!("Skipping skipped slot {}", slot);
                Ok(None)
            }
            Err(e) => Err(BlockchainSyncError::RpcError(e.to_string())),
        }
    }
    
    /// Process a single slot and update the database
    async fn process_slot(&mut self, slot: u64) -> Result<(), BlockchainSyncError> {
        debug!("Processing slot {}", slot);
        
        // Get block data
        let block = self.get_block(slot).await?;
        
        if let Some(block) = block {
            // Emit block event if handler is configured
            if let Some(handler) = &self.event_handler {
                let block_time = self.rpc_client.get_block_time(slot).await.ok();
                if let Err(e) = handler.process_slot(slot, &block, block_time, true).await {
                    error!("Failed to process slot event: {}", e);
                    // Continue processing even if event emission fails
                }
            }
            
            // Store block data
            self.store_block(slot, &block).await?;
            
            // Update state
            self.state.slot = slot;
            self.state.block_hash = block.blockhash.clone();
            self.state.tx_count += block.transactions.len() as u64;
            
            // Save state
            self.save_state()?;
            
            debug!("Processed slot {} with {} transactions", slot, block.transactions.len());
        }
        
        Ok(())
    }
    
    /// Store block data in the database
    async fn store_block(
        &self,
        slot: u64,
        block: &EncodedConfirmedBlock,
    ) -> Result<(), BlockchainSyncError> {
        let cf_blocks = self.db.cf_handle(CF_BLOCKS).ok_or_else(|| {
            BlockchainSyncError::DatabaseError("Blocks column family not found".into())
        })?;
        
        let cf_txs = self.db.cf_handle(CF_TRANSACTIONS).ok_or_else(|| {
            BlockchainSyncError::DatabaseError("Transactions column family not found".into())
        })?;
        
        let cf_accounts = self.db.cf_handle(CF_ACCOUNTS).ok_or_else(|| {
            BlockchainSyncError::DatabaseError("Accounts column family not found".into())
        })?;
        
        // Start a batch write
        let mut batch = rocksdb::WriteBatch::default();
        
        // Serialize and store block
        let block_key = slot.to_be_bytes();
        let block_data = bincode::serialize(&block)
            .map_err(|e| BlockchainSyncError::SerializationError(e.to_string()))?;
        batch.put_cf(&cf_blocks, block_key, block_data);
        
        // Store transactions and accounts
        for tx in &block.transactions {
            if let Some(signature) = &tx.transaction.signatures.get(0) {
                // Store transaction
                let tx_key = signature.as_bytes();
                let tx_data = bincode::serialize(&tx)
                    .map_err(|e| BlockchainSyncError::SerializationError(e.to_string()))?;
                batch.put_cf(&cf_txs, tx_key, tx_data);
                
                // Store accounts
                if let Some(meta) = &tx.meta {
                    for account_key in meta.loaded_addresses.iter() {
                        // In a real implementation, we would fetch and store the account data
                        let account_key_bytes = bs58::decode(account_key)
                            .into_vec()
                            .map_err(|_| BlockchainSyncError::InvalidData("Invalid account key".into()))?;
                        // Just mark the account as touched for now
                        batch.put_cf(&cf_accounts, account_key_bytes, &[]);
                    }
                }
            }
        }
        
        // Commit the batch
        self.db.write(batch)?;
        
        trace!("Stored slot {} ({})", slot, block.blockhash);
        
        Ok(())
    }
    
    /// Synchronize slots from the current slot to the latest
    pub async fn sync_blocks(&mut self) -> Result<(), BlockchainSyncError> {
        info!("Starting Solana blockchain synchronization...");
        info!("Current slot: {}", self.state.slot);
        
        while self.running {
            // Get the current slot from the node
            let node_slot = self.get_slot().await?;
            
            if self.state.slot >= node_slot {
                // We're in sync, wait for new slots
                info!("Blockchain is synchronized up to slot {}", self.state.slot);
                tokio::time::sleep(Duration::from_secs(2)).await; // Solana produces slots every ~400ms
                continue;
            }
            
            // Process slots in batches
            let end_slot = std::cmp::min(
                self.state.slot + self.config.solana.batch_size as u64,
                node_slot
            );
            
            info!("Syncing slots {} to {}...", self.state.slot + 1, end_slot);
            
            for slot in (self.state.slot + 1)..=end_slot {
                if !self.running {
                    info!("Synchronization stopped by user");
                    return Ok(());
                }
                
                match self.process_slot(slot).await {
                    Ok(_) => {
                        // Progress reporting
                        if slot % 1000 == 0 {
                            info!("Processed slot {}/{}", slot, node_slot);
                        }
                    }
                    Err(e) => {
                        error!("Failed to process slot {}: {}", slot, e);
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
        // This would use WebSocket to subscribe to new blocks
        // Implementation depends on the specific requirements
        Ok(())
    }
    
    /// Get the current synchronization state
    pub fn state(&self) -> &SolanaSyncState {
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
    use solana_sdk::signature::Keypair;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_solana_sync() {
        // Set up test environment
        let temp_dir = tempdir().unwrap();
        let config = Config::default(); // You'll need to implement Default for Config
        
        // Create a client with a mock RPC URL (would need a mock server in practice)
        let mut client = SolanaSyncClient::new(config).await.unwrap();
        
        // Test stopping the client
        client.stop();
        assert!(!client.running);
    }
}
