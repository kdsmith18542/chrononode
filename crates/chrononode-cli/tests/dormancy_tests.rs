use chrononode_cli::index::sqlite::SqliteIndex;
use chrononode_cli::index::{open_index, IndexKind};
use chrononode_core::{DormancyProof, OperatorKeypair};
use tempfile::TempDir;

async fn setup_index(tmp: &TempDir) -> SqliteIndex {
    let db_path = tmp.path().join("test_dormancy.db");
    SqliteIndex::open(&db_path).await.unwrap()
}

#[tokio::test]
async fn test_dormancy_set_and_get() {
    let tmp = TempDir::new().unwrap();
    let index = setup_index(&tmp).await;

    index
        .add_watched_address(
            "bitcoin",
            "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
            0,
            None,
            None,
        )
        .await
        .unwrap();

    index
        .record_activity("bitcoin", "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", 100, "tx1")
        .await
        .unwrap();

    index
        .set_dormant(
            "bitcoin",
            "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
            100,
            26280,
            30000,
        )
        .await
        .unwrap();

    let status = index
        .get_dormancy_status("bitcoin", "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa")
        .await
        .unwrap();
    assert_eq!(status, Some((100, 26280, 30000)));
}

#[tokio::test]
async fn test_dormancy_clear() {
    let tmp = TempDir::new().unwrap();
    let index = setup_index(&tmp).await;

    index
        .set_dormant(
            "ethereum",
            "0xdead000000000000000000000000000000000000",
            100,
            30000,
            50000,
        )
        .await
        .unwrap();

    let status = index
        .get_dormancy_status("ethereum", "0xdead000000000000000000000000000000000000")
        .await
        .unwrap();
    assert!(status.is_some());

    index
        .clear_dormant("ethereum", "0xdead000000000000000000000000000000000000")
        .await
        .unwrap();

    let status = index
        .get_dormancy_status("ethereum", "0xdead000000000000000000000000000000000000")
        .await
        .unwrap();
    assert!(status.is_none());
}

#[tokio::test]
async fn test_dormancy_list() {
    let tmp = TempDir::new().unwrap();
    let index = setup_index(&tmp).await;

    index
        .set_dormant("bitcoin", "addr1", 100, 26280, 500)
        .await
        .unwrap();
    index
        .set_dormant("bitcoin", "addr2", 200, 26280, 500)
        .await
        .unwrap();

    let list = index.list_dormant_addresses("bitcoin").await.unwrap();
    assert_eq!(list.len(), 2);

    let other = index.list_dormant_addresses("ethereum").await.unwrap();
    assert!(other.is_empty());
}

#[tokio::test]
async fn test_dormancy_proof_sign_and_verify() {
    let keypair = OperatorKeypair::generate();
    let mut proof = DormancyProof {
        version: "chrononode:dormancy:v1".to_string(),
        chain_id: "bitcoin".to_string(),
        address: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".to_string(),
        dormant_since_block: 100,
        current_block: 50000,
        threshold_blocks: 26280,
        signer_pubkey: None,
        signature: None,
        evm_wallet: None,
        proof_type: "ed25519".to_string(),
        zk_proof: None,
        public_inputs: None,
        confidence_tier: 1,
    };

    proof.sign(&keypair);
    assert!(proof.signer_pubkey.is_some());
    assert!(proof.signature.is_some());

    assert!(proof.verify());

    let mut tampered = proof.clone();
    tampered.dormant_since_block = 999;
    assert!(!tampered.verify());

    let mut bad_sig = proof.clone();
    bad_sig.signature = Some("00".repeat(64));
    assert!(!bad_sig.verify());
}

#[tokio::test]
async fn test_dormancy_proof_no_keypair_fails_verify() {
    let proof = DormancyProof {
        version: "chrononode:dormancy:v1".to_string(),
        chain_id: "bitcoin".to_string(),
        address: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".to_string(),
        dormant_since_block: 100,
        current_block: 50000,
        threshold_blocks: 26280,
        signer_pubkey: None,
        signature: None,
        evm_wallet: None,
        proof_type: "ed25519".to_string(),
        zk_proof: None,
        public_inputs: None,
        confidence_tier: 1,
    };
    assert!(!proof.verify());
}

#[tokio::test]
async fn test_dormancy_via_index_backend_trait() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("test_trait.db");
    let index = open_index(IndexKind::Sqlite, &db_path, "").await.unwrap();

    index
        .set_dormant("baals", "test_addr", 10, 100, 200)
        .await
        .unwrap();

    let status = index
        .get_dormancy_status("baals", "test_addr")
        .await
        .unwrap();
    assert_eq!(status, Some((10, 100, 200)));

    let list = index.list_dormant_addresses("baals").await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, "test_addr");

    index.clear_dormant("baals", "test_addr").await.unwrap();
    let status = index
        .get_dormancy_status("baals", "test_addr")
        .await
        .unwrap();
    assert!(status.is_none());
}
