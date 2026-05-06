use chrononode_core::ChainAdapter;
use chrononode_cli::adapters::mock::MockAdapter;

#[tokio::test]
async fn test_mock_adapter_basic() {
    let adapter = MockAdapter::new();
    assert_eq!(adapter.chain_id(), "mock");
    assert_eq!(adapter.display_name(), "Mock Chain");

    let height = adapter.latest_height().await.unwrap();
    assert_eq!(height, 9999);

    let block = adapter.fetch_block(42).await.unwrap();
    assert_eq!(block.height, 42);
    assert_eq!(block.chain_id, "mock");
    assert_eq!(block.transactions.len(), 1);
    assert_eq!(block.events.len(), 1);
}

#[tokio::test]
async fn test_mock_adapter_range() {
    let adapter = MockAdapter::new();
    let blocks = adapter.fetch_range(0, 9).await.unwrap();
    assert_eq!(blocks.len(), 10);
    for (i, b) in blocks.iter().enumerate() {
        assert_eq!(b.height, i as u64);
    }
}

#[tokio::test]
async fn test_mock_block_has_expected_hash_format() {
    let adapter = MockAdapter::new();
    let block = adapter.fetch_block(100).await.unwrap();
    assert_eq!(block.block_hash.len(), 32, "hash should be 32 bytes");
    assert_eq!(block.schema_version, 1);
}
