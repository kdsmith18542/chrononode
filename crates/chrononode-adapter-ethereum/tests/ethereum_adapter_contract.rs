use chrononode_adapter_ethereum::EthereumAdapter;
use chrononode_core::ChainAdapter;
use mockito::{Matcher, Server};

fn ethereum_block_json(height: u64, hash: &str) -> serde_json::Value {
    serde_json::json!({
        "number": format!("0x{:x}", height),
        "hash": hash,
        "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "timestamp": format!("0x{:x}", 1700000000 + height),
        "stateRoot": "0xstate_root_hash",
        "transactionsRoot": "0xtransactions_root_hash",
        "receiptsRoot": "0xreceipts_root_hash",
        "transactions": [
            {
                "hash": "0x1111111111111111111111111111111111111111111111111111111111111111",
                "from": "0x2222222222222222222222222222222222222222",
                "to": "0x3333333333333333333333333333333333333333",
                "value": "0x3b9aca00", // 1,000,000,000 Wei = 1 Gwei
                "nonce": "0x5",
                "input": "0xabcdef",
                "gas": "0x5208" // 21000
            }
        ]
    })
}

fn ethereum_receipt_json() -> serde_json::Value {
    serde_json::json!({
        "transactionHash": "0x1111111111111111111111111111111111111111111111111111111111111111",
        "transactionIndex": "0x0",
        "gasUsed": "0x5208",
        "contractAddress": null,
        "logs": [
            {
                "address": "0x3333333333333333333333333333333333333333",
                "topics": [
                    "0x9999999999999999999999999999999999999999999999999999999999999999"
                ],
                "data": "0x0000000000000000000000000000000000000000000000000000000000000005",
                "transactionIndex": "0x0",
                "logIndex": "0x0"
            }
        ]
    })
}

#[tokio::test]
async fn test_ethereum_latest_height() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("POST", "/")
        .match_body(Matcher::Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": "chrononode",
            "method": "eth_blockNumber",
            "params": []
        })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({
            "result": "0xc350", // 50000
            "error": null
        }).to_string())
        .create_async()
        .await;

    let adapter = EthereumAdapter::new(&server.url());
    let height = adapter.latest_height().await.unwrap();
    assert_eq!(height, 50000);
    m.assert();
}

#[tokio::test]
async fn test_ethereum_fetch_block_by_height() {
    let mut server = Server::new_async().await;
    let hash = "0x1111111111111111111111111111111111111111111111111111111111111111";

    let m1 = server
        .mock("POST", "/")
        .match_body(Matcher::Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": "chrononode",
            "method": "eth_getBlockByNumber",
            "params": ["0x2a", true]
        })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({
            "result": ethereum_block_json(42, hash),
            "error": null
        }).to_string())
        .create_async()
        .await;

    let m2 = server
        .mock("POST", "/")
        .match_body(Matcher::Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": "chrononode",
            "method": "eth_getTransactionReceipt",
            "params": [hash]
        })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({
            "result": ethereum_receipt_json(),
            "error": null
        }).to_string())
        .create_async()
        .await;

    let adapter = EthereumAdapter::new(&server.url());
    let block = adapter.fetch_block(42).await.unwrap();

    assert_eq!(block.height, 42);
    assert_eq!(block.chain_id, "ethereum");
    assert_eq!(block.block_hash_hex(), hash.trim_start_matches("0x"));
    assert_eq!(block.block_model, "Account");
    assert_eq!(block.transactions.len(), 1);

    let tx = &block.transactions[0];
    assert_eq!(hex::encode(&tx.tx_hash), hash.trim_start_matches("0x"));
    assert_eq!(hex::encode(&tx.sender), "2222222222222222222222222222222222222222");
    assert_eq!(hex::encode(&tx.recipient), "3333333333333333333333333333333333333333");
    assert_eq!(tx.amount, 1); // 1 Gwei
    assert_eq!(tx.nonce, 5);
    assert_eq!(tx.gas_limit, 21000);
    assert_eq!(tx.gas_used, 21000);
    assert_eq!(tx.payload, vec![0xab, 0xcd, 0xef]);

    assert_eq!(block.events.len(), 1);
    let event = &block.events[0];
    assert_eq!(event.event_type, "9999999999999999999999999999999999999999999999999999999999999999");
    assert_eq!(hex::encode(&event.emitter), "3333333333333333333333333333333333333333");
    assert_eq!(event.tx_index, 0);
    assert_eq!(event.event_index, 0);
    assert_eq!(event.payload, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5]);

    m1.assert();
    m2.assert();
}

#[tokio::test]
async fn test_ethereum_not_found() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("POST", "/")
        .match_body(Matcher::Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": "chrononode",
            "method": "eth_getBlockByNumber",
            "params": ["0x2a", true]
        })))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({
            "result": null,
            "error": null
        }).to_string())
        .create_async()
        .await;

    let adapter = EthereumAdapter::new(&server.url());
    let result = adapter.fetch_block(42).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, chrononode_core::CoreError::NotFound(_)));
    m.assert();
}

#[tokio::test]
async fn test_ethereum_retry_logic() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("POST", "/")
        .with_status(500)
        .expect(5)
        .create_async()
        .await;

    let adapter = EthereumAdapter::new(&server.url());
    let result = adapter.latest_height().await;
    assert!(result.is_err());
    m.assert();
}
