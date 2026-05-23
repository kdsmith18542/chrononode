use metrics_exporter_prometheus::PrometheusHandle;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();
static START_TIME: OnceLock<Instant> = OnceLock::new();

pub fn start_time() -> Instant {
    *START_TIME.get_or_init(Instant::now)
}

pub fn install_prometheus_recorder() {
    let handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus recorder");
    PROMETHEUS_HANDLE.set(handle).ok();
}

pub fn render_metrics() -> String {
    match PROMETHEUS_HANDLE.get() {
        Some(handle) => handle.render(),
        None => "# no prometheus handle installed\n".to_string(),
    }
}

pub fn record_request() {
    metrics::counter!("chrononode_requests_total").increment(1);
}

pub fn record_block_served(chain_id: &str) {
    metrics::counter!("chrononode_blocks_served_total", "chain" => chain_id.to_string())
        .increment(1);
}

pub fn record_proof_verified(valid: bool) {
    metrics::counter!("chrononode_proofs_verified_total", "valid" => valid.to_string())
        .increment(1);
}

pub fn record_block_archived(chain_id: &str, height: u64) {
    metrics::counter!("chrononode_blocks_archived_total", "chain" => chain_id.to_string())
        .increment(1);
    metrics::gauge!("chrononode_archive_depth", "chain" => chain_id.to_string()).set(height as f64);
}

pub fn record_ingest_error(chain_id: &str) {
    metrics::counter!("chrononode_ingest_errors_total", "chain" => chain_id.to_string())
        .increment(1);
}

pub fn record_storage_operation(
    backend: &str,
    operation: &str,
    success: bool,
    duration: std::time::Duration,
) {
    metrics::counter!(
        "chrononode_storage_operations_total",
        "backend" => backend.to_string(),
        "operation" => operation.to_string(),
        "success" => success.to_string()
    )
    .increment(1);

    metrics::histogram!(
        "chrononode_storage_operation_duration_seconds",
        "backend" => backend.to_string(),
        "operation" => operation.to_string()
    )
    .record(duration.as_secs_f64());
}

pub fn record_checkpoint_created(chain_id: &str) {
    metrics::counter!("chrononode_checkpoints_created_total", "chain" => chain_id.to_string())
        .increment(1);
}

pub fn record_dormancy_detected(chain_id: &str) {
    metrics::counter!("chrononode_dormancy_detections_total", "chain" => chain_id.to_string())
        .increment(1);
}

pub fn record_attestation_submitted(chain_id: &str, success: bool) {
    metrics::counter!(
        "chrononode_attestations_submitted_total",
        "chain" => chain_id.to_string(),
        "success" => success.to_string()
    )
    .increment(1);
}

pub fn record_watchlist_size(chain_id: &str, size: usize) {
    metrics::gauge!("chrononode_watchlist_size", "chain" => chain_id.to_string()).set(size as f64);
}

pub fn record_dormant_count(chain_id: &str, count: usize) {
    metrics::gauge!("chrononode_dormant_addresses", "chain" => chain_id.to_string())
        .set(count as f64);
}

#[derive(Clone)]
pub struct ApiMetrics {
    pub requests_total: Arc<AtomicU64>,
    pub blocks_served: Arc<AtomicU64>,
    pub proofs_verified: Arc<AtomicU64>,
    pub start_time: Instant,
}

impl ApiMetrics {
    pub fn new() -> Self {
        Self {
            requests_total: Arc::new(AtomicU64::new(0)),
            blocks_served: Arc::new(AtomicU64::new(0)),
            proofs_verified: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    pub fn increment_requests(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        record_request();
    }

    pub fn increment_blocks_served(&self, chain_id: &str) {
        self.blocks_served.fetch_add(1, Ordering::Relaxed);
        record_block_served(chain_id);
    }

    pub fn increment_proofs_verified(&self, valid: bool) {
        self.proofs_verified.fetch_add(1, Ordering::Relaxed);
        record_proof_verified(valid);
    }
}

impl Default for ApiMetrics {
    fn default() -> Self {
        Self::new()
    }
}
