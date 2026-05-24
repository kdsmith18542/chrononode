use chrononode_cli::attestation::BaalsSubmitter;
use chrononode_cli::index::sqlite::SqliteIndex;
use chrononode_core::{
    AttestationConfig, CoreConfig, DormancyConfig, DormancyProof, OperatorKeypair,
};
use tempfile::TempDir;

fn make_submitter_config(api_url: &str, key_path: Option<&str>) -> CoreConfig {
    CoreConfig {
        checkpoint_size: 1000,
        hash_algorithm: "sha256".to_string(),
        repair: Default::default(),
        pruning: Default::default(),
        compression: false,
        dormancy: DormancyConfig::default(),
        attestation: AttestationConfig {
            baals_api_url: Some(api_url.to_string()),
            baals_key_path: key_path.map(|s| s.to_string()),
            auto_submit: true,
            evm_rpc_url: None,
            evm_contract_address: None,
            evm_gas_limit: 1_000_000,
            evm_private_key: None,
            evm_chain_id: None,
        },
    }
}

async fn setup_index(tmp: &TempDir) -> SqliteIndex {
    let db_path = tmp.path().join("test_attestation.db");
    SqliteIndex::open(&db_path).await.unwrap()
}

#[tokio::test]
async fn test_attestation_exists_idempotency() {
    let tmp = TempDir::new().unwrap();
    let index = setup_index(&tmp).await;

    let exists = index
        .attestation_exists("bitcoin", "addr1", 100)
        .await
        .unwrap();
    assert!(!exists);

    index
        .record_attestation("bitcoin", "addr1", 100, Some("txhash"), "submitted")
        .await
        .unwrap();

    let exists = index
        .attestation_exists("bitcoin", "addr1", 100)
        .await
        .unwrap();
    assert!(exists);
}

#[tokio::test]
async fn test_attestation_list() {
    let tmp = TempDir::new().unwrap();
    let index = setup_index(&tmp).await;

    index
        .record_attestation("bitcoin", "addr1", 100, Some("tx1"), "submitted")
        .await
        .unwrap();
    index
        .record_attestation("bitcoin", "addr2", 200, None, "pending")
        .await
        .unwrap();

    let list = index.list_attestations("bitcoin").await.unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn test_baals_submitter_not_configured() {
    let config = make_submitter_config("http://localhost:9999", None);
    let submitter = BaalsSubmitter::new(&config);
    assert!(!submitter.is_configured());
}

#[tokio::test]
async fn test_baals_submitter_submit_with_mock() {
    let mut server = mockito::Server::new_async().await;

    let keypair = OperatorKeypair::generate();
    let tmp = TempDir::new().unwrap();
    let key_path = tmp.path().join("baals.key");
    keypair.save_to_file(&key_path).unwrap();

    let config = make_submitter_config(&server.url(), Some(key_path.to_str().unwrap()));
    let submitter = BaalsSubmitter::new(&config);
    assert!(submitter.is_configured());

    let index = setup_index(&tmp).await;

    let mock = server
        .mock("POST", "/api/v1/oracle/attest")
        .with_status(200)
        .with_body(r#"{"status": "ok", "attestation": {"baals_signature": "0xdeadbeef"}}"#)
        .create();

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
    };

    let result = submitter
        .submit_dormancy_proof(&proof, &index)
        .await
        .unwrap();
    assert_eq!(result, Some("0xdeadbeef".to_string()));
    mock.assert();

    let exists = index
        .attestation_exists("bitcoin", "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", 100)
        .await
        .unwrap();
    assert!(exists);
}

#[tokio::test]
async fn test_baals_submitter_idempotent() {
    let server = mockito::Server::new_async().await;
    let keypair = OperatorKeypair::generate();
    let tmp = TempDir::new().unwrap();
    let key_path = tmp.path().join("baals.key2");
    keypair.save_to_file(&key_path).unwrap();

    let config = make_submitter_config(&server.url(), Some(key_path.to_str().unwrap()));
    let submitter = BaalsSubmitter::new(&config);
    let index = setup_index(&tmp).await;

    index
        .record_attestation("bitcoin", "addr1", 100, Some("existing_tx"), "submitted")
        .await
        .unwrap();

    let proof = DormancyProof {
        version: "chrononode:dormancy:v1".to_string(),
        chain_id: "bitcoin".to_string(),
        address: "addr1".to_string(),
        dormant_since_block: 100,
        current_block: 50000,
        threshold_blocks: 26280,
        signer_pubkey: None,
        signature: None,
        evm_wallet: None,
    };

    let result = submitter
        .submit_dormancy_proof(&proof, &index)
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn test_baals_submitter_retry_on_500() {
    let mut server = mockito::Server::new_async().await;
    let keypair = OperatorKeypair::generate();
    let tmp = TempDir::new().unwrap();
    let key_path = tmp.path().join("baals.key3");
    keypair.save_to_file(&key_path).unwrap();

    let config = make_submitter_config(&server.url(), Some(key_path.to_str().unwrap()));
    let submitter = BaalsSubmitter::new(&config);
    let index = setup_index(&tmp).await;

    let _mock = server
        .mock("POST", "/api/v1/oracle/attest")
        .with_status(500)
        .with_body("Server Error")
        .create();

    let proof = DormancyProof {
        version: "chrononode:dormancy:v1".to_string(),
        chain_id: "bitcoin".to_string(),
        address: "addr_fail".to_string(),
        dormant_since_block: 100,
        current_block: 50000,
        threshold_blocks: 26280,
        signer_pubkey: None,
        signature: None,
        evm_wallet: None,
    };

    let result = submitter.submit_dormancy_proof(&proof, &index).await;
    assert!(result.is_err());
}
