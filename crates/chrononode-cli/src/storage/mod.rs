pub mod arweave;
pub mod fallback;
pub mod ipfs;
pub mod local_fs;
pub mod pinata;
pub mod s3;

use chrononode_core::StorageBackend;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    LocalFs,
    Ipfs,
    Pinata,
    Arweave,
    S3,
}

impl BackendKind {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "local" | "local_fs" | "localfs" => Some(Self::LocalFs),
            "ipfs" => Some(Self::Ipfs),
            "pinata" => Some(Self::Pinata),
            "arweave" => Some(Self::Arweave),
            "s3" => Some(Self::S3),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub local_path: String,
    pub ipfs_api_url: String,
    pub pinata_api_base: String,
    pub pinata_gateway_base: String,
    pub pinata_jwt: Option<String>,
    pub arweave_gateway_base: String,
    pub arweave_bundler_url: String,
}

impl BackendConfig {
    pub fn from_env(local_path: &str) -> Self {
        Self {
            local_path: local_path.to_string(),
            ipfs_api_url: std::env::var("CHRONONODE_IPFS_API_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:5001".to_string()),
            pinata_api_base: std::env::var("CHRONONODE_PINATA_API_BASE")
                .unwrap_or_else(|_| "https://api.pinata.cloud".to_string()),
            pinata_gateway_base: std::env::var("CHRONONODE_PINATA_GATEWAY_BASE")
                .unwrap_or_else(|_| "https://gateway.pinata.cloud".to_string()),
            pinata_jwt: std::env::var("CHRONONODE_PINATA_JWT").ok(),
            arweave_gateway_base: std::env::var("CHRONONODE_ARWEAVE_GATEWAY")
                .unwrap_or_else(|_| "https://arweave.net".to_string()),
            arweave_bundler_url: std::env::var("CHRONONODE_ARWEAVE_BUNDLER")
                .unwrap_or_else(|_| "https://node2.irys.xyz".to_string()),
        }
    }
}

pub fn create_backend(kind: BackendKind, config: &BackendConfig) -> Arc<dyn StorageBackend> {
    match kind {
        BackendKind::LocalFs => Arc::new(local_fs::LocalFsBackend::new(&config.local_path)),
        BackendKind::Ipfs => Arc::new(ipfs::IpfsBackend::new(&config.ipfs_api_url)),
        BackendKind::Pinata => Arc::new(pinata::PinataBackend::new(
            &config.pinata_api_base,
            &config.pinata_gateway_base,
            config.pinata_jwt.clone(),
        )),
        BackendKind::Arweave => Arc::new(arweave::ArweaveBackend::new(
            &config.arweave_gateway_base,
            &config.arweave_bundler_url,
        )),
        BackendKind::S3 => match s3::S3Backend::from_env() {
            Ok(backend) => Arc::new(backend),
            Err(e) => {
                tracing::error!("Failed to initialize S3 backend: {}", e);
                std::process::exit(1);
            }
        },
    }
}
