use chrononode_core::ChainAdapter;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub type FactoryFn = fn(config: serde_json::Value) -> Result<Arc<dyn ChainAdapter>, String>;

#[derive(Clone)]
struct FactoryEntry {
    display_name: String,
    factory: FactoryFn,
}

static REGISTRY: OnceLock<Mutex<HashMap<String, FactoryEntry>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<String, FactoryEntry>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register(name: &str, display_name: &str, factory: FactoryFn) {
    registry().lock().unwrap().insert(
        name.to_string(),
        FactoryEntry {
            display_name: display_name.to_string(),
            factory,
        },
    );
}

pub fn create(name: &str, config: serde_json::Value) -> Result<Arc<dyn ChainAdapter>, String> {
    let reg = registry().lock().unwrap();
    let entry = reg.get(name).ok_or_else(|| {
        format!(
            "unknown adapter: '{}'. registered: {:?}",
            name,
            list_names()
        )
    })?;
    (entry.factory)(config)
}

pub fn list_adapters() -> Vec<(String, String)> {
    registry()
        .lock()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.display_name.clone()))
        .collect()
}

fn list_names() -> Vec<String> {
    registry().lock().unwrap().keys().cloned().collect()
}
