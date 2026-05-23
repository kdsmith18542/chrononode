use chrononode_adapter_mock::MockAdapter;
use chrononode_cli::archive::pipeline::ArchivePipeline;
use chrononode_cli::archive::serializer::{deserialize_block, serialize_block};
use chrononode_cli::index::sqlite::SqliteIndex;
use chrononode_cli::storage::local_fs::LocalFsBackend;
use chrononode_core::{
    proof::{generate_proof, merkle_root, verify_proof, MerkleLeaf},
    ChainAdapter, ChronoBlock, ChronoEvent, ChronoTx, CoreConfig, PruningConfig, PruningMode,
    StorageBackend,
};
use std::sync::Arc;
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
    let bytes = serialize_block(&block, false).unwrap();
    assert_eq!(bytes[0], 0x00); // 0x00 indicates uncompressed
    let restored = deserialize_block(&bytes).unwrap();
    assert_eq!(restored.height, block.height);
    assert_eq!(restored.chain_id, block.chain_id);
    assert_eq!(restored.block_hash, block.block_hash);
    assert_eq!(restored.transactions.len(), block.transactions.len());
    assert_eq!(restored.events.len(), block.events.len());
}

#[tokio::test]
async fn test_serialize_deserialize_roundtrip_compressed() {
    let block = make_test_block(500);
    let bytes = serialize_block(&block, true).unwrap();
    assert_eq!(bytes[0], 0x01); // 0x01 indicates compressed
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
    let bytes = serialize_block(&block, false).unwrap();
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

#[tokio::test]
async fn test_archival_pruning() {
    let adapter = Arc::new(MockAdapter::new());
    let dir = TempDir::new().unwrap();
    let storage = Arc::new(LocalFsBackend::new(dir.path().to_str().unwrap()));
    let db_path = dir.path().join("index.db");
    let index = SqliteIndex::open(&db_path).await.unwrap();
    index
        .register_chain("mock", "Mock Chain", "mock", "EventLedger")
        .await
        .unwrap();

    let config = CoreConfig {
        checkpoint_size: 10,
        hash_algorithm: "sha256".to_string(),
        repair: Default::default(),
        pruning: PruningConfig {
            mode: PruningMode::Height,
            keep_blocks: 1,
            keep_duration_secs: 0,
            prune_utxos: true,
        },
        compression: true,
        dormancy: Default::default(),
        attestation: Default::default(),
    };

    let pipeline = ArchivePipeline::with_config(adapter, storage, Box::new(index), config);

    // Archive block 0
    let (_block0, pointer0) = pipeline.archive_block(0).await.unwrap();

    // Check that block 0 exists in storage
    let retrieved0 = pipeline.storage.get(&pointer0).await;
    assert!(retrieved0.is_ok());

    // Archive block 1
    let (_block1, pointer1) = pipeline.archive_block(1).await.unwrap();

    // Check that block 1 exists in storage
    let retrieved1 = pipeline.storage.get(&pointer1).await;
    assert!(retrieved1.is_ok());

    // Archive block 2 -> this should trigger pruning block 0
    let (_block2, _pointer2) = pipeline.archive_block(2).await.unwrap();

    // Now, storage for block 0 should be deleted!
    let retrieved0_after = pipeline.storage.get(&pointer0).await;
    assert!(
        retrieved0_after.is_err(),
        "Block 0 should have been pruned from storage"
    );

    // Clear pipeline cache so it has to query the index database
    pipeline.cache.archived.invalidate_all();
    pipeline.cache.by_hash.invalidate_all();

    // Also, the storage pointer in index should be cleared (resulting in NotFound)
    let block0_lookup = pipeline.get_block_by_height("mock", 0).await;
    assert!(
        block0_lookup.is_err(),
        "Block 0 should be not found after pruning"
    );

    // Query index location directly to make sure storage pointer is empty
    let loc = pipeline.index.get_block_location("mock", 0).await.unwrap();
    assert_eq!(loc.1, "", "Storage pointer in index should be empty");
}

use proptest::prelude::*;

fn arb_chrono_tx() -> impl Strategy<Value = ChronoTx> {
    (
        prop::collection::vec(any::<u8>(), 32),
        prop::collection::vec(any::<u8>(), 20..32),
        prop::collection::vec(any::<u8>(), 20..32),
        any::<u64>(),
        any::<u64>(),
        prop::collection::vec(any::<u8>(), 0..100),
        any::<u64>(),
        any::<u64>(),
        prop::collection::vec(any::<u8>(), 0..50),
    )
        .prop_map(
            |(
                tx_hash,
                sender,
                recipient,
                amount,
                nonce,
                payload,
                gas_limit,
                gas_used,
                extra_data,
            )| ChronoTx {
                tx_hash,
                sender,
                recipient,
                amount,
                nonce,
                payload,
                gas_limit,
                gas_used,
                extra_data,
            },
        )
}

fn arb_chrono_event() -> impl Strategy<Value = ChronoEvent> {
    (
        "[a-zA-Z0-9_]{3,15}",
        prop::collection::vec(any::<u8>(), 20..32),
        any::<u64>(),
        any::<u64>(),
        prop::collection::vec(any::<u8>(), 0..100),
    )
        .prop_map(
            |(event_type, emitter, tx_index, event_index, payload)| ChronoEvent {
                event_type,
                emitter,
                tx_index,
                event_index,
                payload,
            },
        )
}

fn arb_chrono_block() -> impl Strategy<Value = ChronoBlock> {
    (
        any::<u32>(),
        "[a-zA-Z0-9_]{3,10}",
        any::<u64>(),
        prop::collection::vec(any::<u8>(), 32),
        prop::collection::vec(any::<u8>(), 32),
        any::<u64>(),
        "[a-zA-Z0-9_]{5,15}",
        "[a-zA-Z0-9_]{3,10}",
        prop::collection::vec(arb_chrono_tx(), 0..5),
        prop::collection::vec(arb_chrono_event(), 0..5),
        prop::collection::vec(any::<u8>(), 0..100),
    )
        .prop_map(
            |(
                schema_version,
                chain_id,
                height,
                block_hash,
                prev_hash,
                timestamp,
                block_model,
                hash_algorithm,
                transactions,
                events,
                extra_data,
            )| ChronoBlock {
                schema_version,
                chain_id,
                height,
                block_hash,
                prev_hash,
                timestamp,
                block_model,
                hash_algorithm,
                transactions,
                events,
                extra_data,
            },
        )
}

proptest! {
    #[test]
    fn test_block_serialization_deserialization_idempotency(
        block in arb_chrono_block(),
        compress in any::<bool>(),
    ) {
        let bytes = serialize_block(&block, compress).unwrap();
        let decompressed = deserialize_block(&bytes).unwrap();
        prop_assert_eq!(decompressed, block);
    }
}
