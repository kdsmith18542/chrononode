use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrononode_adapter_mock::MockAdapter;
use chrononode_cli::api::http::{build_router, ApiState, RateLimiter};
use chrononode_cli::archive::pipeline::ArchivePipeline;
use chrononode_cli::index::sqlite::{ArchivedBlockInsert, SqliteIndex};
use chrononode_cli::metrics::ApiMetrics;
use chrononode_cli::storage::{create_backend, BackendConfig, BackendKind};
use chrononode_core::{ChainAdapter, ChronoBlock, ChronoEvent, ChronoTx};
use http_body_util::BodyExt;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

fn make_test_block(height: u64) -> ChronoBlock {
    ChronoBlock {
        schema_version: 1,
        chain_id: "mock".to_string(),
        height,
        block_hash: vec![height as u8; 32],
        prev_hash: if height == 0 {
            vec![0u8; 32]
        } else {
            vec![(height - 1) as u8; 32]
        },
        timestamp: 1700000000 + height,
        block_model: "EventLedger".to_string(),
        hash_algorithm: "sha256".to_string(),
        transactions: vec![ChronoTx {
            tx_hash: {
                let mut h = vec![height as u8; 31];
                h.push(0x11);
                h
            },
            sender: vec![0x01; 32],
            recipient: vec![0x02; 32],
            amount: 1000 + height,
            nonce: height,
            payload: vec![],
            gas_limit: 21000,
            gas_used: 21000,
            extra_data: vec![],
        }],
        events: vec![ChronoEvent {
            event_type: if height % 2 == 0 { "transfer" } else { "swap" }.to_string(),
            emitter: vec![0xaa; 32],
            tx_index: 0,
            event_index: 0,
            payload: vec![],
        }],
        extra_data: vec![],
    }
}

async fn setup_test_state() -> (Arc<ArchivePipeline>, TempDir) {
    let dir = TempDir::new().unwrap();
    let adapter: Arc<dyn ChainAdapter> = Arc::new(MockAdapter::new());
    let storage = create_backend(
        BackendKind::LocalFs,
        &BackendConfig::from_env(dir.path().to_str().unwrap()),
    );
    let db_path = dir.path().join("index.db");
    let index = SqliteIndex::open(&db_path).await.unwrap();
    index
        .register_chain("mock", "Mock Chain", "mock", "EventLedger")
        .await
        .unwrap();

    let pipeline = Arc::new(ArchivePipeline::new(adapter, storage, Box::new(index)));

    for h in 0..5 {
        let block = make_test_block(h);
        let bytes = chrononode_cli::archive::serializer::serialize_block(&block).unwrap();
        let pointer = pipeline.storage.put(&bytes).await.unwrap();
        pipeline.storage.pin(&pointer).await.unwrap();
        let pointer_str = pointer.to_string();

        let insert = ArchivedBlockInsert {
            chain_id: &block.chain_id,
            height: h,
            block_hash: &block.block_hash,
            block_hash_hex: &block.block_hash_hex(),
            prev_hash: &block.prev_hash,
            storage_backend: &pointer.backend,
            storage_pointer: &pointer_str,
            timestamp: block.timestamp,
            byte_size: bytes.len() as u64,
        };
        pipeline
            .index
            .archive_block_atomic(&insert, &block.transactions, &block.events)
            .await
            .unwrap();
    }

    (pipeline, dir)
}

async fn body_to_string(body: Body) -> String {
    let bytes = body.collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn test_api_health_endpoint() {
    let (_pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: None,
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_list_chains() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chains")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    assert!(body.contains("mock"));
    assert!(body.contains("Mock Chain"));
}

#[tokio::test]
async fn test_api_get_block_by_height() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chains/mock/blocks/2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    assert!(body.contains("\"height\":2"));
    assert!(body.contains("\"tx_count\":1"));
}

#[tokio::test]
async fn test_api_get_block_not_found() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chains/mock/blocks/999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_api_get_block_by_hash() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let block_hash = hex::encode(vec![2u8; 32]);
    let uri = format!("/v1/chains/mock/blocks/hash/{}", block_hash);
    let response = app
        .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_block_range_json() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chains/mock/blocks?from=0&to=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    let blocks: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0]["height"], 0);
    assert_eq!(blocks[2]["height"], 2);
}

#[tokio::test]
async fn test_api_block_range_ndjson() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chains/mock/blocks?from=0&to=2&format=ndjson")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    let lines: Vec<&str> = body.trim().split('\n').collect();
    assert_eq!(lines.len(), 3);
    for line in lines {
        let block: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(block.get("height").is_some());
    }
}

#[tokio::test]
async fn test_api_block_range_too_large() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chains/mock/blocks?from=0&to=2000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_get_block_proof() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chains/mock/proofs/block/0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    assert!(body.contains("proof"));
}

#[tokio::test]
async fn test_api_verify_proof() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let proof_body = serde_json::json!({
        "proof_json": {
            "leaves": [],
            "target_index": 0,
            "proof_hashes": [],
            "root_hash": "0000000000000000000000000000000000000000000000000000000000000000"
        }
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/proofs/verify")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&proof_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    assert!(body.contains("valid"));
}

#[tokio::test]
async fn test_api_txs_by_sender() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let sender_hex = hex::encode(vec![0x01; 32]);
    let uri = format!("/v1/chains/mock/txs/sender/{}?limit=10", sender_hex);
    let response = app
        .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    let txs: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(txs.len(), 5);
    assert_eq!(txs[0]["sender"], sender_hex);
}

#[tokio::test]
async fn test_api_txs_by_recipient() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let recipient_hex = hex::encode(vec![0x02; 32]);
    let uri = format!("/v1/chains/mock/txs/recipient/{}?limit=3", recipient_hex);
    let response = app
        .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    let txs: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(txs.len(), 3);
}

#[tokio::test]
async fn test_api_events_by_type() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chains/mock/events/transfer?limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    let events: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(events.len(), 3);
    for event in &events {
        assert_eq!(event["event_type"], "transfer");
    }
}

#[tokio::test]
async fn test_api_auth_middleware_rejects_missing_key() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: Some("secret-key".to_string()),
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_api_auth_middleware_accepts_valid_key() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: Some("secret-key".to_string()),
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("X-API-Key", "secret-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_openapi_docs_available() {
    let (_pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: None,
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api-docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    assert!(body.contains("openapi"));
}

#[tokio::test]
async fn test_api_pagination() {
    let (pipeline, _dir) = setup_test_state().await;
    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });
    let app = build_router(state);

    let sender_hex = hex::encode(vec![0x01; 32]);

    // Page 1, per_page 2 -> elements 0 and 1
    let uri = format!("/v1/chains/mock/txs/sender/{}?page=1&per_page=2", sender_hex);
    let response = app.clone()
        .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    let txs1: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(txs1.len(), 2);

    // Page 2, per_page 2 -> elements 2 and 3
    let uri = format!("/v1/chains/mock/txs/sender/{}?page=2&per_page=2", sender_hex);
    let response = app.clone()
        .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    let txs2: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(txs2.len(), 2);

    // Assert that elements are different
    assert_ne!(txs1[0]["tx_hash"], txs2[0]["tx_hash"]);

    // Page 3, per_page 2 -> element 4
    let uri = format!("/v1/chains/mock/txs/sender/{}?page=3&per_page=2", sender_hex);
    let response = app.clone()
        .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    let txs3: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(txs3.len(), 1);

    // Page 4, per_page 2 -> empty
    let uri = format!("/v1/chains/mock/txs/sender/{}?page=4&per_page=2", sender_hex);
    let response = app.clone()
        .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_string(response.into_body()).await;
    let txs4: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(txs4.len(), 0);
}
