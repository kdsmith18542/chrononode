//! GraphQL API implementation for ChronoNode

use super::{ApiService, BlockQueryParams, TransactionQueryParams, ChainStats, QueryResponse, QueryMetadata};
use crate::models::{BlockHeader, Transaction};
use async_graphql::{
    Context, Object, Result as GraphQLResult, Schema, SimpleObject, InputObject,
    EmptyMutation, EmptySubscription, ComplexObject,
};
use std::sync::Arc;
use std::time::Instant;

/// GraphQL schema type
pub type ChronoNodeSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

/// Root query object
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Get a block by height or hash
    async fn block(
        &self,
        ctx: &Context<'_>,
        height: Option<u64>,
        hash: Option<String>,
    ) -> GraphQLResult<Option<BlockHeader>> {
        let start_time = Instant::now();
        let service = ctx.data::<Arc<dyn ApiService>>()?;
        
        let params = BlockQueryParams {
            height,
            hash,
            start_height: None,
            end_height: None,
            pagination: None,
        };
        
        let result = service.get_block(params).await?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("graphql", "block", execution_time);
        
        Ok(result)
    }
    
    /// Get blocks in a range
    async fn blocks(
        &self,
        ctx: &Context<'_>,
        start_height: Option<u64>,
        end_height: Option<u64>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> GraphQLResult<Vec<BlockHeader>> {
        let start_time = Instant::now();
        let service = ctx.data::<Arc<dyn ApiService>>()?;
        
        let pagination = if limit.is_some() || offset.is_some() {
            Some(super::PaginationParams {
                limit: limit.map(|l| l.max(0).min(100) as usize),
                offset: offset.map(|o| o.max(0) as usize),
            })
        } else {
            None
        };
        
        let params = BlockQueryParams {
            height: None,
            hash: None,
            start_height,
            end_height,
            pagination,
        };
        
        let result = service.get_blocks(params).await?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("graphql", "blocks", execution_time);
        
        Ok(result)
    }
    
    /// Get a transaction by hash
    async fn transaction(
        &self,
        ctx: &Context<'_>,
        hash: String,
    ) -> GraphQLResult<Option<Transaction>> {
        let start_time = Instant::now();
        let service = ctx.data::<Arc<dyn ApiService>>()?;
        
        let params = TransactionQueryParams {
            hash: Some(hash),
            block_height: None,
            from_address: None,
            to_address: None,
            start_timestamp: None,
            end_timestamp: None,
            pagination: None,
        };
        
        let result = service.get_transaction(params).await?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("graphql", "transaction", execution_time);
        
        Ok(result)
    }
    
    /// Get transactions with filters
    async fn transactions(
        &self,
        ctx: &Context<'_>,
        block_height: Option<u64>,
        from_address: Option<String>,
        to_address: Option<String>,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> GraphQLResult<Vec<Transaction>> {
        let start_time = Instant::now();
        let service = ctx.data::<Arc<dyn ApiService>>()?;
        
        let pagination = if limit.is_some() || offset.is_some() {
            Some(super::PaginationParams {
                limit: limit.map(|l| l.max(0).min(100) as usize),
                offset: offset.map(|o| o.max(0) as usize),
            })
        } else {
            None
        };
        
        let params = TransactionQueryParams {
            hash: None,
            block_height,
            from_address,
            to_address,
            start_timestamp,
            end_timestamp,
            pagination,
        };
        
        let result = service.get_transactions(params).await?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("graphql", "transactions", execution_time);
        
        Ok(result)
    }
    
    /// Get the latest block height
    async fn latest_block_height(&self, ctx: &Context<'_>) -> GraphQLResult<u64> {
        let start_time = Instant::now();
        let service = ctx.data::<Arc<dyn ApiService>>()?;
        
        let result = service.get_latest_block_height().await?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("graphql", "latest_block_height", execution_time);
        
        Ok(result)
    }
    
    /// Get chain statistics
    async fn chain_stats(&self, ctx: &Context<'_>) -> GraphQLResult<ChainStats> {
        let start_time = Instant::now();
        let service = ctx.data::<Arc<dyn ApiService>>()?;
        
        let result = service.get_chain_stats().await?;
        
        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as u64;
        crate::metrics::record_api_request("graphql", "chain_stats", execution_time);
        
        Ok(result)
    }
}

/// Create a new GraphQL schema
pub fn create_schema(service: Arc<dyn ApiService>) -> ChronoNodeSchema {
    Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(service)
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::MockApiService;
    
    #[tokio::test]
    async fn test_graphql_block_query() {
        let service = Arc::new(MockApiService);
        let schema = create_schema(service);
        
        let query = r#"
            query {
                block(height: 100) {
                    height
                    hash
                    timestamp
                }
            }
        "#;
        
        let result = schema.execute(query).await;
        assert!(result.errors.is_empty());
    }
    
    #[tokio::test]
    async fn test_graphql_latest_height_query() {
        let service = Arc::new(MockApiService);
        let schema = create_schema(service);
        
        let query = r#"
            query {
                latestBlockHeight
            }
        "#;
        
        let result = schema.execute(query).await;
        assert!(result.errors.is_empty());
    }
    
    #[tokio::test]
    async fn test_graphql_chain_stats_query() {
        let service = Arc::new(MockApiService);
        let schema = create_schema(service);
        
        let query = r#"
            query {
                chainStats {
                    latestBlockHeight
                    totalTransactions
                    totalBlocks
                    averageBlockTime
                    chainId
                }
            }
        "#;
        
        let result = schema.execute(query).await;
        assert!(result.errors.is_empty());
    }
}
