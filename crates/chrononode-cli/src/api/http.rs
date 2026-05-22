use axum::body::{Body, Bytes};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use crate::archive::pipeline::ArchivePipeline;
use crate::metrics::ApiMetrics;

#[derive(Clone)]
pub struct ApiState {
    pub pipeline: Option<Arc<ArchivePipeline>>,
    pub metrics: ApiMetrics,
    pub api_key: Option<String>,
    pub rate_limiter: RateLimiter,
}

#[derive(Clone)]
pub struct RateLimiter {
    pub max_per_second: u64,
    baseline: Instant,
    state: Arc<std::sync::atomic::AtomicU64>,
}

impl RateLimiter {
    pub fn new(max_per_second: u64) -> Self {
        Self {
            max_per_second: max_per_second.max(1),
            baseline: Instant::now(),
            state: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    pub fn allow(&self) -> bool {
        let now_secs = self.baseline.elapsed().as_secs() as u32;
        let mut current = self.state.load(std::sync::atomic::Ordering::Relaxed);
        loop {
            let current_window = (current >> 32) as u32;
            let current_count = (current & 0xFFFF_FFFF) as u32;

            let (new_window, new_count) = if now_secs > current_window {
                (now_secs, 1)
            } else {
                (current_window, current_count.saturating_add(1))
            };

            if new_count as u64 > self.max_per_second {
                if now_secs == current_window {
                    return false;
                }
            }

            let next = ((new_window as u64) << 32) | (new_count as u64);
            match self.state.compare_exchange_weak(
                current,
                next,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            ) {
                Ok(_) => return new_count as u64 <= self.max_per_second,
                Err(actual) => current = actual,
            }
        }
    }
}

type ApiResult<T> = std::result::Result<Json<T>, (StatusCode, String)>;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
}

#[derive(Serialize, ToSchema)]
pub struct ChainInfo {
    pub chain_id: String,
    pub display_name: String,
}

#[derive(Serialize, ToSchema)]
pub struct BlockResponse {
    pub chain_id: String,
    pub height: u64,
    pub block_hash: String,
    pub timestamp: u64,
    pub tx_count: usize,
    pub event_count: usize,
}

#[derive(Deserialize, ToSchema)]
pub struct RangeQuery {
    pub from: u64,
    pub to: u64,
    pub format: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct VerifyRequest {
    pub proof_json: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
pub struct VerifyResponse {
    pub valid: bool,
}

async fn health(State(state): State<Arc<ApiState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        uptime_seconds: state.metrics.start_time.elapsed().as_secs(),
    })
}

async fn list_chains(State(state): State<Arc<ApiState>>) -> ApiResult<Vec<ChainInfo>> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    let rows = pipeline
        .index
        .get_chain_list()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let chains = rows
        .into_iter()
        .map(|(chain_id, display_name)| ChainInfo {
            chain_id,
            display_name,
        })
        .collect();
    Ok(Json(chains))
}

async fn get_block(
    State(state): State<Arc<ApiState>>,
    Path((chain_id, height)): Path<(String, u64)>,
) -> ApiResult<BlockResponse> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    let block = pipeline
        .get_block_by_height(&chain_id, height)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    state.metrics.increment_blocks_served(&chain_id);
    Ok(Json(BlockResponse {
        chain_id: block.chain_id.clone(),
        height: block.height,
        block_hash: block.block_hash_hex(),
        timestamp: block.timestamp,
        tx_count: block.transactions.len(),
        event_count: block.events.len(),
    }))
}

async fn get_block_range(
    State(state): State<Arc<ApiState>>,
    Path(chain_id): Path<String>,
    Query(range): Query<RangeQuery>,
) -> Response {
    state.metrics.increment_requests();
    if range.to < range.from {
        return (
            StatusCode::BAD_REQUEST,
            "`to` must be greater than or equal to `from`",
        )
            .into_response();
    }
    if (range.to - range.from) >= 1_000 {
        return (
            StatusCode::BAD_REQUEST,
            "range too large; max 1000 blocks per request",
        )
            .into_response();
    }
    let pipeline = match state.pipeline.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (StatusCode::SERVICE_UNAVAILABLE, "pipeline not initialized").into_response();
        }
    };
    let chain_id_clone = chain_id.clone();

    if range.format.as_deref() == Some("ndjson") {
        let stream = stream::iter(range.from..=range.to)
            .then(move |h| {
                let pipeline = pipeline.clone();
                let chain_id = chain_id_clone.clone();
                async move {
                    match pipeline.get_block_by_height(&chain_id, h).await {
                        Ok(b) => {
                            let resp = BlockResponse {
                                chain_id: b.chain_id.clone(),
                                height: b.height,
                                block_hash: b.block_hash_hex(),
                                timestamp: b.timestamp,
                                tx_count: b.transactions.len(),
                                event_count: b.events.len(),
                            };
                            match serde_json::to_string(&resp) {
                                Ok(json) => Ok(format!("{}\n", json)),
                                Err(e) => Err(format!("serialization error: {}\n", e)),
                            }
                        }
                        Err(_) => Err("not found".to_string()),
                    }
                }
            })
            .filter_map(|result| async move { result.ok() });

        Response::new(Body::from_stream(stream.map(|line| {
            Ok::<Bytes, std::convert::Infallible>(Bytes::from(line.into_bytes()))
        })))
    } else {
        let mut blocks = Vec::new();
        for h in range.from..=range.to {
            match pipeline.get_block_by_height(&chain_id, h).await {
                Ok(b) => {
                    state.metrics.increment_blocks_served(&chain_id);
                    blocks.push(BlockResponse {
                        chain_id: b.chain_id.clone(),
                        height: b.height,
                        block_hash: b.block_hash_hex(),
                        timestamp: b.timestamp,
                        tx_count: b.transactions.len(),
                        event_count: b.events.len(),
                    });
                }
                Err(_) => break,
            }
        }
        Json(blocks).into_response()
    }
}

async fn verify_proof_endpoint(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<VerifyRequest>,
) -> Json<VerifyResponse> {
    state.metrics.increment_requests();
    let proof_json = serde_json::from_value(req.proof_json);
    let valid = proof_json
        .as_ref()
        .ok()
        .map(crate::verification::verify_proof_json)
        .unwrap_or(false);
    state.metrics.increment_proofs_verified(valid);
    Json(VerifyResponse { valid })
}

async fn metrics_prometheus() -> String {
    crate::metrics::render_metrics()
}

#[derive(Serialize, ToSchema)]
pub struct ProofResponse {
    #[schema(value_type = Object)]
    pub proof: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
pub struct CheckpointResponse {
    pub checkpoint_id: String,
    pub chain_id: String,
    pub start_height: i64,
    pub end_height: i64,
    pub root_hash: String,
    pub signer_pubkey: Option<String>,
    pub signature: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct LimitQuery {
    pub limit: Option<u64>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

async fn get_block_by_hash(
    State(state): State<Arc<ApiState>>,
    Path((chain_id, block_hash)): Path<(String, String)>,
) -> ApiResult<BlockResponse> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    let block = pipeline
        .get_block_by_hash(&chain_id, &block_hash)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    state.metrics.increment_blocks_served(&chain_id);
    Ok(Json(BlockResponse {
        chain_id: block.chain_id.clone(),
        height: block.height,
        block_hash: block.block_hash_hex(),
        timestamp: block.timestamp,
        tx_count: block.transactions.len(),
        event_count: block.events.len(),
    }))
}

async fn get_block_proof(
    State(state): State<Arc<ApiState>>,
    Path((chain_id, height)): Path<(String, u64)>,
) -> ApiResult<ProofResponse> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    let block = pipeline
        .get_block_by_height(&chain_id, height)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    let location = pipeline
        .index
        .get_block_location(&chain_id, height)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let pointer = chrononode_core::StoragePointer::from_string(&location.1).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid pointer".to_string(),
        )
    })?;
    let cp_config = chrononode_core::CoreConfig::default();
    let builder = crate::verification::checkpoint::CheckpointBuilder::new(cp_config);
    let result = builder
        .build_checkpoint(&[(block, pointer)], &chain_id, height)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let proof = chrononode_core::proof::generate_proof(&result.leaves, 0).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to generate proof".to_string(),
        )
    })?;
    let proof_json = crate::verification::merkle::proof_to_json(
        &proof,
        &result.checkpoint_id,
        result.start_height,
        result.signer_pubkey,
        result.signature,
        None,
        None,
    );
    let proof_value = serde_json::to_value(&proof_json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ProofResponse { proof: proof_value }))
}

async fn get_checkpoint(
    State(state): State<Arc<ApiState>>,
    Path(checkpoint_id): Path<String>,
) -> ApiResult<CheckpointResponse> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    let row = pipeline
        .index
        .get_checkpoint(&checkpoint_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (cp_id, chain_id, start, end, root, pubkey, sig) =
        row.ok_or_else(|| (StatusCode::NOT_FOUND, "checkpoint not found".to_string()))?;
    Ok(Json(CheckpointResponse {
        checkpoint_id: cp_id,
        chain_id,
        start_height: start,
        end_height: end,
        root_hash: hex::encode(root),
        signer_pubkey: pubkey.map(hex::encode),
        signature: sig.map(hex::encode),
    }))
}

async fn auth_middleware(
    State(state): State<Arc<ApiState>>,
    req: axum::extract::Request,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    if let Some(ref expected) = state.api_key {
        let header = req.headers().get("X-API-Key").and_then(|v| v.to_str().ok());
        match header {
            Some(key) if key == expected => {}
            _ => return Err(StatusCode::UNAUTHORIZED),
        }
    }
    Ok(next.run(req).await)
}

async fn rate_limit_middleware(
    State(state): State<Arc<ApiState>>,
    req: axum::extract::Request,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    if !state.rate_limiter.allow() {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    Ok(next.run(req).await)
}

async fn get_txs_by_sender(
    State(state): State<Arc<ApiState>>,
    Path((chain_id, sender)): Path<(String, String)>,
    Query(limit_query): Query<LimitQuery>,
) -> ApiResult<Vec<serde_json::Value>> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    let page = limit_query.page.unwrap_or(1).max(1);
    let per_page = limit_query.per_page.or(limit_query.limit).unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let txs = pipeline
        .index
        .get_txns_by_sender(&chain_id, &sender, per_page, offset)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(txs))
}

async fn get_txs_by_recipient(
    State(state): State<Arc<ApiState>>,
    Path((chain_id, recipient)): Path<(String, String)>,
    Query(limit_query): Query<LimitQuery>,
) -> ApiResult<Vec<serde_json::Value>> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    let page = limit_query.page.unwrap_or(1).max(1);
    let per_page = limit_query.per_page.or(limit_query.limit).unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let txs = pipeline
        .index
        .get_txns_by_recipient(&chain_id, &recipient, per_page, offset)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(txs))
}

async fn get_events_by_type(
    State(state): State<Arc<ApiState>>,
    Path((chain_id, event_type)): Path<(String, String)>,
    Query(limit_query): Query<LimitQuery>,
) -> ApiResult<Vec<serde_json::Value>> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    let page = limit_query.page.unwrap_or(1).max(1);
    let per_page = limit_query.per_page.or(limit_query.limit).unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let events = pipeline
        .index
        .get_events_by_type(&chain_id, &event_type, per_page, offset)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(events))
}

#[derive(Deserialize, ToSchema)]
pub struct CreateCheckpointRequest {
    pub from: u64,
    pub to: u64,
}

#[derive(Serialize, ToSchema)]
pub struct CreateCheckpointResponse {
    pub checkpoint_id: String,
    pub chain_id: String,
    pub start_height: u64,
    pub end_height: u64,
    pub root_hash: String,
    pub leaf_count: u64,
    pub signer_pubkey: Option<String>,
    pub signature: Option<String>,
}

async fn create_checkpoint(
    State(state): State<Arc<ApiState>>,
    Path(chain_id): Path<String>,
    Json(req): Json<CreateCheckpointRequest>,
) -> ApiResult<CreateCheckpointResponse> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;

    if req.to < req.from {
        return Err((
            StatusCode::BAD_REQUEST,
            "`to` must be greater than or equal to `from`".to_string(),
        ));
    }

    let mut blocks_with_pointers = Vec::new();
    for h in req.from..=req.to {
        let block = pipeline
            .get_block_by_height(&chain_id, h)
            .await
            .map_err(|e| (StatusCode::NOT_FOUND, format!("block {}: {}", h, e)))?;
        let location = pipeline
            .index
            .get_block_location(&chain_id, h)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let pointer =
            chrononode_core::StoragePointer::from_string(&location.1).ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "invalid storage pointer".to_string(),
                )
            })?;
        blocks_with_pointers.push((block, pointer));
    }

    let cp_config = chrononode_core::CoreConfig::default();
    let builder = crate::verification::checkpoint::CheckpointBuilder::new(cp_config);
    let result = builder
        .build_checkpoint(&blocks_with_pointers, &chain_id, req.from)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    pipeline
        .index
        .insert_checkpoint(
            &result.checkpoint_id,
            &chain_id,
            result.start_height,
            result.end_height,
            &result.root_hash,
            result.signer_pubkey.as_ref(),
            result.signature.as_ref(),
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    crate::metrics::record_checkpoint_created(&chain_id);

    Ok(Json(CreateCheckpointResponse {
        checkpoint_id: result.checkpoint_id,
        chain_id: result.chain_id,
        start_height: result.start_height,
        end_height: result.end_height,
        root_hash: hex::encode(result.root_hash),
        leaf_count: result.leaf_count,
        signer_pubkey: result.signer_pubkey.map(hex::encode),
        signature: result.signature.map(hex::encode),
    }))
}

#[derive(OpenApi)]
#[openapi(components(schemas(
    HealthResponse,
    ChainInfo,
    BlockResponse,
    ProofResponse,
    CheckpointResponse,
    CreateCheckpointRequest,
    CreateCheckpointResponse,
    RangeQuery,
    VerifyRequest,
    VerifyResponse,
    LimitQuery
)))]
struct ApiDoc;

pub fn build_router(state: Arc<ApiState>) -> Router {
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/v1/chains", get(list_chains))
        .route("/v1/chains/{chain_id}/blocks/{height}", get(get_block))
        .route(
            "/v1/chains/{chain_id}/blocks/hash/{block_hash}",
            get(get_block_by_hash),
        )
        .route("/v1/chains/{chain_id}/blocks", get(get_block_range))
        .route(
            "/v1/chains/{chain_id}/proofs/block/{height}",
            get(get_block_proof),
        )
        .route("/v1/checkpoints/{checkpoint_id}", get(get_checkpoint))
        .route("/v1/chains/{chain_id}/checkpoints", post(create_checkpoint))
        .route("/v1/proofs/verify", post(verify_proof_endpoint))
        .route(
            "/v1/chains/{chain_id}/txs/sender/{sender}",
            get(get_txs_by_sender),
        )
        .route(
            "/v1/chains/{chain_id}/txs/recipient/{recipient}",
            get(get_txs_by_recipient),
        )
        .route(
            "/v1/chains/{chain_id}/events/{event_type}",
            get(get_events_by_type),
        )
        .route("/metrics", get(metrics_prometheus))
        .merge(SwaggerUi::new("/api-docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state.clone());

    router = router.layer(middleware::from_fn_with_state(
        state.clone(),
        rate_limit_middleware,
    ));

    if state.api_key.is_some() {
        router = router.layer(middleware::from_fn_with_state(state, auth_middleware));
    }

    router
}
