use chrononode_cli::index::sqlite::SqliteIndex;
use chrononode_cli::index::{open_index, IndexKind};
use tempfile::TempDir;

async fn setup_index(tmp: &TempDir) -> SqliteIndex {
    let db_path = tmp.path().join("test_watch.db");
    SqliteIndex::open(&db_path).await.unwrap()
}

#[tokio::test]
async fn test_watch_list_add_and_list() {
    let tmp = TempDir::new().unwrap();
    let index = setup_index(&tmp).await;

    index
        .add_watched_address(
            "bitcoin",
            "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
            0,
            Some("genesis"),
            None,
        )
        .await
        .unwrap();

    index
        .add_watched_address(
            "bitcoin",
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            500000,
            None,
            None,
        )
        .await
        .unwrap();

    let list = index.list_watched_addresses("bitcoin").await.unwrap();
    assert_eq!(list.len(), 2);

    let watched = index
        .is_address_watched("bitcoin", "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa")
        .await
        .unwrap();
    assert!(watched);

    let not_watched = index
        .is_address_watched("bitcoin", "nonexistent")
        .await
        .unwrap();
    assert!(!not_watched);
}

#[tokio::test]
async fn test_watch_list_remove() {
    let tmp = TempDir::new().unwrap();
    let index = setup_index(&tmp).await;

    index
        .add_watched_address(
            "ethereum",
            "0xdead000000000000000000000000000000000000",
            0,
            None,
            None,
        )
        .await
        .unwrap();

    let list = index.list_watched_addresses("ethereum").await.unwrap();
    assert_eq!(list.len(), 1);

    index
        .remove_watched_address("ethereum", "0xdead000000000000000000000000000000000000")
        .await
        .unwrap();

    let list = index.list_watched_addresses("ethereum").await.unwrap();
    assert_eq!(list.len(), 0);

    let watched = index
        .is_address_watched("ethereum", "0xdead000000000000000000000000000000000000")
        .await
        .unwrap();
    assert!(!watched);
}

#[tokio::test]
async fn test_watch_list_evm_wallet() {
    let tmp = TempDir::new().unwrap();
    let index = setup_index(&tmp).await;

    index
        .add_watched_address(
            "bitcoin",
            "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
            0,
            Some("genesis"),
            Some("0x42060A5Fc138ee019BC3F777B51c6490A1b881f0"),
        )
        .await
        .unwrap();

    let list = index.list_watched_addresses("bitcoin").await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0].3.as_deref(),
        Some("0x42060A5Fc138ee019BC3F777B51c6490A1b881f0")
    );

    // Re-adding without evm_wallet should not overwrite existing one
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

    let list = index.list_watched_addresses("bitcoin").await.unwrap();
    assert_eq!(
        list[0].3.as_deref(),
        Some("0x42060A5Fc138ee019BC3F777B51c6490A1b881f0")
    );
}

#[tokio::test]
async fn test_activity_index_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let index = setup_index(&tmp).await;

    index
        .record_activity("bitcoin", "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", 1, "tx1")
        .await
        .unwrap();

    let last = index
        .get_last_seen("bitcoin", "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa")
        .await
        .unwrap();
    assert_eq!(last, Some((1, "tx1".to_string())));

    index
        .record_activity("bitcoin", "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", 2, "tx2")
        .await
        .unwrap();

    let last = index
        .get_last_seen("bitcoin", "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa")
        .await
        .unwrap();
    assert_eq!(last, Some((2, "tx2".to_string())));

    let no_activity = index
        .get_last_seen("bitcoin", "unknown_address")
        .await
        .unwrap();
    assert_eq!(no_activity, None);
}

#[tokio::test]
async fn test_activity_index_multi_chain_isolation() {
    let tmp = TempDir::new().unwrap();
    let index = setup_index(&tmp).await;

    index
        .record_activity("bitcoin", "btc_address", 10, "btc_tx")
        .await
        .unwrap();

    index
        .record_activity("ethereum", "eth_address", 20, "eth_tx")
        .await
        .unwrap();

    let btc_last = index.get_last_seen("bitcoin", "btc_address").await.unwrap();
    assert_eq!(btc_last, Some((10, "btc_tx".to_string())));

    let eth_last = index
        .get_last_seen("ethereum", "eth_address")
        .await
        .unwrap();
    assert_eq!(eth_last, Some((20, "eth_tx".to_string())));

    let no_cross = index.get_last_seen("bitcoin", "eth_address").await.unwrap();
    assert_eq!(no_cross, None);
}

#[tokio::test]
async fn test_watch_list_via_index_backend_trait() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("test_trait.db");
    let index = open_index(IndexKind::Sqlite, &db_path, "").await.unwrap();

    index
        .add_watched_address("baals", "addr123", 42, Some("test-label"), None)
        .await
        .unwrap();

    let list = index.list_watched_addresses("baals").await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, "addr123");
    assert_eq!(list[0].1, 42);
    assert_eq!(list[0].2.as_deref(), Some("test-label"));
    assert_eq!(list[0].3, None);

    let watched = index.is_address_watched("baals", "addr123").await.unwrap();
    assert!(watched);

    index
        .remove_watched_address("baals", "addr123")
        .await
        .unwrap();
    let list = index.list_watched_addresses("baals").await.unwrap();
    assert!(list.is_empty());
}
