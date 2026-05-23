use chrononode_cli::storage::arweave::ArweaveBackend;
use chrononode_cli::storage::ipfs::IpfsBackend;
use chrononode_cli::storage::pinata::PinataBackend;
use chrononode_cli::storage::s3::S3Backend;
use chrononode_core::StorageBackend;
use mockito::{Matcher, Server};
use sha2::{Digest, Sha256};

#[tokio::test]
async fn test_ipfs_backend_put_get_pin_health() {
    let mut server = Server::new_async().await;
    let bytes = b"chrononode-ipfs-test";
    let expected_hash = hex::encode(Sha256::digest(bytes));

    let add_mock = server
        .mock("POST", "/api/v0/add")
        .match_query(Matcher::UrlEncoded("pin".into(), "true".into()))
        .with_status(200)
        .with_body("{\"Name\":\"chronoblock.bin\",\"Hash\":\"bafytestcid\",\"Size\":\"19\"}\n")
        .create_async()
        .await;

    let cat_mock = server
        .mock("POST", "/api/v0/cat")
        .match_query(Matcher::UrlEncoded("arg".into(), "bafytestcid".into()))
        .with_status(200)
        .with_body(bytes.as_slice())
        .create_async()
        .await;

    let pin_mock = server
        .mock("POST", "/api/v0/pin/add")
        .match_query(Matcher::UrlEncoded("arg".into(), "bafytestcid".into()))
        .with_status(200)
        .with_body("{}")
        .create_async()
        .await;

    let health_mock = server
        .mock("POST", "/api/v0/version")
        .with_status(200)
        .with_body("{}")
        .create_async()
        .await;

    let backend = IpfsBackend::new(&server.url());
    let pointer = backend.put(bytes).await.expect("ipfs put should succeed");
    assert_eq!(pointer.backend, "ipfs");
    assert_eq!(pointer.key, format!("{}:bafytestcid", expected_hash));

    let restored = backend
        .get(&pointer)
        .await
        .expect("ipfs get should succeed");
    assert_eq!(restored, bytes);

    backend
        .pin(&pointer)
        .await
        .expect("ipfs pin should succeed");

    let health = backend
        .health_check()
        .await
        .expect("ipfs health should return status");
    assert!(health.healthy);

    add_mock.assert_async().await;
    cat_mock.assert_async().await;
    pin_mock.assert_async().await;
    health_mock.assert_async().await;
}

#[tokio::test]
async fn test_pinata_backend_put_get_pin_health() {
    let mut server = Server::new_async().await;
    let bytes = b"chrononode-pinata-test";
    let expected_hash = hex::encode(Sha256::digest(bytes));
    let token = "pinata-test-token";

    let upload_mock = server
        .mock("POST", "/pinning/pinFileToIPFS")
        .match_header("authorization", format!("Bearer {}", token).as_str())
        .with_status(200)
        .with_body("{\"IpfsHash\":\"bafypinata123\"}")
        .create_async()
        .await;

    let gateway_mock = server
        .mock("GET", "/ipfs/bafypinata123")
        .match_header("authorization", format!("Bearer {}", token).as_str())
        .with_status(200)
        .with_body(bytes.as_slice())
        .create_async()
        .await;

    let pin_mock = server
        .mock("POST", "/pinning/pinByHash")
        .match_header("authorization", format!("Bearer {}", token).as_str())
        .match_body(Matcher::Regex("hashToPin".to_string()))
        .with_status(200)
        .with_body("{}")
        .create_async()
        .await;

    let health_mock = server
        .mock("GET", "/data/testAuthentication")
        .match_header("authorization", format!("Bearer {}", token).as_str())
        .with_status(200)
        .with_body("{\"message\":\"ok\"}")
        .create_async()
        .await;

    let backend = PinataBackend::new(&server.url(), &server.url(), Some(token.to_string()));
    let pointer = backend.put(bytes).await.expect("pinata put should succeed");
    assert_eq!(pointer.backend, "pinata");
    assert_eq!(pointer.key, format!("{}:bafypinata123", expected_hash));

    let restored = backend
        .get(&pointer)
        .await
        .expect("pinata get should succeed");
    assert_eq!(restored, bytes);

    backend
        .pin(&pointer)
        .await
        .expect("pinata pin should succeed");

    let health = backend
        .health_check()
        .await
        .expect("pinata health should return status");
    assert!(health.healthy);

    upload_mock.assert_async().await;
    gateway_mock.assert_async().await;
    pin_mock.assert_async().await;
    health_mock.assert_async().await;
}

#[tokio::test]
async fn test_pinata_backend_requires_jwt_for_upload() {
    let backend = PinataBackend::new(
        "https://api.pinata.cloud",
        "https://gateway.pinata.cloud",
        None,
    );
    let err = backend
        .put(b"missing-token")
        .await
        .expect_err("pinata upload should fail without JWT");
    assert!(
        err.to_string().contains("Pinata JWT missing"),
        "unexpected error: {}",
        err
    );
}

#[tokio::test]
async fn test_arweave_backend_put_get_health() {
    let mut server = Server::new_async().await;
    let bytes = b"chrononode-arweave-test";

    let tx_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    let upload_mock = server
        .mock("POST", "/tx")
        .match_header("Content-Type", "application/octet-stream")
        .with_status(200)
        .with_body(format!("{{\"id\":\"{}\"}}", tx_id))
        .create_async()
        .await;

    let data_mock = server
        .mock("GET", format!("/{}", tx_id).as_str())
        .with_status(200)
        .with_body(bytes.as_slice())
        .create_async()
        .await;

    let health_mock = server
        .mock("GET", "/info")
        .with_status(200)
        .with_body("{\"network\":\"test\"}")
        .create_async()
        .await;

    let backend = ArweaveBackend::new(&server.url(), &server.url());
    let pointer = backend
        .put(bytes)
        .await
        .expect("arweave put should succeed");
    assert_eq!(pointer.backend, "arweave");

    let restored = backend
        .get(&pointer)
        .await
        .expect("arweave get should succeed");
    assert_eq!(restored, bytes);

    let health = backend
        .health_check()
        .await
        .expect("arweave health should return status");
    assert!(health.healthy);

    upload_mock.assert_async().await;
    data_mock.assert_async().await;
    health_mock.assert_async().await;
}

#[tokio::test]
async fn test_arweave_backend_content_verification() {
    let mut server = Server::new_async().await;
    let original = b"chrononode-arweave-verify";
    let tampered = b"tampered-data";

    let tx_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    let upload_mock = server
        .mock("POST", "/tx")
        .match_header("Content-Type", "application/octet-stream")
        .with_status(200)
        .with_body(format!("{{\"id\":\"{}\"}}", tx_id))
        .create_async()
        .await;

    // Return tampered data on get — should fail content verification
    let data_mock = server
        .mock("GET", format!("/{}", tx_id).as_str())
        .with_status(200)
        .with_body(tampered.as_slice())
        .create_async()
        .await;

    let backend = ArweaveBackend::new(&server.url(), &server.url());
    let pointer = backend.put(original).await.expect("arweave put");
    upload_mock.assert_async().await;

    let err = backend
        .get(&pointer)
        .await
        .expect_err("get should fail due to content mismatch");
    assert!(
        err.to_string().contains("content hash mismatch"),
        "expected content hash mismatch error, got: {}",
        err
    );

    data_mock.assert_async().await;
}

#[tokio::test]
#[ignore = "requires a running S3-compatible server (e.g. MinIO)"]
async fn test_s3_backend_put_get_health() {
    let bucket =
        std::env::var("CHRONONODE_S3_BUCKET").unwrap_or_else(|_| "test-bucket".to_string());
    let region = std::env::var("CHRONONODE_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let endpoint = std::env::var("CHRONONODE_S3_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:9000".to_string());

    let backend =
        S3Backend::new(&bucket, &region, Some(&endpoint)).expect("S3 backend should initialize");

    let bytes = b"chrononode-s3-test";
    let pointer = backend.put(bytes).await.expect("s3 put should succeed");
    assert_eq!(pointer.backend, "s3");

    let restored = backend.get(&pointer).await.expect("s3 get should succeed");
    assert_eq!(restored, bytes);

    let health = backend
        .health_check()
        .await
        .expect("s3 health should return status");
    assert!(health.healthy);
}
