use axum::body::{Body, Bytes};
use axum::{
    extract::{Path, Query, State},
    http::header::CONTENT_TYPE,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
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
    pub operator_keypair: Option<chrononode_core::OperatorKeypair>,
}

#[derive(Clone)]
pub struct RateLimiter {
    pub max_per_second: u64,
    baseline: Instant,
    state: Arc<std::sync::atomic::AtomicU64>,
}

impl RateLimiter {
    const TOKENS_MASK: u64 = 0xFF_FFFF; // 24 bits
    const TIME_MASK: u64 = 0xFF_FFFF_FFFF; // 40 bits (milliseconds since baseline)

    pub fn new(max_per_second: u64) -> Self {
        // Keep within the 24-bit token field.
        let max_per_second = max_per_second.clamp(1, Self::TOKENS_MASK);
        Self {
            max_per_second,
            baseline: Instant::now(),
            state: Arc::new(std::sync::atomic::AtomicU64::new(max_per_second)),
        }
    }

    pub fn allow(&self) -> bool {
        let now_ms = self.baseline.elapsed().as_millis() as u64 & Self::TIME_MASK;
        self.allow_at_ms(now_ms)
    }

    fn allow_at_ms(&self, now_ms: u64) -> bool {
        let mut current = self.state.load(std::sync::atomic::Ordering::Acquire);
        loop {
            let last_refill_ms = current >> 24;
            let current_tokens = current & Self::TOKENS_MASK;

            let elapsed_ms = if now_ms >= last_refill_ms {
                now_ms - last_refill_ms
            } else {
                ((Self::TIME_MASK + 1) - last_refill_ms) + now_ms
            };

            let refilled_tokens = (self.max_per_second * elapsed_ms) / 1000;
            let new_tokens = current_tokens.saturating_add(refilled_tokens);

            let (next_tokens, next_refill_ms) = if new_tokens >= self.max_per_second {
                (self.max_per_second, now_ms & Self::TIME_MASK)
            } else {
                let time_consumed_ms = (refilled_tokens * 1000) / self.max_per_second;
                (
                    new_tokens,
                    last_refill_ms.wrapping_add(time_consumed_ms) & Self::TIME_MASK,
                )
            };

            if next_tokens == 0 {
                return false;
            }

            let final_tokens = next_tokens - 1;
            let next = (next_refill_ms << 24) | final_tokens;

            match self.state.compare_exchange_weak(
                current,
                next,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }
}

type ApiResult<T> = std::result::Result<Json<T>, (StatusCode, String)>;

static ARTIFACT_POINTER_INDEX: std::sync::OnceLock<tokio::sync::RwLock<HashMap<String, String>>> =
    std::sync::OnceLock::new();

fn artifact_pointer_index() -> &'static tokio::sync::RwLock<HashMap<String, String>> {
    ARTIFACT_POINTER_INDEX.get_or_init(|| tokio::sync::RwLock::new(HashMap::new()))
}

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
}

#[derive(Serialize, Clone, ToSchema)]
pub struct ChainInfo {
    pub chain_id: String,
    pub display_name: String,
}

#[derive(Serialize, Clone, ToSchema)]
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

#[derive(Serialize, ToSchema)]
pub struct ArtifactSubmitResponse {
    pub storage_pointer: String,
    pub content_hash: String,
    pub checkpoint_id: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ArtifactLookupResponse {
    pub storage_pointer: String,
    pub content_hash: String,
    pub checkpoint_id: Option<String>,
}

async fn health(State(state): State<Arc<ApiState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        uptime_seconds: state.metrics.start_time.elapsed().as_secs(),
    })
}

async fn list_chains(
    State(state): State<Arc<ApiState>>,
    Query(limit_query): Query<LimitQuery>,
) -> ApiResult<Vec<ChainInfo>> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    let page = limit_query.page.unwrap_or(1).max(1);
    let per_page = limit_query
        .per_page
        .or(limit_query.limit)
        .unwrap_or(20)
        .min(100);
    let offset = (page - 1) * per_page;

    let rows = pipeline
        .index
        .get_chain_list(per_page, offset)
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

fn parse_content_hash(content_hash: &str) -> Option<&str> {
    let hash_hex = content_hash.strip_prefix("sha256:")?;
    if hash_hex.len() == 64 && hash_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(hash_hex)
    } else {
        None
    }
}

async fn resolve_artifact_pointer(
    state: &ApiState,
    content_hash: &str,
) -> std::result::Result<chrononode_core::StoragePointer, (StatusCode, String)> {
    let hash_hex = parse_content_hash(content_hash).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "invalid content hash; expected sha256:<64-hex>".to_string(),
        )
    })?;

    if let Some(pointer_str) = artifact_pointer_index()
        .read()
        .await
        .get(content_hash)
        .cloned()
    {
        if let Some(pointer) = chrononode_core::StoragePointer::from_string(&pointer_str) {
            return Ok(pointer);
        }
    }

    // Recovery path for local_fs-like deterministic pointers.
    let local_fs_pointer = chrononode_core::StoragePointer::new("local_fs", hash_hex.to_string());
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    if pipeline.storage.get(&local_fs_pointer).await.is_ok() {
        let pointer_str = local_fs_pointer.to_string();
        artifact_pointer_index()
            .write()
            .await
            .insert(content_hash.to_string(), pointer_str);
        return Ok(local_fs_pointer);
    }

    Err((StatusCode::NOT_FOUND, "artifact not found".to_string()))
}

async fn submit_artifact(
    State(state): State<Arc<ApiState>>,
    body: Bytes,
) -> ApiResult<ArtifactSubmitResponse> {
    state.metrics.increment_requests();
    if body.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty artifact body".to_string()));
    }
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;

    let hash_hex = hex::encode(Sha256::digest(&body));
    let content_hash = format!("sha256:{}", hash_hex);

    let pointer = pipeline
        .storage
        .put(&body)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    pipeline
        .storage
        .pin(&pointer)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let pointer_str = pointer.to_string();
    artifact_pointer_index()
        .write()
        .await
        .insert(content_hash.clone(), pointer_str.clone());

    Ok(Json(ArtifactSubmitResponse {
        storage_pointer: pointer_str,
        content_hash,
        checkpoint_id: None,
    }))
}

async fn get_artifact_metadata(
    State(state): State<Arc<ApiState>>,
    Path(content_hash): Path<String>,
) -> ApiResult<ArtifactLookupResponse> {
    state.metrics.increment_requests();
    let pointer = resolve_artifact_pointer(&state, &content_hash).await?;
    Ok(Json(ArtifactLookupResponse {
        storage_pointer: pointer.to_string(),
        content_hash,
        checkpoint_id: None,
    }))
}

async fn get_artifact_bytes(
    State(state): State<Arc<ApiState>>,
    Path(content_hash): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    state.metrics.increment_requests();
    let pointer = resolve_artifact_pointer(&state, &content_hash).await?;
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;

    let bytes = pipeline
        .storage
        .get(&pointer)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(bytes))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(response)
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

#[derive(Serialize, ToSchema)]
pub struct CheckpointAnchorResponse {
    pub chain_id: String,
    pub height: u64,
    pub arweave_tx_id: String,
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

async fn get_checkpoint_anchor(
    State(state): State<Arc<ApiState>>,
    Path((chain_id, height)): Path<(String, u64)>,
) -> ApiResult<CheckpointAnchorResponse> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    let tx_id = pipeline
        .index
        .get_checkpoint_anchor(&chain_id, height)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "checkpoint anchor not found".to_string(),
            )
        })?;
    Ok(Json(CheckpointAnchorResponse {
        chain_id,
        height,
        arweave_tx_id: tx_id,
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
    let per_page = limit_query
        .per_page
        .or(limit_query.limit)
        .unwrap_or(20)
        .min(100);
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
    let per_page = limit_query
        .per_page
        .or(limit_query.limit)
        .unwrap_or(20)
        .min(100);
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
    let per_page = limit_query
        .per_page
        .or(limit_query.limit)
        .unwrap_or(20)
        .min(100);
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

#[derive(Serialize, ToSchema)]
pub struct LastSeenResponse {
    pub chain_id: String,
    pub address: String,
    pub last_block_height: Option<u64>,
    pub last_tx_hash: Option<String>,
}

async fn get_address_last_seen(
    State(state): State<Arc<ApiState>>,
    Path((chain_id, address)): Path<(String, String)>,
) -> ApiResult<LastSeenResponse> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;
    let last_seen = pipeline
        .index
        .get_last_seen(&chain_id, &address)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(LastSeenResponse {
        chain_id,
        address,
        last_block_height: last_seen.as_ref().map(|(h, _)| *h),
        last_tx_hash: last_seen.map(|(_, tx)| tx),
    }))
}

#[derive(Serialize, ToSchema)]
pub struct DormancyStatusResponse {
    pub chain_id: String,
    pub address: String,
    pub status: String,
    pub dormant_since_block: Option<u64>,
    pub threshold_blocks: Option<u64>,
    pub determined_at_block: Option<u64>,
    pub last_block_height: Option<u64>,
    pub last_tx_hash: Option<String>,
}

async fn get_dormancy_status(
    State(state): State<Arc<ApiState>>,
    Path((chain_id, address)): Path<(String, String)>,
) -> ApiResult<DormancyStatusResponse> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;

    let dormant = pipeline
        .index
        .get_dormancy_status(&chain_id, &address)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let last_seen = pipeline
        .index
        .get_last_seen(&chain_id, &address)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (status_str, dormant_since, threshold, determined) = match dormant {
        Some((s, t, d)) => ("dormant".to_string(), Some(s), Some(t), Some(d)),
        None => ("active".to_string(), None, None, None),
    };

    Ok(Json(DormancyStatusResponse {
        chain_id,
        address,
        status: status_str,
        dormant_since_block: dormant_since,
        threshold_blocks: threshold,
        determined_at_block: determined,
        last_block_height: last_seen.as_ref().map(|(h, _)| *h),
        last_tx_hash: last_seen.map(|(_, tx)| tx),
    }))
}

#[derive(Serialize, ToSchema)]
pub struct DormancyProofResponse {
    #[schema(value_type = Object)]
    pub proof: chrononode_core::DormancyProof,
}

#[derive(Deserialize, ToSchema)]
pub struct DormancyProofQuery {
    pub evm_wallet: Option<String>,
}

async fn get_dormancy_proof(
    State(state): State<Arc<ApiState>>,
    Path((chain_id, address)): Path<(String, String)>,
    Query(query): Query<DormancyProofQuery>,
) -> ApiResult<DormancyProofResponse> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;

    let keypair = state.operator_keypair.clone().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "no operator keypair configured".to_string(),
        )
    })?;

    let dormant = pipeline
        .index
        .get_dormancy_status(&chain_id, &address)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("address {} is not dormant on chain {}", address, chain_id),
            )
        })?;

    let current_block = pipeline
        .get_adapter()
        .await
        .latest_height()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut proof = chrononode_core::DormancyProof {
        version: "chrononode:dormancy:v1".to_string(),
        chain_id: chain_id.clone(),
        address: address.clone(),
        dormant_since_block: dormant.0,
        current_block,
        threshold_blocks: dormant.1,
        signer_pubkey: None,
        signature: None,
        evm_wallet: query.evm_wallet,
        proof_type: "ed25519".to_string(),
        zk_proof: None,
        public_inputs: None,
    };
    proof.sign(&keypair);

    Ok(Json(DormancyProofResponse { proof }))
}

#[derive(Deserialize, ToSchema)]
pub struct Sp1ProofQuery {
    pub mock: Option<bool>,
}

#[allow(unused_variables, unused_mut)]
async fn get_sp1_proof(
    State(state): State<Arc<ApiState>>,
    Path(address): Path<String>,
    Query(query): Query<Sp1ProofQuery>,
) -> ApiResult<DormancyProofResponse> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;

    let adapter = pipeline.get_adapter().await;
    let chain_id = adapter.chain_id().to_string();

    let dormant = pipeline
        .index
        .get_dormancy_status(&chain_id, &address)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("address {} is not dormant on chain {}", address, chain_id),
            )
        })?;

    let current_block = pipeline
        .latest_archived_height(&chain_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "No blocks archived for this chain".to_string(),
            )
        })?;

    let dormant_since_block = dormant.0;
    let threshold_blocks = dormant.1;

    if current_block < dormant_since_block {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Current block {} is less than dormant since block {}",
                current_block, dormant_since_block
            ),
        ));
    }
    let diff = current_block - dormant_since_block;
    if diff < threshold_blocks {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Dormancy window ({} blocks) does not satisfy the threshold ({} blocks)",
                diff, threshold_blocks
            ),
        ));
    }

    #[cfg(feature = "zkvm")]
    let mut blocks: Vec<chrononode_core::zkvm::BlockSummary> = Vec::new();
    #[cfg(not(feature = "zkvm"))]
    let mut blocks: Vec<()> = Vec::new();
    for height in dormant_since_block..=current_block {
        let block = pipeline
            .get_block_by_height(&chain_id, height)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        #[cfg(feature = "zkvm")]
        let transactions = block
            .transactions
            .iter()
            .map(|tx| chrononode_core::zkvm::TxSummary {
                sender: chrononode_core::zkvm::bytes_to_address(&chain_id, &tx.sender),
                recipient: chrononode_core::zkvm::bytes_to_address(&chain_id, &tx.recipient),
            })
            .collect();
        #[cfg(not(feature = "zkvm"))]
        let transactions: Vec<()> = Vec::new();

        #[cfg(feature = "zkvm")]
        blocks.push(chrononode_core::zkvm::BlockSummary {
            height: block.height,
            block_hash: block.block_hash_hex(),
            prev_hash: block.prev_hash_hex(),
            transactions,
        });
    }

    #[cfg(feature = "zkvm")]
    {
        let input = chrononode_core::zkvm::GuestInput {
            chain_id: chain_id.clone(),
            address: address.clone(),
            dormant_since_block,
            current_block,
            threshold_blocks,
            blocks,
        };

        let mock = query.mock.unwrap_or(false);

        let (zk_proof, public_inputs) = if mock {
            chrononode_core::zkvm::generate_sp1_proof(&[], &input, true)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        } else {
            let elf_path = std::env::var("CHRONONODE_SP1_ELF").unwrap_or_else(|_| {
                let relative = "crates/chrononode-zkvm-program/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/chrononode-zkvm-program";
                if std::path::Path::new(relative).exists() {
                    relative.to_string()
                } else {
                    "../chrononode-zkvm-program/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/chrononode-zkvm-program".to_string()
                }
            });
            let elf = std::fs::read(&elf_path).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to read SP1 ELF from {}: {}. Make sure to run `cargo prove build` in crates/chrononode-zkvm-program first.", elf_path, e),
                )
            })?;
            chrononode_core::zkvm::generate_sp1_proof(&elf, &input, false)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        };

        let proof = chrononode_core::DormancyProof {
            version: "chrononode:dormancy:v1".to_string(),
            chain_id,
            address,
            dormant_since_block,
            current_block,
            threshold_blocks,
            signer_pubkey: None,
            signature: None,
            evm_wallet: None,
            proof_type: "sp1_groth16".to_string(),
            zk_proof: Some(zk_proof),
            public_inputs: Some(public_inputs),
        };

        Ok(Json(DormancyProofResponse { proof }))
    }

    #[cfg(not(feature = "zkvm"))]
    {
        let _ = address;
        let _ = query;
        let _ = chain_id;
        let _ = dormant_since_block;
        let _ = threshold_blocks;
        let _ = blocks;
        Err((
            StatusCode::NOT_IMPLEMENTED,
            "zkVM feature is not enabled. Build with --features zkvm to enable SP1 proving."
                .to_string(),
        ))
    }
}

#[derive(Deserialize, ToSchema)]
pub struct AttestationSubmitRequest {
    pub chain_id: String,
    pub address: String,
    pub evm_wallet: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct AttestationSubmitResponse {
    pub status: String,
    pub tx_hash: Option<String>,
    pub message: String,
}

#[derive(Serialize, ToSchema)]
pub struct AttestationEntry {
    pub chain_id: String,
    pub address: String,
    pub dormant_since_block: i64,
    pub baals_tx_hash: Option<String>,
    pub status: String,
    pub submitted_at: Option<i64>,
}

#[derive(Deserialize, ToSchema)]
pub struct AttestationListQuery {
    pub chain_id: Option<String>,
}

async fn list_attestations(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<AttestationListQuery>,
) -> ApiResult<Vec<AttestationEntry>> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;

    let chain_id = query.chain_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "chain_id query parameter is required".to_string(),
        )
    })?;

    let rows = pipeline
        .index
        .list_attestations(&chain_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let entries: Vec<AttestationEntry> = rows
        .into_iter()
        .map(|(address, dormant_since_block, baals_tx_hash, status, submitted_at)| {
            AttestationEntry {
                chain_id: chain_id.clone(),
                address,
                dormant_since_block,
                baals_tx_hash,
                status,
                submitted_at,
            }
        })
        .collect();

    Ok(Json(entries))
}

async fn submit_attestation(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<AttestationSubmitRequest>,
) -> ApiResult<AttestationSubmitResponse> {
    state.metrics.increment_requests();
    let pipeline = state.pipeline.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "pipeline not initialized".to_string(),
        )
    })?;

    let dormant = pipeline
        .index
        .get_dormancy_status(&req.chain_id, &req.address)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!(
                    "address {} is not dormant on chain {}",
                    req.address, req.chain_id
                ),
            )
        })?;

    let current = pipeline
        .get_adapter()
        .await
        .latest_height()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let config = chrononode_core::CoreConfig::default();
    let submitter = crate::attestation::BaalsSubmitter::new(&config);

    if !submitter.is_configured() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "BaaLS submitter not configured".to_string(),
        ));
    }

    let proof = chrononode_core::DormancyProof {
        version: "chrononode:dormancy:v1".to_string(),
        chain_id: req.chain_id.clone(),
        address: req.address.clone(),
        dormant_since_block: dormant.0,
        current_block: current,
        threshold_blocks: dormant.1,
        signer_pubkey: None,
        signature: None,
        evm_wallet: req.evm_wallet,
        proof_type: "ed25519".to_string(),
        zk_proof: None,
        public_inputs: None,
    };

    match submitter
        .submit_dormancy_proof(&proof, pipeline.index.as_ref())
        .await
    {
        Ok(Some(tx_hash)) => Ok(Json(AttestationSubmitResponse {
            status: "submitted".to_string(),
            tx_hash: Some(tx_hash),
            message: "Attestation submitted successfully".to_string(),
        })),
        Ok(None) => Ok(Json(AttestationSubmitResponse {
            status: "already_exists".to_string(),
            tx_hash: None,
            message: "Attestation already exists for this address+block".to_string(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Attestation submission failed: {}", e),
        )),
    }
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
    LimitQuery,
    LastSeenResponse,
    DormancyStatusResponse,
    DormancyProofResponse,
    AttestationSubmitRequest,
    AttestationSubmitResponse,
    ArtifactSubmitResponse,
    ArtifactLookupResponse,
    Sp1ProofQuery
)))]
struct ApiDoc;

pub fn build_router(state: Arc<ApiState>) -> Router {
    use crate::api::rpc::ChronoRpcServer;

    let gql_schema = async_graphql::Schema::build(
        crate::api::graphql::Query,
        async_graphql::EmptyMutation,
        async_graphql::EmptySubscription,
    )
    .data(state.clone())
    .finish();

    let rpc_impl = crate::api::rpc::ChronoRpcImpl {
        state: state.clone(),
    };
    let rpc_module = Arc::new(ChronoRpcServer::into_rpc(rpc_impl));

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
        .route(
            "/v1/chains/{chain_id}/checkpoints/{height}/anchor",
            get(get_checkpoint_anchor),
        )
        .route("/v1/chains/{chain_id}/checkpoints", post(create_checkpoint))
        .route("/v1/proofs/verify", post(verify_proof_endpoint))
        .route("/v1/proofs/{address}/sp1", get(get_sp1_proof))
        .route("/v1/artifacts", post(submit_artifact))
        .route("/v1/artifacts/{content_hash}", get(get_artifact_metadata))
        .route(
            "/v1/artifacts/{content_hash}/bytes",
            get(get_artifact_bytes),
        )
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
        .route(
            "/v1/chains/{chain_id}/addresses/{address}/last-seen",
            get(get_address_last_seen),
        )
        .route(
            "/v1/chains/{chain_id}/addresses/{address}/dormancy",
            get(get_dormancy_status),
        )
        .route(
            "/v1/chains/{chain_id}/addresses/{address}/dormancy/proof",
            get(get_dormancy_proof),
        )
        .route("/v1/attestations/submit", post(submit_attestation))
        .route("/v1/attestations", get(list_attestations))
        .route("/metrics", get(metrics_prometheus))
        .route(
            "/graphql",
            get(crate::api::graphql::graphql_playground).post(crate::api::graphql::graphql_handler),
        )
        .route("/rpc", post(crate::api::rpc::rpc_handler))
        .merge(SwaggerUi::new("/api-docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::Extension(gql_schema))
        .layer(axum::Extension(rpc_module))
        .with_state(state.clone());

    router = router.layer(middleware::from_fn_with_state(
        state.clone(),
        rate_limit_middleware,
    ));

    if state.api_key.is_some() {
        router = router.layer(middleware::from_fn_with_state(state, auth_middleware));
    }

    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers(tower_http::cors::Any);

    router = router.layer(cors);

    router
}

#[cfg(test)]
mod rate_limiter_tests {
    use super::RateLimiter;

    #[test]
    fn deterministic_refill_rate() {
        let limiter = RateLimiter::new(4);

        // Drain initial burst capacity at t=0.
        assert!(limiter.allow_at_ms(0));
        assert!(limiter.allow_at_ms(0));
        assert!(limiter.allow_at_ms(0));
        assert!(limiter.allow_at_ms(0));
        assert!(!limiter.allow_at_ms(0));

        // 4 tokens/sec => 1 token every 250ms.
        assert!(limiter.allow_at_ms(250));
        assert!(!limiter.allow_at_ms(250));

        assert!(limiter.allow_at_ms(500));
        assert!(!limiter.allow_at_ms(500));
    }

    #[test]
    fn deterministic_burst_cap_after_idle() {
        let limiter = RateLimiter::new(3);

        // Consume all tokens.
        assert!(limiter.allow_at_ms(0));
        assert!(limiter.allow_at_ms(0));
        assert!(limiter.allow_at_ms(0));
        assert!(!limiter.allow_at_ms(0));

        // Long idle should refill to max, not beyond max.
        assert!(limiter.allow_at_ms(10_000));
        assert!(limiter.allow_at_ms(10_000));
        assert!(limiter.allow_at_ms(10_000));
        assert!(!limiter.allow_at_ms(10_000));
    }
}
