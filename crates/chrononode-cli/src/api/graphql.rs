use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema, SimpleObject, FieldResult};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::response::Html;
use std::sync::Arc;
use crate::api::ApiState;

pub struct Query;

#[derive(SimpleObject)]
pub struct GqlBlock {
    pub chain_id: String,
    pub height: u64,
    pub block_hash: String,
    pub prev_hash: String,
    pub timestamp: u64,
    pub block_model: String,
    pub tx_count: usize,
    pub event_count: usize,
}

#[derive(SimpleObject)]
pub struct GqlChainInfo {
    pub chain_id: String,
    pub display_name: String,
}

#[Object]
impl Query {
    async fn block(&self, ctx: &Context<'_>, chain_id: String, height: u64) -> FieldResult<GqlBlock> {
        let state = ctx.data::<Arc<ApiState>>()?;
        let pipeline = state.pipeline.as_ref().ok_or_else(|| async_graphql::Error::new("pipeline not initialized"))?;
        let block = pipeline.get_block_by_height(&chain_id, height).await?;
        Ok(GqlBlock {
            chain_id: block.chain_id.clone(),
            height: block.height,
            block_hash: block.block_hash_hex(),
            prev_hash: hex::encode(&block.prev_hash),
            timestamp: block.timestamp,
            block_model: block.block_model.clone(),
            tx_count: block.transactions.len(),
            event_count: block.events.len(),
        })
    }

    async fn block_by_hash(&self, ctx: &Context<'_>, chain_id: String, hash: String) -> FieldResult<GqlBlock> {
        let state = ctx.data::<Arc<ApiState>>()?;
        let pipeline = state.pipeline.as_ref().ok_or_else(|| async_graphql::Error::new("pipeline not initialized"))?;
        let block = pipeline.get_block_by_hash(&chain_id, &hash).await?;
        Ok(GqlBlock {
            chain_id: block.chain_id.clone(),
            height: block.height,
            block_hash: block.block_hash_hex(),
            prev_hash: hex::encode(&block.prev_hash),
            timestamp: block.timestamp,
            block_model: block.block_model.clone(),
            tx_count: block.transactions.len(),
            event_count: block.events.len(),
        })
    }

    async fn transactions_by_sender(
        &self,
        ctx: &Context<'_>,
        chain_id: String,
        sender: String,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> FieldResult<Vec<async_graphql::types::Json<serde_json::Value>>> {
        let state = ctx.data::<Arc<ApiState>>()?;
        let pipeline = state.pipeline.as_ref().ok_or_else(|| async_graphql::Error::new("pipeline not initialized"))?;
        let txs = pipeline.index.get_txns_by_sender(
            &chain_id,
            &sender,
            limit.unwrap_or(20),
            offset.unwrap_or(0),
        ).await?;
        Ok(txs.into_iter().map(async_graphql::types::Json).collect())
    }

    async fn transactions_by_recipient(
        &self,
        ctx: &Context<'_>,
        chain_id: String,
        recipient: String,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> FieldResult<Vec<async_graphql::types::Json<serde_json::Value>>> {
        let state = ctx.data::<Arc<ApiState>>()?;
        let pipeline = state.pipeline.as_ref().ok_or_else(|| async_graphql::Error::new("pipeline not initialized"))?;
        let txs = pipeline.index.get_txns_by_recipient(
            &chain_id,
            &recipient,
            limit.unwrap_or(20),
            offset.unwrap_or(0),
        ).await?;
        Ok(txs.into_iter().map(async_graphql::types::Json).collect())
    }

    async fn events_by_type(
        &self,
        ctx: &Context<'_>,
        chain_id: String,
        event_type: String,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> FieldResult<Vec<async_graphql::types::Json<serde_json::Value>>> {
        let state = ctx.data::<Arc<ApiState>>()?;
        let pipeline = state.pipeline.as_ref().ok_or_else(|| async_graphql::Error::new("pipeline not initialized"))?;
        let events = pipeline.index.get_events_by_type(
            &chain_id,
            &event_type,
            limit.unwrap_or(20),
            offset.unwrap_or(0),
        ).await?;
        Ok(events.into_iter().map(async_graphql::types::Json).collect())
    }

    async fn chain_list(
        &self,
        ctx: &Context<'_>,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> FieldResult<Vec<GqlChainInfo>> {
        let state = ctx.data::<Arc<ApiState>>()?;
        let pipeline = state.pipeline.as_ref().ok_or_else(|| async_graphql::Error::new("pipeline not initialized"))?;
        let chains = pipeline.index.get_chain_list(
            limit.unwrap_or(20),
            offset.unwrap_or(0),
        ).await?;
        Ok(chains
            .into_iter()
            .map(|(chain_id, display_name)| GqlChainInfo {
                chain_id,
                display_name,
            })
            .collect())
    }

    async fn stats(&self, ctx: &Context<'_>, chain_id: String) -> FieldResult<async_graphql::types::Json<serde_json::Value>> {
        let state = ctx.data::<Arc<ApiState>>()?;
        let pipeline = state.pipeline.as_ref().ok_or_else(|| async_graphql::Error::new("pipeline not initialized"))?;
        let stats = pipeline.index.get_stats(&chain_id).await?;
        Ok(async_graphql::types::Json(stats))
    }
}

pub type GqlSchema = Schema<Query, EmptyMutation, EmptySubscription>;

pub async fn graphql_handler(
    axum::Extension(schema): axum::Extension<GqlSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

pub async fn graphql_playground() -> Html<String> {
    Html(async_graphql::http::GraphiQLSource::build().endpoint("/graphql").finish())
}
