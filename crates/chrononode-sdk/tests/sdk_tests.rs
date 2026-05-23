use chrononode_cli::api::http::{build_router, ApiState, RateLimiter};
use chrononode_cli::archive::pipeline::ArchivePipeline;
use chrononode_cli::index::sqlite::{ArchivedBlockInsert, SqliteIndex};
use chrononode_cli::metrics::ApiMetrics;
use chrononode_cli::storage::{create_backend, BackendConfig, BackendKind};
use chrononode_core::{ChainAdapter, ChronoBlock, ChronoEvent, ChronoTx};
use chrononode_sdk::ChronoNodeClient;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;

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

async fn setup_test_server() -> (String, TempDir) {
    let dir = TempDir::new().unwrap();
    let adapter: Arc<dyn ChainAdapter> = Arc::new(chrononode_adapter_mock::MockAdapter::new());
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
        let bytes = chrononode_cli::archive::serializer::serialize_block(&block, false).unwrap();
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

    let state = Arc::new(ApiState {
        pipeline: Some(pipeline),
        metrics: ApiMetrics::new(),
        api_key: None,
        rate_limiter: RateLimiter::new(1000),
    });

    let app = build_router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{}", addr), dir)
}

#[tokio::test]
async fn test_sdk_health_and_chains() {
    let (url, _dir) = setup_test_server().await;
    let client = ChronoNodeClient::new(&url, None);

    let health = client.health().await.unwrap();
    assert_eq!(health.status, "ok");

    let chains = client.list_chains(None, None).await.unwrap();
    assert_eq!(chains.len(), 1);
    assert_eq!(chains[0].chain_id, "mock");
    assert_eq!(chains[0].display_name, "Mock Chain");

    // Test with pagination
    let paginated_chains = client.list_chains(Some(1), Some(1)).await.unwrap();
    assert_eq!(paginated_chains.len(), 1);

    let empty_chains = client.list_chains(Some(2), Some(1)).await.unwrap();
    assert!(empty_chains.is_empty());
}

#[tokio::test]
async fn test_sdk_get_block() {
    let (url, _dir) = setup_test_server().await;
    let client = ChronoNodeClient::new(&url, None);

    let block = client.get_block_by_height("mock", 2).await.unwrap();
    assert_eq!(block.height, 2);
    assert_eq!(block.chain_id, "mock");

    let block_by_hash = client.get_block_by_hash("mock", &block.block_hash).await.unwrap();
    assert_eq!(block_by_hash.height, 2);
}

#[tokio::test]
async fn test_sdk_get_block_range() {
    let (url, _dir) = setup_test_server().await;
    let client = ChronoNodeClient::new(&url, None);

    let blocks_json = client.get_block_range("mock", 0, 2, None).await.unwrap();
    assert_eq!(blocks_json.len(), 3);
    assert_eq!(blocks_json[0]["height"], 0);

    let blocks_ndjson = client.get_block_range("mock", 0, 2, Some("ndjson")).await.unwrap();
    assert_eq!(blocks_ndjson.len(), 3);
    assert_eq!(blocks_ndjson[1]["height"], 1);
}

#[tokio::test]
async fn test_sdk_proof_verification() {
    let (url, _dir) = setup_test_server().await;
    let client = ChronoNodeClient::new(&url, None);

    let proof = client.get_block_proof("mock", 0).await.unwrap();
    assert_eq!(proof.height, 0);

    // Verify via API
    let api_valid = client.verify_proof_api(&proof).await.unwrap();
    assert!(api_valid);

    // Verify locally (client-side verification)
    let local_valid = client.verify_proof_locally(&proof);
    assert!(local_valid);

    // Corrupt proof and verify it fails locally
    let mut corrupted = proof.clone();
    corrupted.block_hash = hex::encode(vec![0xff; 32]);
    let local_invalid = client.verify_proof_locally(&corrupted);
    assert!(!local_invalid);
}
