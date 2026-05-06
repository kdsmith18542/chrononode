//! API module for ChronoNode Archival Client
//! 
//! This module provides GraphQL and JSON-RPC APIs for querying blockchain data.

pub mod graphql;
pub mod jsonrpc;
pub mod server;

use crate::models::{BlockHeader, Transaction, StateChange};
use async_graphql::{Context, Object, Result as GraphQLResult, Schema, SimpleObject};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Server bind address
    pub bind_address: String,
    /// Enable GraphQL endpoint
    pub enable_graphql: bool,
    /// Enable JSON-RPC endpoint
    pub enable_jsonrpc: bool,
    /// Maximum query complexity for GraphQL
    pub max_query_complexity: usize,
    /// Maximum query depth for GraphQL
    pub max_query_depth: usize,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Requests per minute
    pub requests_per_minute: u32,
    /// Burst size
    pub burst_size: u32,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0:8080".to_string(),
            enable_graphql: true,
            enable_jsonrpc: true,
            max_query_complexity: 1000,
            max_query_depth: 10,
            rate_limit: RateLimitConfig {
                requests_per_minute: 100,
                burst_size: 10,
            },
        }
    }
}

/// Query response wrapper
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct QueryResponse<T> {
    /// The actual data
    pub data: T,
    /// Metadata about the query
    pub metadata: QueryMetadata,
}

/// Query metadata
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct QueryMetadata {
    /// Query execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of results returned
    pub result_count: usize,
    /// Whether the result is cached
    pub cached: bool,
}

/// Pagination parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    /// Number of items to return (max 100)
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            limit: Some(50),
            offset: Some(0),
        }
    }
}

/// Block query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockQueryParams {
    /// Block height
    pub height: Option<u64>,
    /// Block hash
    pub hash: Option<String>,
    /// Start height for range queries
    pub start_height: Option<u64>,
    /// End height for range queries
    pub end_height: Option<u64>,
    /// Pagination
    pub pagination: Option<PaginationParams>,
}

/// Transaction query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionQueryParams {
    /// Transaction hash
    pub hash: Option<String>,
    /// Block height
    pub block_height: Option<u64>,
    /// From address
    pub from_address: Option<String>,
    /// To address
    pub to_address: Option<String>,
    /// Start timestamp
    pub start_timestamp: Option<u64>,
    /// End timestamp
    pub end_timestamp: Option<u64>,
    /// Pagination
    pub pagination: Option<PaginationParams>,
}

/// API service trait for data access
#[async_trait::async_trait]
pub trait ApiService: Send + Sync {
    /// Get block by height or hash
    async fn get_block(&self, params: BlockQueryParams) -> GraphQLResult<Option<BlockHeader>>;
    
    /// Get blocks in a range
    async fn get_blocks(&self, params: BlockQueryParams) -> GraphQLResult<Vec<BlockHeader>>;
    
    /// Get transaction by hash
    async fn get_transaction(&self, params: TransactionQueryParams) -> GraphQLResult<Option<Transaction>>;
    
    /// Get transactions
    async fn get_transactions(&self, params: TransactionQueryParams) -> GraphQLResult<Vec<Transaction>>;
    
    /// Get latest block height
    async fn get_latest_block_height(&self) -> GraphQLResult<u64>;
    
    /// Get chain statistics
    async fn get_chain_stats(&self) -> GraphQLResult<ChainStats>;
}

/// Chain statistics
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct ChainStats {
    /// Latest block height
    pub latest_block_height: u64,
    /// Total number of transactions
    pub total_transactions: u64,
    /// Total number of blocks
    pub total_blocks: u64,
    /// Average block time in seconds
    pub average_block_time: f64,
    /// Chain ID
    pub chain_id: String,
}

/// Mock API service for testing
pub struct MockApiService;

#[async_trait::async_trait]
impl ApiService for MockApiService {
    async fn get_block(&self, _params: BlockQueryParams) -> GraphQLResult<Option<BlockHeader>> {
        Ok(Some(BlockHeader::default()))
    }
    
    async fn get_blocks(&self, _params: BlockQueryParams) -> GraphQLResult<Vec<BlockHeader>> {
        Ok(vec![BlockHeader::default()])
    }
    
    async fn get_transaction(&self, _params: TransactionQueryParams) -> GraphQLResult<Option<Transaction>> {
        Ok(Some(Transaction::default()))
    }
    
    async fn get_transactions(&self, _params: TransactionQueryParams) -> GraphQLResult<Vec<Transaction>> {
        Ok(vec![Transaction::default()])
    }
    
    async fn get_latest_block_height(&self) -> GraphQLResult<u64> {
        Ok(1000)
    }
    
    async fn get_chain_stats(&self) -> GraphQLResult<ChainStats> {
        Ok(ChainStats {
            latest_block_height: 1000,
            total_transactions: 50000,
            total_blocks: 1000,
            average_block_time: 600.0,
            chain_id: "bitcoin".to_string(),
        })
    }
}
