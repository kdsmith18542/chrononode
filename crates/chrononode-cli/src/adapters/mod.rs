pub mod baals;
pub mod mock;

use chrononode_core::ChainAdapter;
use std::sync::Arc;

pub enum AdapterKind {
    Mock,
    Baals,
}

pub fn create_adapter(kind: AdapterKind) -> Arc<dyn ChainAdapter> {
    match kind {
        AdapterKind::Mock => Arc::new(mock::MockAdapter::new()),
        AdapterKind::Baals => Arc::new(baals::BaalsAdapter::new("http://localhost:8545")),
    }
}
