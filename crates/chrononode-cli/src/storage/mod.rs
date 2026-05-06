pub mod ipfs;
pub mod local_fs;
pub mod pinata;

use async_trait::async_trait;
use chrononode_core::{Result, StorageBackend, StorageHealth, StoragePointer};
use std::sync::Arc;

pub enum BackendKind {
    LocalFs,
    Ipfs,
    Pinata,
}

pub fn create_backend(kind: BackendKind, base_path: &str) -> Arc<dyn StorageBackend> {
    match kind {
        BackendKind::LocalFs => Arc::new(local_fs::LocalFsBackend::new(base_path)),
        BackendKind::Ipfs => Arc::new(ipfs::IpfsBackend::new(base_path)),
        BackendKind::Pinata => Arc::new(pinata::PinataBackend::new(base_path)),
    }
}
