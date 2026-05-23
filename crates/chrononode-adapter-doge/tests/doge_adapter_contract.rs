use chrononode_adapter_doge::DogeAdapter;
use chrononode_core::ChainAdapter;
use mockito::Server;

#[tokio::test]
async fn test_doge_latest_height() {
    let mut server = Server::new_async().await;
    let chain_json = serde_json::json!({
        "height": 5000000,
        "hash": "abcd",
        "name": "DOGE.main"
    });

    let mock = server
        .mock("GET", "/v1/doge/main")
        .with_status(200)
        .with_body(serde_json::to_string(&chain_json).unwrap())
        .create();

    let adapter = DogeAdapter::new(&server.url());
    let height = adapter.latest_height().await.unwrap();
    assert_eq!(height, 5000000);
    mock.assert();
}

#[tokio::test]
async fn test_doge_fetch_block_by_height() {
    let mut server = Server::new_async().await;

    let block_json = serde_json::json!({
        "hash": "000000000000000000024bead8df69990852c202db0e0097c1a1ea2c0b1f2f3f",
        "height": 12345,
        "chain": "DOGE.main",
        "time": 1234567890,
        "received_time": 1234567890,
        "size": 1000,
        "prev_block": "000000000000000000015f6c0c6c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c",
        "mrkl_root": "abcd",
        "txids": ["tx1hash", "tx2hash"],
        "nonce": 0,
        "bits": "1a1a1a1a"
    });

    let _block_mock = server
        .mock("GET", "/v1/doge/main/blocks/12345")
        .with_status(200)
        .with_body(serde_json::to_string(&block_json).unwrap())
        .create();

    let tx_json = serde_json::json!({
        "tx_hash": "tx1hash",
        "vin": [{
            "tx_hash": "prevtx",
            "vout_index": 0,
            "coinbase": false
        }],
        "vout": [{
            "value": 1000000000u64,
            "scriptpubkey_addresses": ["DAddress1"]
        }],
        "lock_time": 0,
        "total": 1000000000u64
    });

    let _tx1_mock = server
        .mock("GET", "/v1/doge/main/txs/tx1hash")
        .with_status(200)
        .with_body(serde_json::to_string(&tx_json).unwrap())
        .create();

    let tx2_json = serde_json::json!({
        "tx_hash": "tx2hash",
        "vin": [{
            "coinbase": true
        }],
        "vout": [{
            "value": 5000000000u64,
            "scriptpubkey_addresses": ["DAddress2"]
        }],
        "lock_time": 0,
        "total": 5000000000u64
    });

    let _tx2_mock = server
        .mock("GET", "/v1/doge/main/txs/tx2hash")
        .with_status(200)
        .with_body(serde_json::to_string(&tx2_json).unwrap())
        .create();

    let adapter = DogeAdapter::new(&server.url());
    let block = adapter.fetch_block(12345).await.unwrap();
    assert_eq!(block.height, 12345);
    assert_eq!(block.chain_id, "dogecoin");
    assert_eq!(block.block_model, "Utxo");
    assert_eq!(block.hash_algorithm, "scrypt");
    assert_eq!(block.transactions.len(), 2);
    assert_eq!(block.transactions[0].amount, 1000000000);
    assert_eq!(block.transactions[1].amount, 5000000000);
}

#[tokio::test]
async fn test_doge_fetch_block_by_hash() {
    let mut server = Server::new_async().await;
    let hash_hex = "000000000000000000024bead8df69990852c202db0e0097c1a1ea2c0b1f2f3f";
    let hash = hex::decode(hash_hex).unwrap();

    let block_json = serde_json::json!({
        "hash": hash_hex,
        "height": 12345,
        "chain": "DOGE.main",
        "time": 1234567890,
        "received_time": 1234567890,
        "size": 1000,
        "prev_block": "000000000000000000015f6c0c6c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c",
        "mrkl_root": "abcd",
        "txids": ["txabc"],
        "nonce": 0,
        "bits": "1a1a1a1a"
    });

    let _block_mock = server
        .mock("GET", format!("/v1/doge/main/blocks/{}", hash_hex).as_str())
        .with_status(200)
        .with_body(serde_json::to_string(&block_json).unwrap())
        .create();

    let tx_json = serde_json::json!({
        "tx_hash": "txabc",
        "vin": [{
            "tx_hash": "prevtx",
            "vout_index": 0,
            "coinbase": false
        }],
        "vout": [{
            "value": 100000000u64,
            "scriptpubkey_addresses": ["DAddress1"]
        }],
        "lock_time": 0,
        "total": 100000000u64
    });

    let _tx_mock = server
        .mock("GET", "/v1/doge/main/txs/txabc")
        .with_status(200)
        .with_body(serde_json::to_string(&tx_json).unwrap())
        .create();

    let adapter = DogeAdapter::new(&server.url());
    let block = adapter.fetch_block_by_hash(&hash).await.unwrap();
    assert_eq!(block.height, 12345);
    assert_eq!(block.transactions.len(), 1);
}

#[tokio::test]
async fn test_doge_not_found() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/v1/doge/main/blocks/99999999")
        .with_status(404)
        .with_body("Not found")
        .create();

    let adapter = DogeAdapter::new(&server.url());
    let result = adapter.fetch_block(99999999).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_doge_server_error_retries() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/doge/main")
        .with_status(503)
        .with_body("Service Unavailable")
        .expect_at_least(1)
        .create();

    let adapter = DogeAdapter::new(&server.url());
    let result = adapter.latest_height().await;
    assert!(result.is_err());
    mock.assert();
}
