use chrononode_adapter_bitcoin_light::BitcoinLightAdapter;
use chrononode_core::ChainAdapter;
use mockito::Server;

#[tokio::test]
async fn test_bitcoin_light_latest_height() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/api/blocks/tip/height")
        .with_status(200)
        .with_body("750000")
        .create();

    let adapter = BitcoinLightAdapter::new(&server.url());
    let height = adapter.latest_height().await.unwrap();
    assert_eq!(height, 750000);
    mock.assert();
}

#[tokio::test]
async fn test_bitcoin_light_fetch_block_by_height() {
    let mut server = Server::new_async().await;

    let _hash_mock = server
        .mock("GET", "/api/block-height/100")
        .with_status(200)
        .with_body("000000000000000000024bead8df69990852c202db0e0097c1a1ea2c0b1f2f3f")
        .create();

    let block_json = serde_json::json!({
        "id": "000000000000000000024bead8df69990852c202db0e0097c1a1ea2c0b1f2f3f",
        "height": 100,
        "version": 1,
        "timestamp": 1234567890,
        "tx_count": 2,
        "size": 1000,
        "weight": 4000,
        "merkle_root": "abcd",
        "previousblockhash": "000000000000000000015f6c0c6c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c",
        "nonce": 0,
        "bits": 0,
        "difficulty": 1.0
    });

    let _block_mock = server
        .mock(
            "GET",
            "/api/block/000000000000000000024bead8df69990852c202db0e0097c1a1ea2c0b1f2f3f",
        )
        .with_status(200)
        .with_body(serde_json::to_string(&block_json).unwrap())
        .create();

    let txs_json = serde_json::json!([
        {
            "txid": "aabbccdd",
            "version": 1,
            "locktime": 0,
            "vin": [{
                "txid": "prevtx",
                "vout": 0,
                "is_coinbase": false,
                "sequence": 4294967295u64
            }],
            "vout": [{
                "value": 5000000000u64,
                "scriptpubkey_address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
                "scriptpubkey_type": "p2pkh"
            }]
        }
    ]);

    let _txs_mock = server
        .mock(
            "GET",
            "/api/block/000000000000000000024bead8df69990852c202db0e0097c1a1ea2c0b1f2f3f/txs/0",
        )
        .with_status(200)
        .with_body(serde_json::to_string(&txs_json).unwrap())
        .create();

    let adapter = BitcoinLightAdapter::new(&server.url());
    let block = adapter.fetch_block(100).await.unwrap();
    assert_eq!(block.height, 100);
    assert_eq!(block.chain_id, "bitcoin");
    assert_eq!(block.block_model, "Utxo");
    assert_eq!(block.transactions.len(), 1);
    assert_eq!(block.transactions[0].amount, 5000000000);
}

#[tokio::test]
async fn test_bitcoin_light_fetch_block_by_hash() {
    let mut server = Server::new_async().await;
    let hash_hex = "000000000000000000024bead8df69990852c202db0e0097c1a1ea2c0b1f2f3f";
    let hash = hex::decode(hash_hex).unwrap();

    let block_json = serde_json::json!({
        "id": hash_hex,
        "height": 100,
        "version": 1,
        "timestamp": 1234567890,
        "tx_count": 1,
        "size": 1000,
        "weight": 4000,
        "merkle_root": "abcd",
        "previousblockhash": "000000000000000000015f6c0c6c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c5c",
        "nonce": 0,
        "bits": 0,
        "difficulty": 1.0
    });

    let _block_mock = server
        .mock("GET", format!("/api/block/{}", hash_hex).as_str())
        .with_status(200)
        .with_body(serde_json::to_string(&block_json).unwrap())
        .create();

    let txs_json = serde_json::json!([
        {
            "txid": "aabbccdd",
            "version": 1,
            "locktime": 0,
            "vin": [{
                "txid": "prevtx",
                "vout": 0,
                "is_coinbase": false,
                "sequence": 4294967295u64
            }],
            "vout": [{
                "value": 100000000u64,
                "scriptpubkey_address": "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
                "scriptpubkey_type": "p2pkh"
            }]
        }
    ]);

    let _txs_mock = server
        .mock("GET", format!("/api/block/{}/txs/0", hash_hex).as_str())
        .with_status(200)
        .with_body(serde_json::to_string(&txs_json).unwrap())
        .create();

    let adapter = BitcoinLightAdapter::new(&server.url());
    let block = adapter.fetch_block_by_hash(&hash).await.unwrap();
    assert_eq!(block.height, 100);
    assert_eq!(block.transactions[0].amount, 100000000);
}

#[tokio::test]
async fn test_bitcoin_light_not_found() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/block-height/99999999")
        .with_status(404)
        .with_body("Block not found")
        .create();

    let adapter = BitcoinLightAdapter::new(&server.url());
    let result = adapter.fetch_block(99999999).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_bitcoin_light_server_error_retries() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/api/blocks/tip/height")
        .with_status(503)
        .with_body("Service Unavailable")
        .expect_at_least(1)
        .create();

    let adapter = BitcoinLightAdapter::new(&server.url());
    let result = adapter.latest_height().await;
    assert!(result.is_err());
    mock.assert();
}

#[tokio::test]
async fn test_bitcoin_light_fallback_endpoint_on_429() {
    let mut primary = Server::new_async().await;
    let mut secondary = Server::new_async().await;

    let primary_mock = primary
        .mock("GET", "/api/blocks/tip/height")
        .with_status(429)
        .with_body("Too Many Requests")
        .expect_at_least(1)
        .create();

    let secondary_mock = secondary
        .mock("GET", "/api/blocks/tip/height")
        .with_status(200)
        .with_body("750001")
        .create();

    let adapter = BitcoinLightAdapter::new_with_fallbacks(vec![primary.url(), secondary.url()]);
    let height = adapter.latest_height().await.unwrap();
    assert_eq!(height, 750001);

    primary_mock.assert();
    secondary_mock.assert();
}
