use chrononode_adapter_baals::BaalsAdapter;
use chrononode_core::ChainAdapter;
use mockito::{Matcher, Server};

fn baals_block_json(height: u64, hash: &str) -> serde_json::Value {
    serde_json::json!({
        "index": height,
        "timestamp": 1700000000 + height,
        "prev_hash": "0000000000000000000000000000000000000000000000000000000000000000",
        "state_root": "0000000000000000000000000000000000000000000000000000000000000000",
        "hash": hash,
        "nonce": 0,
        "transactions": [{
            "hash": format!("{:0>64}", format!("tx_{}", height)),
            "sender": "1111111111111111111111111111111111111111111111111111111111111111",
            "nonce": height,
            "timestamp": 1700000000 + height,
            "recipient": "2222222222222222222222222222222222222222222222222222222222222222",
            "payload": {
                "type": "transfer",
                "amount": 1000
            },
            "gas_limit": 21000,
            "gas_price": 1
        }]
    })
}

#[tokio::test]
async fn test_baals_latest_height() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("GET", "/api/v1/chain/head")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"latest_block_index":42,"hash":"abcd","timestamp":1700000042,"tx_count":1}"#)
        .create_async()
        .await;
    let adapter = BaalsAdapter::new(&server.url());
    let height = adapter.latest_height().await.unwrap();
    assert_eq!(height, 42);
    m.assert();
}

#[tokio::test]
async fn test_baals_fetch_block_by_height() {
    let mut server = Server::new_async().await;
    let hash = "aaaa000000000000000000000000000000000000000000000000000000000000";
    let m = server
        .mock("GET", "/api/v1/blocks/7")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(baals_block_json(7, hash).to_string())
        .create_async()
        .await;
    let adapter = BaalsAdapter::new(&server.url());
    let block = adapter.fetch_block(7).await.unwrap();
    assert_eq!(block.height, 7);
    assert_eq!(block.chain_id, "baals");
    assert_eq!(block.block_hash_hex(), hash);
    assert_eq!(block.transactions.len(), 1);
    assert_eq!(block.transactions[0].amount, 1000);
    assert_eq!(block.transactions[0].nonce, 7);
    assert_eq!(block.transactions[0].gas_limit, 21000);
    m.assert();
}

#[tokio::test]
async fn test_baals_fetch_block_by_hash() {
    let mut server = Server::new_async().await;
    let hash = "bbbb000000000000000000000000000000000000000000000000000000000000";
    let m = server
        .mock(
            "GET",
            Matcher::Regex(format!("^/api/v1/blocks/by_hash/{}$", hash)),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(baals_block_json(3, hash).to_string())
        .create_async()
        .await;
    let adapter = BaalsAdapter::new(&server.url());
    let hash_bytes = hex::decode(hash).unwrap();
    let block = adapter.fetch_block_by_hash(&hash_bytes).await.unwrap();
    assert_eq!(block.height, 3);
    assert_eq!(block.block_hash_hex(), hash);
    m.assert();
}

#[tokio::test]
async fn test_baals_server_error_retries_then_fails() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("GET", "/api/v1/chain/head")
        .with_status(507)
        .expect(5)
        .create_async()
        .await;
    let adapter = BaalsAdapter::new(&server.url());
    let result = adapter.latest_height().await;
    assert!(result.is_err());
    m.assert();
}

#[tokio::test]
async fn test_baals_client_error_no_retry() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("GET", "/api/v1/blocks/9999")
        .with_status(404)
        .with_body(r#"{"error":"block not found"}"#)
        .create_async()
        .await;
    let adapter = BaalsAdapter::new(&server.url());
    let result = adapter.fetch_block(9999).await;
    assert!(result.is_err());
    m.assert();
}
