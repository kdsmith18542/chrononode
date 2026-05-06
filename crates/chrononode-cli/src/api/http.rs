use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::archive::pipeline::ArchivePipeline;

#[derive(Clone)]
pub struct ApiState {
    pub pipeline: Option<Arc<ArchivePipeline>>,
    pub metrics: MetricsState,
    pub api_key: Option<String>,
}

#[derive(Clone)]
pub struct MetricsState {
    pub requests_total: Arc<AtomicU64>,
    pub blocks_served: Arc<AtomicU64>,
    pub proofs_verified: Arc<AtomicU64>,
    pub start_time: Instant,
}

impl MetricsState {
    pub fn new() -> Self {
        Self {
            requests_total: Arc::new(AtomicU64::new(0)),
            blocks_served: Arc::new(AtomicU64::new(0)),
            proofs_verified: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }
}

type ApiResult<T> = std::result::Result<Json<T>, (StatusCode, String)>;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
}

#[derive(Serialize)]
pub struct ChainInfo {
    pub chain_id: String,
    pub display_name: String,
}

#[derive(Serialize)]
pub struct BlockResponse {
    pub chain_id: String,
    pub height: u64,
    pub block_hash: String,
    pub timestamp: u64,
    pub tx_count: usize,
    pub event_count: usize,
}

#[derive(Deserialize)]
pub struct RangeQuery {
    pub from: u64,
    pub to: u64,
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub proof_json: serde_json::Value,
}

#[derive(Serialize)]
pub struct VerifyResponse {
    pub valid: bool,
}

#[derive(Serialize)]
pub struct MetricsOutput {
    pub requests_total: u64,
    pub blocks_served: u64,
    pub proofs_verified: u64,
    pub uptime_seconds: u64,
}

async fn health(
    State(state): State<Arc<ApiState>>,
) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        uptime_seconds: state.metrics.start_time.elapsed().as_secs(),
    })
}

async fn list_chains(
    State(state): State<Arc<ApiState>>,
) -> ApiResult<Vec<ChainInfo>> {
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    Ok(Json(vec![ChainInfo {
        chain_id: "mock".to_string(),
        display_name: "Mock Chain".to_string(),
    }]))
}

async fn get_block(
    State(state): State<Arc<ApiState>>,
    Path((chain_id, height)): Path<(String, u64)>,
) -> ApiResult<BlockResponse> {
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    state.metrics.blocks_served.fetch_add(1, Ordering::Relaxed);
    let pipeline = state
        .pipeline
        .as_ref()
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, "pipeline not initialized".to_string()))?;
    let block = pipeline
        .get_block_by_height(&chain_id, height)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    let cid = block.chain_id.clone();
    Ok(Json(BlockResponse {
        chain_id: cid,
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
) -> ApiResult<Vec<BlockResponse>> {
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    let pipeline = state
        .pipeline
        .as_ref()
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, "pipeline not initialized".to_string()))?;
    let mut blocks = Vec::new();
    for h in range.from..=range.to {
        match pipeline.get_block_by_height(&chain_id, h).await {
            Ok(b) => {
                let cid = b.chain_id.clone();
                blocks.push(BlockResponse {
                    chain_id: cid,
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
    Ok(Json(blocks))
}

async fn verify_proof_endpoint(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<VerifyRequest>,
) -> Json<VerifyResponse> {
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    state.metrics.proofs_verified.fetch_add(1, Ordering::Relaxed);
    let proof_json = serde_json::from_value(req.proof_json);
    let valid = proof_json
        .as_ref()
        .ok()
        .map(|pj| crate::verification::verify_proof_json(pj))
        .unwrap_or(false);
    Json(VerifyResponse { valid })
}

async fn metrics_endpoint(
    State(state): State<Arc<ApiState>>,
) -> Json<MetricsOutput> {
    Json(MetricsOutput {
        requests_total: state.metrics.requests_total.load(Ordering::Relaxed),
        blocks_served: state.metrics.blocks_served.load(Ordering::Relaxed),
        proofs_verified: state.metrics.proofs_verified.load(Ordering::Relaxed),
        uptime_seconds: state.metrics.start_time.elapsed().as_secs(),
    })
}

async fn auth_middleware(
    State(state): State<Arc<ApiState>>,
    req: axum::extract::Request,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    if let Some(ref expected) = state.api_key {
        let header = req
            .headers()
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok());
        match header {
            Some(key) if key == expected => {}
            _ => return Err(StatusCode::UNAUTHORIZED),
        }
    }
    Ok(next.run(req).await)
}

pub fn build_router(state: Arc<ApiState>) -> Router {
    let mut router = Router::new()
        .route("/health", get(health))
        .route("/v1/chains", get(list_chains))
        .route("/v1/chains/{chain_id}/blocks/{height}", get(get_block))
        .route("/v1/chains/{chain_id}/blocks", get(get_block_range))
        .route("/v1/proofs/verify", post(verify_proof_endpoint))
        .route("/metrics", get(metrics_endpoint))
        .with_state(state.clone());

    if state.api_key.is_some() {
        router = router.layer(middleware::from_fn_with_state(state, auth_middleware));
    }

    router
}
