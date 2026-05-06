//! Metrics collection and reporting for ChronoNode.
//! 
//! This module provides functionality for collecting and exposing metrics
//! about the system's performance and behavior.
//! 
//! # Features
//! - Event processing metrics (counters, latency, errors)
//! - Queue size monitoring
//! - System metrics (CPU, memory, etc.)
//! - Prometheus metrics endpoint
//! - Health check endpoint

pub mod server;

use lazy_static::lazy_static;
use prometheus::{
    self, 
    opts, 
    Registry,
    Encoder, 
    TextEncoder,
    IntCounter, IntCounterVec, IntGauge, IntGaugeVec,
    Histogram, HistogramVec, HistogramOpts,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::RwLock;
use std::time::{Instant, Duration};

use crate::error::Result;

lazy_static! {
    /// Global registry for all metrics
    static ref REGISTRY: Registry = Registry::new();
    
    /// Common labels that will be added to all metrics
    static ref COMMON_LABELS: RwLock<HashMap<String, String>> = RwLock::new(HashMap::new());
    
    /// Tracks the number of events processed, by type and status
    pub static ref EVENT_PROCESSING_COUNTER: IntCounterVec = IntCounterVec::new(
        opts!(
            "chrononode_events_processed_total",
            "Number of events processed by type and status"
        ),
        &["event_type", "status"]
    ).expect("Failed to create EVENT_PROCESSING_COUNTER");

    /// Tracks the latency of event processing, by event type
    pub static ref EVENT_PROCESSING_LATENCY: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "chrononode_event_processing_duration_seconds",
            "Time taken to process events in seconds"
        ).buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.5, 5.0, 10.0]),
        &["event_type"]
    ).expect("Failed to create EVENT_PROCESSING_LATENCY");

    /// Tracks the number of events in the processing queue
    pub static ref EVENT_QUEUE_SIZE: IntGaugeVec = IntGaugeVec::new(
        opts!(
            "chrononode_event_queue_size",
            "Number of events currently in the processing queue"
        ),
        &["queue"]
    ).expect("Failed to create EVENT_QUEUE_SIZE");

    /// Tracks the number of event processing errors, by error type
    pub static ref EVENT_PROCESSING_ERRORS: IntCounterVec = IntCounterVec::new(
        opts!(
            "chrononode_event_processing_errors_total",
            "Number of event processing errors by type"
        ),
        &["error_type"]
    ).expect("Failed to create EVENT_PROCESSING_ERRORS");
    
    /// System uptime in seconds
    pub static ref SYSTEM_UPTIME: IntGauge = IntGauge::new(
        "chrononode_system_uptime_seconds",
        "System uptime in seconds"
    ).expect("Failed to create SYSTEM_UPTIME");
    
    /// System memory usage in bytes
    pub static ref SYSTEM_MEMORY_USAGE: IntGauge = IntGauge::new(
        "chrononode_system_memory_usage_bytes",
        "Current memory usage in bytes"
    ).expect("Failed to create SYSTEM_MEMORY_USAGE");
}

/// Initialize the metrics system with custom labels
pub fn init_metrics_labels(labels: &[(&str, &str)]) {
    let mut common_labels = COMMON_LABELS.write().unwrap();
    common_labels.clear();
    common_labels.extend(
        labels
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string())),
    );
}

// Register all metrics with the registry
fn register_metrics() -> Result<()> {
    let metrics: Vec<Box<dyn prometheus::core::Collector>> = vec![
        Box::new(EVENT_PROCESSING_COUNTER.clone()),
        Box::new(EVENT_PROCESSING_LATENCY.clone()),
        Box::new(EVENT_QUEUE_SIZE.clone()),
        Box::new(EVENT_PROCESSING_ERRORS.clone()),
        Box::new(SYSTEM_UPTIME.clone()),
        Box::new(SYSTEM_MEMORY_USAGE.clone()),
    ];

    for metric in metrics {
        REGISTRY.register(metric)?;
    }

    Ok(())
}

/// Records the start of an event processing operation
#[inline]
pub fn start_event_processing_timer(event_type: &str) -> Instant {
    let _timer = EVENT_PROCESSING_LATENCY
        .with_label_values(&[event_type])
        .start_timer();
    Instant::now()
}

/// Records the completion of an event processing operation
#[inline]
pub fn record_event_processing(
    event_type: &str,
    status: &str,
    start_time: Instant,
) {
    let duration = start_time.elapsed();
    EVENT_PROCESSING_LATENCY
        .with_label_values(&[event_type])
        .observe(duration.as_secs_f64());
    EVENT_PROCESSING_COUNTER
        .with_label_values(&[event_type, status])
        .inc();
}

/// Records an event processing error
#[inline]
pub fn record_event_processing_error(error_type: &str) {
    EVENT_PROCESSING_ERRORS
        .with_label_values(&[error_type])
        .inc();
}

/// Updates the event queue size metric
#[inline]
pub fn update_queue_size(queue_name: &str, size: usize) {
    EVENT_QUEUE_SIZE
        .with_label_values(&[queue_name])
        .set(size as i64);
}

/// Record API request metrics
pub fn record_api_request(api_type: &str, method: &str, execution_time_ms: u64) {
    // For now, just log the metrics
    // In a full implementation, you'd have dedicated API metrics
    log::debug!("API request: {} {} took {}ms", api_type, method, execution_time_ms);
}

/// Start collecting system metrics in the background
fn start_system_metrics_collection() {
    // Only proceed if the system metrics are enabled
    if !cfg!(feature = "system-metrics") {
        return;
    }
    
    // Start a background task to update system metrics
    std::thread::spawn(|| {
        let start_time = std::time::Instant::now();
        
        loop {
            // Update system uptime
            SYSTEM_UPTIME.set(start_time.elapsed().as_secs() as i64);
            
            // Update memory usage (approximate)
            if let Ok(usage) = sys_info::mem_info() {
                let total_used = (usage.total - usage.free - usage.buffers - usage.cached) * 1024; // Convert KB to bytes
                SYSTEM_MEMORY_USAGE.set(total_used as i64);
            }
            
            // Sleep for a short interval
            std::thread::sleep(Duration::from_secs(5));
        }
    });
}

/// Returns the current metrics in Prometheus text format
pub fn gather_metrics() -> Result<String> {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    
    // Gather all metrics from the global registry
    let metric_families = REGISTRY.gather();
    
    // Encode metrics to text format
    encoder.encode(&metric_families, &mut buffer)?;
    
    // Convert to string and return
    String::from_utf8(buffer).map_err(Into::into)
}

/// Initializes the metrics system
/// 
/// # Arguments
/// * `metrics_addr` - Optional socket address to start the metrics server on
/// * `labels` - Optional key-value pairs to add as labels to all metrics
/// 
/// # Returns
/// Returns a handle to the metrics server if started, or an error if initialization fails
pub async fn init_metrics(
    metrics_addr: Option<SocketAddr>,
    labels: &[(&str, &str)],
) -> Result<Option<tokio::task::JoinHandle<()>>> {
    // Register all metrics
    register_metrics()?;
    
    // Initialize common labels if provided
    if !labels.is_empty() {
        init_metrics_labels(labels);
    }
    
    // Start system metrics collection
    start_system_metrics_collection();
    
    // If no address is provided, don't start the server
    let addr = match metrics_addr {
        Some(addr) => addr,
        None => return Ok(None),
    };

    // Start the metrics server in a background task
    let handle = tokio::spawn(async move {
        let server = server::MetricsServer::new(addr);
        if let Err(e) = server.start().await {
            log::error!("Metrics server error: {}", e);
        }
    });
    
    log::info!("Metrics system initialized");
    Ok(Some(handle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr};
    use tokio::runtime::Runtime;

    #[test]
    fn test_metrics_initialization() {
        // Create a runtime for testing
        let rt = Runtime::new().unwrap();
        
        // Test initialization without starting the server
        rt.block_on(async {
            let result = init_metrics(None, &[]).await;
            assert!(result.is_ok());
            
            // Test with a random port (port 0 lets the OS choose an available port)
            let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
            let result = init_metrics(Some(addr), &[("test", "value")]).await;
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_metrics_recording() {
        // Test recording metrics
        let _ = start_event_processing_timer("test_event");
        record_event_processing("test_event", "success", Instant::now());
        record_event_processing_error("test_error");
        update_queue_size("test_queue", 42);
        
        // Verify metrics can be gathered
        let metrics = gather_metrics().unwrap();
        assert!(!metrics.is_empty());
        assert!(metrics.contains("chrononode_events_processed_total"));
        assert!(metrics.contains("chrononode_event_processing_errors_total"));
        assert!(metrics.contains("chrononode_event_queue_size"));
    }
}
