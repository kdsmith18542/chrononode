//! HTTP server for exposing Prometheus metrics

use axum::{
    routing::get,
    Router,
    response::IntoResponse,
    http::StatusCode,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Metrics server state
#[derive(Clone)]
pub struct MetricsServer {
    addr: SocketAddr,
}

impl MetricsServer {
    /// Create a new metrics server
    pub fn new(addr: impl Into<SocketAddr>) -> Self {
        Self { addr: addr.into() }
    }

    /// Start the metrics server
    pub async fn start(self) -> Result<(), anyhow::Error> {
        // Build our application with a route
        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .route("/health", get(health_handler));

        // Run the server
        log::info!("Starting metrics server on {}", self.addr);
        axum::Server::bind(&self.addr)
            .serve(app.into_make_service())
            .await?;

        Ok(())
    }
}

/// Handler for the /metrics endpoint
async fn metrics_handler() -> impl IntoResponse {
    match crate::metrics::gather_metrics() {
        Ok(metrics) => (StatusCode::OK, metrics),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to gather metrics: {}", e),
        ),
    }
}

/// Handler for the /health endpoint
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_health_endpoint() {
        // Create a test server on a random port
        let addr = SocketAddr::from_str("127.0.0.1:0").unwrap();
        let server = MetricsServer::new(addr);
        
        // Start the server in the background
        let server_handle = tokio::spawn(server.start());
        
        // Give the server a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Test the health endpoint
        let client = reqwest::Client::new();
        let response = client
            .get(&format!("http://{}/health", addr))
            .send()
            .await
            .unwrap();
            
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "OK");
        
        // Clean up
        server_handle.abort();
    }
}
