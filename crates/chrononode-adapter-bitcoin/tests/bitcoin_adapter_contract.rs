use chrononode_adapter_bitcoin::BitcoinAdapter;
use chrononode_core::ChainAdapter;
use mockito::{Matcher, Server};

fn bitcoin_block_json(height: u64, hash: &str) -> serde_json::Value {
    serde_json::json!({
        "hash": hash,
        "height": height,
        "time": 1700000000 + height,
        "previousblockhash": "0000000000000000000000000000000000000000000000000000000000000000",
        "tx": [
            {
                "txid": "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
                "locktime": 100 + height,
                "vin": [
                    {
                        "coinbase": "04ffff001d0104"
                    }
                ],
                "vout": [
                    {
                        "value": 50.0,
                        "n": 0,
                        "scriptPubKey": {
                            "hex": "76a914760941548545831512",
                            "address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
                        }
                    }
                ]
            },
            {
                "txid": "1111111111111111111111111111111111111111111111111111111111111111",
                "locktime": 200 + height,
                "vin": [
                    {
                        "txid": "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
                        "vout": 0
                    }
                ],
                "vout": [
                    {
                        "value": 49.9,
                        "n": 0,
                        "scriptPubKey": {
                            "hex": "76a914760941548545831512",
                            "address": "1B1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
                        }
                    }
                ]
            }
        ]
    })
}

#[tokio::test]
async fn test_bitcoin_latest_height() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("POST", "/")
        .match_body(Matcher::Json(serde_json::json!({
            "jsonrpc": "1.0",
            "id": "chrononode",
            "method": "getblockcount",
            "params": []
        })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({
            "result": 800000,
            "error": null
        }).to_string())
        .create_async()
        .await;

    let adapter = BitcoinAdapter::new(&server.url(), None, None);
    let height = adapter.latest_height().await.unwrap();
    assert_eq!(height, 800000);
    m.assert();
}

#[tokio::test]
async fn test_bitcoin_fetch_block_by_height() {
    let mut server = Server::new_async().await;
    let hash = "000000000000000000000000000000000000000000000000000000000000abcd";

    // 1. mock getblockhash
    let m1 = server
        .mock("POST", "/")
        .match_body(Matcher::Json(serde_json::json!({
            "jsonrpc": "1.0",
            "id": "chrononode",
            "method": "getblockhash",
            "params": [42]
        })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({
            "result": hash,
            "error": null
        }).to_string())
        .create_async()
        .await;

    // 2. mock getblock (verbosity = 2)
    let m2 = server
        .mock("POST", "/")
        .match_body(Matcher::Json(serde_json::json!({
            "jsonrpc": "1.0",
            "id": "chrononode",
            "method": "getblock",
            "params": [hash, 2]
        })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({
            "result": bitcoin_block_json(42, hash),
            "error": null
        }).to_string())
        .create_async()
        .await;

    let adapter = BitcoinAdapter::new(&server.url(), None, None);
    let block = adapter.fetch_block(42).await.unwrap();

    assert_eq!(block.height, 42);
    assert_eq!(block.chain_id, "bitcoin");
    assert_eq!(block.block_hash_hex(), hash);
    assert_eq!(block.block_model, "Utxo");
    assert_eq!(block.transactions.len(), 2);

    // Verify coinbase transaction
    let tx1 = &block.transactions[0];
    assert_eq!(tx1.sender, b"coinbase");
    assert_eq!(tx1.recipient, b"1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa");
    assert_eq!(tx1.amount, 5000000000); // 50 BTC in Satoshis
    assert_eq!(tx1.nonce, 142);

    // Verify non-coinbase transaction
    let tx2 = &block.transactions[1];
    assert_eq!(
        String::from_utf8(tx2.sender.clone()).unwrap(),
        "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890:0"
    );
    assert_eq!(tx2.recipient, b"1B1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa");
    assert_eq!(tx2.amount, 4990000000); // 49.9 BTC in Satoshis
    assert_eq!(tx2.nonce, 242);

    m1.assert();
    m2.assert();
}

#[tokio::test]
async fn test_bitcoin_fetch_block_by_hash() {
    let mut server = Server::new_async().await;
    let hash = "000000000000000000000000000000000000000000000000000000000000eeee";

    let m = server
        .mock("POST", "/")
        .match_body(Matcher::Json(serde_json::json!({
            "jsonrpc": "1.0",
            "id": "chrononode",
            "method": "getblock",
            "params": [hash, 2]
        })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({
            "result": bitcoin_block_json(100, hash),
            "error": null
        }).to_string())
        .create_async()
        .await;

    let adapter = BitcoinAdapter::new(&server.url(), None, None);
    let hash_bytes = hex::decode(hash).unwrap();
    let block = adapter.fetch_block_by_hash(&hash_bytes).await.unwrap();

    assert_eq!(block.height, 100);
    assert_eq!(block.block_hash_hex(), hash);
    m.assert();
}

#[tokio::test]
async fn test_bitcoin_not_found() {
    let mut server = Server::new_async().await;
    let hash = "0000000000000000000000000000000000000000000000000000000000004044";

    let m = server
        .mock("POST", "/")
        .match_body(Matcher::Json(serde_json::json!({
            "jsonrpc": "1.0",
            "id": "chrononode",
            "method": "getblock",
            "params": [hash, 2]
        })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({
            "result": null,
            "error": {
                "code": -5,
                "message": "Block not found"
            }
        }).to_string())
        .create_async()
        .await;

    let adapter = BitcoinAdapter::new(&server.url(), None, None);
    let hash_bytes = hex::decode(hash).unwrap();
    let result = adapter.fetch_block_by_hash(&hash_bytes).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, chrononode_core::CoreError::NotFound(_)));

    m.assert();
}

#[tokio::test]
async fn test_bitcoin_retry_logic() {
    let mut server = Server::new_async().await;

    let m = server
        .mock("POST", "/")
        .with_status(500)
        .expect(5) // retry 5 times
        .create_async()
        .await;

    let adapter = BitcoinAdapter::new(&server.url(), None, None);
    let result = adapter.latest_height().await;
    assert!(result.is_err());
    m.assert();
}
