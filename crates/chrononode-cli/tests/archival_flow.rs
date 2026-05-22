use chrononode_adapter_mock::MockAdapter;
use chrononode_cli::archive::serializer::{deserialize_block, serialize_block};
use chrononode_cli::storage::local_fs::LocalFsBackend;
use chrononode_core::{
    proof::{generate_proof, merkle_root, verify_proof, MerkleLeaf},
    ChainAdapter, ChronoBlock, ChronoEvent, ChronoTx, StorageBackend,
};
use tempfile::TempDir;

fn make_test_block(height: u64) -> ChronoBlock {
    ChronoBlock {
        schema_version: 1,
        chain_id: "test".to_string(),
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
            tx_hash: vec![height as u8; 32],
            sender: vec![0x01; 32],
            recipient: vec![0x02; 32],
            amount: 1000,
            nonce: height,
            payload: vec![],
            gas_limit: 21000,
            gas_used: 21000,
            extra_data: vec![],
        }],
        events: vec![ChronoEvent {
            event_type: "test".to_string(),
            emitter: vec![0xaa; 32],
            tx_index: 0,
            event_index: 0,
            payload: vec![],
        }],
        extra_data: vec![],
    }
}

#[tokio::test]
async fn test_serialize_deserialize_roundtrip() {
    let block = make_test_block(500);
    let bytes = serialize_block(&block).unwrap();
    let restored = deserialize_block(&bytes).unwrap();
    assert_eq!(restored.height, block.height);
    assert_eq!(restored.chain_id, block.chain_id);
    assert_eq!(restored.block_hash, block.block_hash);
    assert_eq!(restored.transactions.len(), block.transactions.len());
    assert_eq!(restored.events.len(), block.events.len());
}

#[tokio::test]
async fn test_local_fs_put_get() {
    let dir = TempDir::new().unwrap();
    let backend = LocalFsBackend::new(dir.path().to_str().unwrap());
    let data = b"hello chrononode";
    let pointer = backend.put(data).await.unwrap();
    assert_eq!(pointer.backend, "local_fs");
    let retrieved = backend.get(&pointer).await.unwrap();
    assert_eq!(retrieved, data);
}

#[tokio::test]
async fn test_archival_flow_mock_adapter() {
    let adapter = MockAdapter::new();
    let block = adapter.fetch_block(0).await.unwrap();
    let bytes = serialize_block(&block).unwrap();
    let restored = deserialize_block(&bytes).unwrap();
    assert_eq!(restored.height, 0);
    assert_eq!(restored.block_hash, block.block_hash);
}

#[tokio::test]
async fn test_proof_verification_with_real_data() {
    let leaves: Vec<MerkleLeaf> = (0..10)
        .map(|h| {
            let block = make_test_block(h);
            MerkleLeaf::from_block(&block, "local_fs", &format!("test/{}", h))
        })
        .collect();
    let root = merkle_root(&leaves).unwrap();
    assert_eq!(root.len(), 32);
    for i in 0..10 {
        let proof = generate_proof(&leaves, i).unwrap();
        assert!(verify_proof(&proof), "proof failed for index {}", i);
    }
}
