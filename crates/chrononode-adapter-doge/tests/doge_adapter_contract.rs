use chrononode_adapter_doge::DogeAdapter;
use chrononode_core::ChainAdapter;
use mockito::{Matcher, Server};

fn block_json(hash: &str, height: u64, txids: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "hash": hash,
        "height": height,
        "chain": "DOGE.main",
        // Real BlockCypher format: ISO 8601 string
        "time": "2021-04-23T09:24:36Z",
        "received_time": "2021-04-23T09:24:36Z",
        "size": 1000u64,
        "prev_block": "000000000000000000015f6c0c6c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c",
        "mrkl_root": "abcd",
        "txids": txids,
        "nonce": 0u64,
        // Real BlockCypher format: integer, not hex string
        "bits": 436482088u64,
    })
}

fn regular_tx_json(hash: &str, prev_hash: &str, amount: u64, addr: &str) -> serde_json::Value {
    serde_json::json!({
        // Real BlockCypher field: "hash" not "tx_hash"
        "hash": hash,
        // Real BlockCypher field: "inputs" not "vin"
        "inputs": [{
            "prev_hash": prev_hash,
            "output_index": 0
        }],
        // Real BlockCypher field: "outputs" not "vout"
        "outputs": [{
            "value": amount,
            // Real BlockCypher field: "addresses" not "scriptpubkey_addresses"
            "addresses": [addr]
        }],
        "total": amount,
    })
}

fn coinbase_tx_json(hash: &str, amount: u64, addr: &str) -> serde_json::Value {
    serde_json::json!({
        "hash": hash,
        "inputs": [{
            // output_index == -1 signals coinbase
            "output_index": -1
        }],
        "outputs": [{
            "value": amount,
            "addresses": [addr]
        }],
        "total": amount,
    })
}

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
    let block_hash = "000000000000000000024bead8df69990852c202db0e0097c1a1ea2c0b1f2f3f";

    let _block_mock = server
        .mock("GET", "/v1/doge/main/blocks/12345")
        .with_status(200)
        .with_body(
            serde_json::to_string(&block_json(block_hash, 12345, &["tx1hash", "tx2hash"])).unwrap(),
        )
        .create();

    let _tx1_mock = server
        .mock("GET", "/v1/doge/main/txs/tx1hash")
        .with_status(200)
        .with_body(
            serde_json::to_string(&regular_tx_json(
                "tx1hash",
                "prevtx",
                1_000_000_000,
                "DAddress1",
            ))
            .unwrap(),
        )
        .create();

    let _tx2_mock = server
        .mock("GET", "/v1/doge/main/txs/tx2hash")
        .with_status(200)
        .with_body(
            serde_json::to_string(&coinbase_tx_json("tx2hash", 5_000_000_000, "DAddress2"))
                .unwrap(),
        )
        .create();

    let adapter = DogeAdapter::new(&server.url());
    let block = adapter.fetch_block(12345).await.unwrap();
    assert_eq!(block.height, 12345);
    assert_eq!(block.chain_id, "dogecoin");
    assert_eq!(block.block_model, "Utxo");
    assert_eq!(block.hash_algorithm, "scrypt");
    assert_eq!(block.transactions.len(), 2);
    assert_eq!(block.transactions[0].amount, 1_000_000_000);
    assert_eq!(block.transactions[1].amount, 5_000_000_000);
    // Coinbase sender should be the literal string "coinbase"
    assert_eq!(block.transactions[1].sender, b"coinbase");
    // Regular tx sender is "prevtx:0"
    assert_eq!(block.transactions[0].sender, b"prevtx:0");
}

#[tokio::test]
async fn test_doge_fetch_block_by_hash() {
    let mut server = Server::new_async().await;
    let hash_hex = "000000000000000000024bead8df69990852c202db0e0097c1a1ea2c0b1f2f3f";
    let hash = hex::decode(hash_hex).unwrap();

    let _block_mock = server
        .mock("GET", format!("/v1/doge/main/blocks/{}", hash_hex).as_str())
        .with_status(200)
        .with_body(serde_json::to_string(&block_json(hash_hex, 12345, &["txabc"])).unwrap())
        .create();

    let _tx_mock = server
        .mock("GET", "/v1/doge/main/txs/txabc")
        .with_status(200)
        .with_body(
            serde_json::to_string(&regular_tx_json(
                "txabc",
                "prevtx",
                100_000_000,
                "DAddress1",
            ))
            .unwrap(),
        )
        .create();

    let adapter = DogeAdapter::new(&server.url());
    let block = adapter.fetch_block_by_hash(&hash).await.unwrap();
    assert_eq!(block.height, 12345);
    assert_eq!(block.transactions.len(), 1);
    assert_eq!(block.transactions[0].amount, 100_000_000);
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

#[tokio::test]
async fn test_doge_fallback_endpoint_on_429() {
    let mut primary = Server::new_async().await;
    let mut secondary = Server::new_async().await;

    let primary_mock = primary
        .mock("GET", "/v1/doge/main")
        .with_status(429)
        .with_body("Too Many Requests")
        .expect_at_least(1)
        .create();

    let chain_json = serde_json::json!({
        "height": 5000123,
        "hash": "abcd",
        "name": "DOGE.main"
    });

    let secondary_mock = secondary
        .mock("GET", "/v1/doge/main")
        .with_status(200)
        .with_body(serde_json::to_string(&chain_json).unwrap())
        .create();

    let adapter = DogeAdapter::new_with_options(vec![primary.url(), secondary.url()], None);
    let height = adapter.latest_height().await.unwrap();
    assert_eq!(height, 5000123);

    primary_mock.assert();
    secondary_mock.assert();
}

#[tokio::test]
async fn test_doge_appends_api_token_query() {
    let mut server = Server::new_async().await;
    let chain_json = serde_json::json!({
        "height": 5000999,
        "hash": "abcd",
        "name": "DOGE.main"
    });

    let mock = server
        .mock("GET", "/v1/doge/main")
        .match_query(Matcher::Regex("token=abc123".into()))
        .with_status(200)
        .with_body(serde_json::to_string(&chain_json).unwrap())
        .create();

    let adapter = DogeAdapter::new_with_options(vec![server.url()], Some("abc123".to_string()));
    let height = adapter.latest_height().await.unwrap();
    assert_eq!(height, 5000999);
    mock.assert();
}
