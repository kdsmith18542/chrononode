#[cfg(any(feature = "mongodb", feature = "scylla"))]
use chrononode_cli::index::{open_index, IndexKind};
#[cfg(any(feature = "mongodb", feature = "scylla"))]
use tempfile::TempDir;

#[cfg(feature = "mongodb")]
#[tokio::test]
async fn mongodb_backend_falls_back_to_sqlite() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("mongodb-fallback.db");

    let index = open_index(IndexKind::MongoDb, &db_path, "")
        .await
        .expect("mongodb compatibility backend should open");

    index
        .add_watched_address("bitcoin", "addr-mongo-1", 0, Some("mongo"), None)
        .await
        .expect("should write through sqlite fallback");

    let rows = index
        .list_watched_addresses("bitcoin")
        .await
        .expect("should list through sqlite fallback");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "addr-mongo-1");
}

#[cfg(feature = "scylla")]
#[tokio::test]
async fn scylla_backend_falls_back_to_sqlite() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("scylla-fallback.db");

    let index = open_index(IndexKind::Scylla, &db_path, "")
        .await
        .expect("scylla compatibility backend should open");

    index
        .add_watched_address("dogecoin", "addr-scylla-1", 0, Some("scylla"), None)
        .await
        .expect("should write through sqlite fallback");

    let rows = index
        .list_watched_addresses("dogecoin")
        .await
        .expect("should list through sqlite fallback");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "addr-scylla-1");
}
