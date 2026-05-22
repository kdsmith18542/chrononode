use chrononode_cli::archive::pipeline::ArchivePipeline;
use chrononode_cli::index::{open_index, IndexKind};
use chrononode_cli::storage::{create_backend, BackendConfig, BackendKind};
use chrononode_cli::verification::checkpoint::CheckpointBuilder;
use chrononode_core::{
    ChainAdapter, ChronoBlock, ChronoTx, CoreConfig, OperatorKeypair, StoragePointer,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

struct TestAdapter {
    blocks: Vec<ChronoBlock>,
}

impl TestAdapter {
    fn new(count: u64) -> Self {
        let blocks = (0..count).map(make_test_block).collect();
        Self { blocks }
    }
}

#[async_trait::async_trait]
impl ChainAdapter for TestAdapter {
    fn chain_id(&self) -> &str {
        "test"
    }

    fn display_name(&self) -> &str {
        "Test Chain"
    }

    fn block_model(&self) -> chrononode_core::BlockModel {
        chrononode_core::BlockModel::EventLedger
    }

    async fn latest_height(&self) -> chrononode_core::Result<u64> {
        Ok(self.blocks.len() as u64 - 1)
    }

    async fn fetch_block(&self, height: u64) -> chrononode_core::Result<ChronoBlock> {
        self.blocks
            .get(height as usize)
            .cloned()
            .ok_or_else(|| chrononode_core::CoreError::NotFound(format!("block {}", height)))
    }

    async fn fetch_block_by_hash(&self, hash: &[u8]) -> chrononode_core::Result<ChronoBlock> {
        let block = self.blocks.iter().find(|b| b.block_hash == hash).cloned();
        block.ok_or_else(|| {
            chrononode_core::CoreError::NotFound(format!("block with hash {}", hex::encode(hash)))
        })
    }
}

fn make_test_block(height: u64) -> ChronoBlock {
    let mut h = Sha256::default();
    h.update(b"test-block");
    h.update(height.to_be_bytes());
    let block_hash = h.finalize().to_vec();

    ChronoBlock {
        schema_version: 1,
        chain_id: "test".to_string(),
        height,
        block_hash,
        prev_hash: if height == 0 {
            vec![0u8; 32]
        } else {
            let mut h = Sha256::default();
            h.update(b"test-block");
            h.update((height - 1).to_be_bytes());
            h.finalize().to_vec()
        },
        timestamp: 1700000000 + height,
        block_model: "EventLedger".to_string(),
        hash_algorithm: "sha256".to_string(),
        transactions: vec![ChronoTx {
            tx_hash: vec![height as u8; 32],
            sender: vec![0x01; 32],
            recipient: vec![0x02; 32],
            amount: 1000 + height,
            nonce: height,
            payload: vec![],
            gas_limit: 21000,
            gas_used: 21000,
            extra_data: vec![],
        }],
        events: vec![],
        extra_data: vec![],
    }
}

#[tokio::test]
async fn test_checkpoint_creation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("index.db");
    let data_path = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_path).unwrap();

    let adapter: Arc<dyn ChainAdapter> = Arc::new(TestAdapter::new(10));
    let storage = create_backend(
        BackendKind::LocalFs,
        &BackendConfig::from_env(data_path.to_str().unwrap()),
    );
    let index = open_index(IndexKind::Sqlite, &db_path, "postgres://localhost/test")
        .await
        .unwrap();

    index
        .register_chain("test", "Test Chain", "test", "EventLedger")
        .await
        .unwrap();

    let pipeline = ArchivePipeline::new(adapter, storage, index);

    for h in 0..5u64 {
        pipeline.archive_block(h).await.unwrap();
    }

    let keypair = OperatorKeypair::generate();
    let config = CoreConfig::default();
    let builder = CheckpointBuilder::new(config).with_keypair(keypair);

    let mut blocks_with_pointers = Vec::new();
    for h in 0..5u64 {
        let block = pipeline.get_block_by_height("test", h).await.unwrap();
        let location = pipeline
            .index
            .as_ref()
            .get_block_location("test", h)
            .await
            .unwrap();
        let pointer = StoragePointer::from_string(&location.1).unwrap();
        blocks_with_pointers.push((block, pointer));
    }

    let result = builder
        .build_checkpoint(&blocks_with_pointers, "test", 0)
        .unwrap();

    assert_eq!(result.checkpoint_id, "test-0-4");
    assert_eq!(result.start_height, 0);
    assert_eq!(result.end_height, 4);
    assert_eq!(result.leaves.len(), 5);
    assert!(result.signer_pubkey.is_some());
    assert!(result.signature.is_some());

    pipeline
        .index
        .insert_checkpoint(
            &result.checkpoint_id,
            "test",
            result.start_height,
            result.end_height,
            &result.root_hash,
            result.signer_pubkey.as_ref(),
            result.signature.as_ref(),
        )
        .await
        .unwrap();

    let checkpoint = pipeline
        .index
        .as_ref()
        .get_checkpoint("test-0-4")
        .await
        .unwrap();
    assert!(checkpoint.is_some());
    let (id, chain_id, start, end, root, pubkey, sig) = checkpoint.unwrap();
    assert_eq!(id, "test-0-4");
    assert_eq!(chain_id, "test");
    assert_eq!(start, 0);
    assert_eq!(end, 4);
    assert_eq!(root, result.root_hash);
    assert!(pubkey.is_some());
    assert!(sig.is_some());
}

#[tokio::test]
async fn test_checkpoint_single_block() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("index.db");
    let data_path = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_path).unwrap();

    let adapter: Arc<dyn ChainAdapter> = Arc::new(TestAdapter::new(1));
    let storage = create_backend(
        BackendKind::LocalFs,
        &BackendConfig::from_env(data_path.to_str().unwrap()),
    );
    let index = open_index(IndexKind::Sqlite, &db_path, "postgres://localhost/test")
        .await
        .unwrap();

    index
        .register_chain("test", "Test Chain", "test", "EventLedger")
        .await
        .unwrap();

    let pipeline = ArchivePipeline::new(adapter, storage, index);
    pipeline.archive_block(0).await.unwrap();

    let config = CoreConfig::default();
    let builder = CheckpointBuilder::new(config);

    let block = pipeline.get_block_by_height("test", 0).await.unwrap();
    let location = pipeline
        .index
        .as_ref()
        .get_block_location("test", 0)
        .await
        .unwrap();
    let pointer = StoragePointer::from_string(&location.1).unwrap();

    let result = builder
        .build_checkpoint(&[(block, pointer)], "test", 0)
        .unwrap();

    assert_eq!(result.checkpoint_id, "test-0-0");
    assert_eq!(result.leaves.len(), 1);
}

#[test]
fn test_checkpoint_builder_without_keypair() {
    let config = CoreConfig::default();
    let builder = CheckpointBuilder::new(config);

    let block = make_test_block(0);
    let pointer = StoragePointer::from_string("sha256:abc123").unwrap();

    let result = builder
        .build_checkpoint(&[(block, pointer)], "test", 0)
        .unwrap();

    assert_eq!(result.checkpoint_id, "test-0-0");
    assert!(result.signer_pubkey.is_none());
    assert!(result.signature.is_none());
}

#[tokio::test]
async fn test_checkpoint_sign_export_verify_e2e() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("index.db");
    let data_path = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_path).unwrap();

    let adapter: Arc<dyn ChainAdapter> = Arc::new(TestAdapter::new(10));
    let storage = create_backend(
        BackendKind::LocalFs,
        &BackendConfig::from_env(data_path.to_str().unwrap()),
    );
    let index = open_index(IndexKind::Sqlite, &db_path, "postgres://localhost/test")
        .await
        .unwrap();

    index
        .register_chain("test", "Test Chain", "test", "EventLedger")
        .await
        .unwrap();

    let pipeline = ArchivePipeline::new(adapter.clone(), storage, index);

    for h in 0..5u64 {
        pipeline.archive_block(h).await.unwrap();
    }

    // Step 1: Generate keypair
    let keypair = OperatorKeypair::generate();
    let config = CoreConfig::default();
    let builder = CheckpointBuilder::new(config).with_keypair(keypair);

    // Step 2: Create checkpoint with signing
    let mut blocks_with_pointers = Vec::new();
    for h in 0..5u64 {
        let block = pipeline.get_block_by_height("test", h).await.unwrap();
        let location = pipeline
            .index
            .as_ref()
            .get_block_location("test", h)
            .await
            .unwrap();
        let pointer = StoragePointer::from_string(&location.1).unwrap();
        blocks_with_pointers.push((block, pointer));
    }

    let result = builder
        .build_checkpoint(&blocks_with_pointers, "test", 0)
        .unwrap();

    assert!(result.signer_pubkey.is_some(), "checkpoint should be signed");
    assert!(result.signature.is_some(), "checkpoint should have a signature");

    // Step 3: Export a proof for block 2
    let proof = chrononode_core::proof::generate_proof(&result.leaves, 2).unwrap();
    let proof_json = chrononode_cli::verification::merkle::proof_to_json(
        &proof,
        &result.checkpoint_id,
        result.start_height,
        result.signer_pubkey,
        result.signature,
        None,
        None,
    );

    // Step 4: Verify the proof
    let valid = chrononode_cli::verification::merkle::verify_proof_json(&proof_json);
    assert!(valid, "signed checkpoint proof should verify");

    // Step 5: Verify that tampering with the proof causes verification failure
    let mut tampered = proof_json.clone();
    if let Some(pubkey) = &mut tampered.checkpoint.signer_pubkey {
        let mut bytes = hex::decode(pubkey.as_str()).unwrap();
        bytes[0] ^= 0x01;
        *pubkey = hex::encode(bytes);
    }
    let tampered_valid = chrononode_cli::verification::merkle::verify_proof_json(&tampered);
    assert!(!tampered_valid, "tampered proof should not verify");
}
