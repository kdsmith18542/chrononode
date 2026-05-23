use crate::api::http::{BlockResponse, ChainInfo};
use crate::api::ApiState;
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use std::sync::Arc;

#[rpc(server)]
pub trait ChronoRpc {
    #[method(name = "chrono_getBlockByHeight")]
    async fn get_block_by_height(&self, chain_id: String, height: u64) -> RpcResult<BlockResponse>;

    #[method(name = "chrono_getBlockByHash")]
    async fn get_block_by_hash(&self, chain_id: String, hash: String) -> RpcResult<BlockResponse>;

    #[method(name = "chrono_getChainList")]
    async fn get_chain_list(
        &self,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> RpcResult<Vec<ChainInfo>>;
}

pub struct ChronoRpcImpl {
    pub state: Arc<ApiState>,
}

#[async_trait::async_trait]
impl ChronoRpcServer for ChronoRpcImpl {
    async fn get_block_by_height(&self, chain_id: String, height: u64) -> RpcResult<BlockResponse> {
        let pipeline = self.state.pipeline.as_ref().ok_or_else(|| {
            jsonrpsee::types::ErrorObjectOwned::owned(
                jsonrpsee::types::error::ErrorCode::ServerIsBusy.code(),
                "pipeline not initialized",
                None::<()>,
            )
        })?;
        let block = pipeline
            .get_block_by_height(&chain_id, height)
            .await
            .map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InvalidParams.code(),
                    e.to_string(),
                    None::<()>,
                )
            })?;
        Ok(BlockResponse {
            chain_id: block.chain_id.clone(),
            height: block.height,
            block_hash: block.block_hash_hex(),
            timestamp: block.timestamp,
            tx_count: block.transactions.len(),
            event_count: block.events.len(),
        })
    }

    async fn get_block_by_hash(&self, chain_id: String, hash: String) -> RpcResult<BlockResponse> {
        let pipeline = self.state.pipeline.as_ref().ok_or_else(|| {
            jsonrpsee::types::ErrorObjectOwned::owned(
                jsonrpsee::types::error::ErrorCode::ServerIsBusy.code(),
                "pipeline not initialized",
                None::<()>,
            )
        })?;
        let block = pipeline
            .get_block_by_hash(&chain_id, &hash)
            .await
            .map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InvalidParams.code(),
                    e.to_string(),
                    None::<()>,
                )
            })?;
        Ok(BlockResponse {
            chain_id: block.chain_id.clone(),
            height: block.height,
            block_hash: block.block_hash_hex(),
            timestamp: block.timestamp,
            tx_count: block.transactions.len(),
            event_count: block.events.len(),
        })
    }

    async fn get_chain_list(
        &self,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> RpcResult<Vec<ChainInfo>> {
        let pipeline = self.state.pipeline.as_ref().ok_or_else(|| {
            jsonrpsee::types::ErrorObjectOwned::owned(
                jsonrpsee::types::error::ErrorCode::ServerIsBusy.code(),
                "pipeline not initialized",
                None::<()>,
            )
        })?;
        let chains = pipeline
            .index
            .get_chain_list(limit.unwrap_or(20), offset.unwrap_or(0))
            .await
            .map_err(|e| {
                jsonrpsee::types::ErrorObjectOwned::owned(
                    jsonrpsee::types::error::ErrorCode::InternalError.code(),
                    e.to_string(),
                    None::<()>,
                )
            })?;
        Ok(chains
            .into_iter()
            .map(|(chain_id, display_name)| ChainInfo {
                chain_id,
                display_name,
            })
            .collect())
    }
}

pub async fn rpc_handler(
    axum::Extension(module): axum::Extension<Arc<jsonrpsee::server::RpcModule<ChronoRpcImpl>>>,
    body: axum::body::Bytes,
) -> impl axum::response::IntoResponse {
    let req = String::from_utf8_lossy(&body).into_owned();
    let response = module.raw_json_request(&req, 10 * 1024 * 1024).await;
    let json_body = response.map(|(r, _)| r).unwrap_or_else(|e| {
        format!(
            r#"{{"jsonrpc":"2.0","error":{{"code":-32603,"message":{:?}}},"id":null}}"#,
            e.to_string()
        )
    });
    (
        axum::http::HeaderMap::new(),
        axum::response::Response::builder()
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(json_body))
            .unwrap(),
    )
}
