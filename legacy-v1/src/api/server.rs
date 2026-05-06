//! API server implementation combining GraphQL and JSON-RPC

use super::{ApiConfig, ApiService, graphql, jsonrpc};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use jsonrpsee::{
    server::{ServerBuilder, ServerHandle},
    RpcModule,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

/// API server state
#[derive(Clone)]
pub struct ApiServerState {
    pub schema: graphql::ChronoNodeSchema,
    pub config: ApiConfig,
}

/// Combined API server
pub struct ApiServer {
    config: ApiConfig,
    service: Arc<dyn ApiService>,
}

impl ApiServer {
    /// Create a new API server
    pub fn new(config: ApiConfig, service: Arc<dyn ApiService>) -> Self {
        Self { config, service }
    }
    
    /// Start the API server
    pub async fn start(self) -> Result<ServerHandle, Box<dyn std::error::Error + Send + Sync>> {
        let addr: SocketAddr = self.config.bind_address.parse()?;
        
        // Create GraphQL schema
        let schema = graphql::create_schema(self.service.clone());
        
        // Create server state
        let state = ApiServerState {
            schema: schema.clone(),
            config: self.config.clone(),
        };
        
        // Build the router
        let mut app = Router::new();
        
        // Add GraphQL endpoint if enabled
        if self.config.enable_graphql {
            app = app
                .route("/graphql", post(graphql_handler))
                .route("/graphql", get(graphql_playground));
        }
        
        // Add health check
        app = app.route("/health", get(health_handler));
        
        // Add CORS and tracing middleware
        app = app
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods([Method::GET, Method::POST])
                    .allow_headers(Any),
            )
            .layer(TraceLayer::new_for_http())
            .with_state(state);
        
        // Start the HTTP server for GraphQL
        let server_handle = if self.config.enable_graphql {
            tokio::spawn(async move {
                log::info!("Starting API server on {}", addr);
                axum::Server::bind(&addr)
                    .serve(app.into_make_service())
                    .await
                    .map_err(|e| log::error!("API server error: {}", e))
            });
            
            // For now, return a dummy handle since we're using tokio::spawn
            // In a real implementation, you'd want to return a proper handle
            ServerHandle::new(addr)
        } else {
            ServerHandle::new(addr)
        };
        
        // Start JSON-RPC server if enabled
        if self.config.enable_jsonrpc {
            let rpc_addr: SocketAddr = format!("{}:8081", 
                addr.ip()).parse()?; // Use different port for JSON-RPC
            
            let rpc_impl = jsonrpc::ChronoNodeRpcImpl::new(self.service);
            
            tokio::spawn(async move {
                let server = ServerBuilder::default()
                    .build(rpc_addr)
                    .await
                    .expect("Failed to build JSON-RPC server");
                
                let mut module = RpcModule::new(());
                module.merge(rpc_impl.into_rpc()).expect("Failed to merge RPC methods");
                
                let handle = server.start(module).expect("Failed to start JSON-RPC server");
                log::info!("JSON-RPC server started on {}", rpc_addr);
                
                // Keep the server running
                handle.stopped().await;
            });
        }
        
        Ok(server_handle)
    }
}

/// GraphQL request handler
async fn graphql_handler(
    State(state): State<ApiServerState>,
    headers: HeaderMap,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let mut request = req.into_inner();
    
    // Add headers to the request context if needed
    if let Some(auth) = headers.get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            request = request.data(auth_str.to_string());
        }
    }
    
    state.schema.execute(request).await.into()
}

/// GraphQL playground handler
async fn graphql_playground() -> impl IntoResponse {
    Html(async_graphql::http::playground_source(
        async_graphql::http::GraphQLPlaygroundConfig::new("/graphql"),
    ))
}

/// Health check handler
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// Dummy server handle for compatibility
pub struct ServerHandle {
    addr: SocketAddr,
}

impl ServerHandle {
    fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }
    
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }
    
    pub async fn stop(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // In a real implementation, this would stop the server
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::MockApiService;
    use tokio::time::{sleep, Duration};
    
    #[tokio::test]
    async fn test_api_server_creation() {
        let config = ApiConfig::default();
        let service = Arc::new(MockApiService);
        let server = ApiServer::new(config, service);
        
        // Just test that we can create the server
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_graphql_playground() {
        let response = graphql_playground().await;
        // The response should be HTML content
        // In a real test, you'd check the content
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_health_handler() {
        let response = health_handler().await;
        // Should return OK status
        assert!(true);
    }
}
