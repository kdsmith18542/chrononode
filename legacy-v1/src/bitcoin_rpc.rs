//! Bitcoin RPC client implementation for interacting with a Bitcoin Core node.

use bitcoin::blockdata::block::Block as BitcoinBlock;
use bitcoin::blockdata::block::BlockHeader;
use bitcoin::hashes::Hash;
use bitcoin::BlockHash;
use jsonrpc_core_client::RpcChannel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

/// Errors that can occur during RPC operations
#[derive(Debug)]
pub enum BitcoinRpcError {
    /// Error from the RPC client
    RpcError(String),
    
    /// Error parsing response
    ParseError(String),
    
    /// Block not found
    BlockNotFound(BlockHash),
    
    /// Other errors
    Other(String),
}

impl fmt::Display for BitcoinRpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BitcoinRpcError::RpcError(e) => write!(f, "RPC error: {}", e),
            BitcoinRpcError::ParseError(e) => write!(f, "Parse error: {}", e),
            BitcoinRpcError::BlockNotFound(hash) => write!(f, "Block not found: {}", hash),
            BitcoinRpcError::Other(e) => write!(f, "Error: {}", e),
        }
    }
}

impl Error for BitcoinRpcError {}

/// Bitcoin RPC client
pub struct BitcoinRpcClient {
    client: RpcChannel,
}

impl BitcoinRpcClient {
    /// Create a new Bitcoin RPC client
    pub async fn new(url: &str) -> Result<Self, Box<dyn Error>> {
        let client = RpcChannel::new(url.to_string())
            .await
            .map_err(|e| BitcoinRpcError::RpcError(e.to_string()))?;
            
        Ok(Self { client })
    }
    
    /// Get the current block count
    pub async fn get_block_count(&self) -> Result<u64, BitcoinRpcError> {
        let result: Value = self.client
            .send_request("getblockcount", ())
            .await
            .map_err(|e| BitcoinRpcError::RpcError(e.to_string()))?;
            
        result.as_u64()
            .ok_or_else(|| BitcoinRpcError::ParseError("Invalid block count response".into()))
    }
    
    /// Get the block hash at a specific height
    pub async fn get_block_hash(&self, height: u64) -> Result<BlockHash, BitcoinRpcError> {
        let hash_str: String = self.client
            .send_request("getblockhash", (height,))
            .await
            .map_err(|e| BitcoinRpcError::RpcError(e.to_string()))?;
            
        BlockHash::from_str(&hash_str)
            .map_err(|e| BitcoinRpcError::ParseError(format!("Invalid block hash: {}", e)))
    }
    
    /// Get a block by its hash
    pub async fn get_block(&self, hash: &BlockHash) -> Result<BitcoinBlock, BitcoinRpcError> {
        #[derive(Deserialize)]
        struct GetBlockResponse {
            #[serde(with = "hex::serde")]
            hex: Vec<u8>,
        }
        
        let response: GetBlockResponse = self.client
            .send_request("getblock", (hash.to_string(), 0))
            .await
            .map_err(|e| BitcoinRpcError::RpcError(e.to_string()))?;
            
        let block = bitcoin::consensus::deserialize(&response.hex)
            .map_err(|e| BitcoinRpcError::ParseError(format!("Failed to deserialize block: {}", e)))?;
            
        Ok(block)
    }
    
    /// Get block header by hash
    pub async fn get_block_header(&self, hash: &BlockHash) -> Result<BlockHeader, BitcoinRpcError> {
        let block = self.get_block(hash).await?;
        Ok(block.header)
    }
    
    /// Get the best block hash
    pub async fn get_best_block_hash(&self) -> Result<BlockHash, BitcoinRpcError> {
        let hash_str: String = self.client
            .send_request("getbestblockhash", ())
            .await
            .map_err(|e| BitcoinRpcError::RpcError(e.to_string()))?;
            
        BlockHash::from_str(&hash_str)
            .map_err(|e| BitcoinRpcError::ParseError(format!("Invalid best block hash: {}", e)))
    }
    
    /// Get the chain tip information
    pub async fn get_chain_tip(&self) -> Result<BlockHash, BitcoinRpcError> {
        self.get_best_block_hash().await
    }
    
    /// Verify that the RPC connection is working
    pub async fn ping(&self) -> Result<(), BitcoinRpcError> {
        self.client
            .send_request::<_, Value>("getblockcount", ())
            .await
            .map(|_| ())
            .map_err(|e| BitcoinRpcError::RpcError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{mock, Server};
    use serde_json::json;
    
    #[tokio::test]
    async fn test_get_block_count() {
        let mut server = Server::new();
        let _m = mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"jsonrpc":"2.0","result":12345,"id":0}"#)
            .create();
            
        let client = BitcoinRpcClient::new(&server.url()).await.unwrap();
        let count = client.get_block_count().await.unwrap();
        
        assert_eq!(count, 12345);
    }
    
    #[tokio::test]
    async fn test_get_block_hash() {
        let mut server = Server::new();
        let _m = mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"jsonrpc":"2.0","result":"00000000000000000001c80148258b0ccc75a49c1be8e5f9b8b8a1cda0f8a4d3","id":0}"#)
            .create();
            
        let client = BitcoinRpcClient::new(&server.url()).await.unwrap();
        let hash = client.get_block_hash(12345).await.unwrap();
        
        assert_eq!(
            hash.to_string(),
            "00000000000000000001c80148258b0ccc75a49c1be8e5f9b8b8a1cda0f8a4d3"
        );
    }
}
