//! JSON-RPC API implementation for ChronoNode

use super::{ApiService, BlockQueryParams, TransactionQueryParams, ChainStats};
use crate::models::{BlockHeader, Transaction};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::ErrorCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// JSON-RPC API trait
#[rpc(server)]
pub trait ChronoNodeRpc {
    /// Get a block by height
    #[method(name = "getBlockByHeight")]
    async fn get_block_by_height(&self, height: u64) -> RpcResult<Option<BlockHeader>>;
    
    /// Get a block by hash
    #[method(name = "getBlockByHash")]
    async fn get_block_by_hash(&self, hash: String) -> RpcResult<Option<BlockHeader>>;
    
    /// Get blocks in a range
    #[method(name = "getBlocks")]
    async fn get_blocks(&self, start_height: u64, end_height: u64, limit: Option<usize>) -> RpcResult<Vec<BlockHeader>>;
    
    /// Get a transaction by hash
    #[method(name = "getTransaction")]
    async fn get_transaction(&self, hash: String) -> RpcResult<Option<Transaction>>;
    
    /// Get transactions by block height
    #[method(name = "getTransactionsByBlock")]
    async fn get_transactions_by_block(&self, block_height: u64, limit: Option<usize>) -> RpcResult<Vec<Transaction>>;
    
    /// Get transactions by address
    #[method(name = "getTransactionsByAddress")]
    async fn get_transactions_by_address(&self, address: String, limit: Option<usize>) -> RpcResult<Vec<Transaction>>;
    
    /// Get the latest block height
    #[method(name = "getLatestBlockHeight")]
    async fn get_latest_block_height(&self) -> RpcResult<u64>;
    
    /// Get chain statistics
    #[method(name = "getChainStats")]
    async fn get_chain_stats(&self) -> RpcResult<ChainStats>;
    
    /// Health check
    #[method(name = "health")]
    async fn health(&self) -> RpcResult<String>;
}

/// JSON-RPC server implementation
pub struct ChronoNodeRpcImpl {
    service: Arc<dyn ApiService>,
}

impl ChronoNodeRpcImpl {
    /// Create a new JSON-RPC server implementation
    pub fn new(service: Arc<dyn ApiService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl ChronoNodeRpcServer for ChronoNodeRpcImpl {
    async fn get_block_by_height(&self, height: u64) -> RpcResult<Option<BlockHeader>> {
        let start_time = Instant::now();
        
        let params = BlockQueryParams {
            height: Some(height),
            hash: None,
            start_height: None,
            end_height: None,
            pagination: None,
        };
        
        let result = self.service.get_block(params).await
            .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("jsonrpc", "get_block_by_height", execution_time);
        
        Ok(result)
    }
    
    async fn get_block_by_hash(&self, hash: String) -> RpcResult<Option<BlockHeader>> {
        let start_time = Instant::now();
        
        let params = BlockQueryParams {
            height: None,
            hash: Some(hash),
            start_height: None,
            end_height: None,
            pagination: None,
        };
        
        let result = self.service.get_block(params).await
            .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("jsonrpc", "get_block_by_hash", execution_time);
        
        Ok(result)
    }
    
    async fn get_blocks(&self, start_height: u64, end_height: u64, limit: Option<usize>) -> RpcResult<Vec<BlockHeader>> {
        let start_time = Instant::now();
        
        // Validate range
        if end_height < start_height {
            return Err(jsonrpsee::core::Error::Custom("end_height must be >= start_height".to_string()));
        }
        
        // Limit the range to prevent abuse
        if end_height - start_height > 1000 {
            return Err(jsonrpsee::core::Error::Custom("Range too large, maximum 1000 blocks".to_string()));
        }
        
        let pagination = limit.map(|l| super::PaginationParams {
            limit: Some(l.min(100)), // Max 100 blocks
            offset: Some(0),
        });
        
        let params = BlockQueryParams {
            height: None,
            hash: None,
            start_height: Some(start_height),
            end_height: Some(end_height),
            pagination,
        };
        
        let result = self.service.get_blocks(params).await
            .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("jsonrpc", "get_blocks", execution_time);
        
        Ok(result)
    }
    
    async fn get_transaction(&self, hash: String) -> RpcResult<Option<Transaction>> {
        let start_time = Instant::now();
        
        let params = TransactionQueryParams {
            hash: Some(hash),
            block_height: None,
            from_address: None,
            to_address: None,
            start_timestamp: None,
            end_timestamp: None,
            pagination: None,
        };
        
        let result = self.service.get_transaction(params).await
            .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("jsonrpc", "get_transaction", execution_time);
        
        Ok(result)
    }
    
    async fn get_transactions_by_block(&self, block_height: u64, limit: Option<usize>) -> RpcResult<Vec<Transaction>> {
        let start_time = Instant::now();
        
        let pagination = limit.map(|l| super::PaginationParams {
            limit: Some(l.min(1000)), // Max 1000 transactions
            offset: Some(0),
        });
        
        let params = TransactionQueryParams {
            hash: None,
            block_height: Some(block_height),
            from_address: None,
            to_address: None,
            start_timestamp: None,
            end_timestamp: None,
            pagination,
        };
        
        let result = self.service.get_transactions(params).await
            .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("jsonrpc", "get_transactions_by_block", execution_time);
        
        Ok(result)
    }
    
    async fn get_transactions_by_address(&self, address: String, limit: Option<usize>) -> RpcResult<Vec<Transaction>> {
        let start_time = Instant::now();
        
        let pagination = limit.map(|l| super::PaginationParams {
            limit: Some(l.min(1000)), // Max 1000 transactions
            offset: Some(0),
        });
        
        let params = TransactionQueryParams {
            hash: None,
            block_height: None,
            from_address: Some(address.clone()),
            to_address: Some(address), // Search both from and to
            start_timestamp: None,
            end_timestamp: None,
            pagination,
        };
        
        let result = self.service.get_transactions(params).await
            .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("jsonrpc", "get_transactions_by_address", execution_time);
        
        Ok(result)
    }
    
    async fn get_latest_block_height(&self) -> RpcResult<u64> {
        let start_time = Instant::now();
        
        let result = self.service.get_latest_block_height().await
            .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("jsonrpc", "get_latest_block_height", execution_time);
        
        Ok(result)
    }
    
    async fn get_chain_stats(&self) -> RpcResult<ChainStats> {
        let start_time = Instant::now();
        
        let result = self.service.get_chain_stats().await
            .map_err(|e| jsonrpsee::core::Error::Custom(e.to_string()))?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("jsonrpc", "get_chain_stats", execution_time);
        
        Ok(result)
    }
    
    async fn health(&self) -> RpcResult<String> {
        Ok("OK".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::MockApiService;
    
    #[tokio::test]
    async fn test_jsonrpc_get_latest_height() {
        let service = Arc::new(MockApiService);
        let rpc_impl = ChronoNodeRpcImpl::new(service);
        
        let result = rpc_impl.get_latest_block_height().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1000);
    }
    
    #[tokio::test]
    async fn test_jsonrpc_health() {
        let service = Arc::new(MockApiService);
        let rpc_impl = ChronoNodeRpcImpl::new(service);
        
        let result = rpc_impl.health().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "OK");
    }
}
