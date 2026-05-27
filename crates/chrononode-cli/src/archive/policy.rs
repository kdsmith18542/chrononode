/// Phase 5 — CanvasContracts Policy Archive.
///
/// Handles archiving policy manifests (graph definitions + compiled WASM)
/// to content-addressed storage backends (IPFS, Arweave, local FS).
///
/// Archive flow:
///   1. Compile policy graph to WASM (policy_compiler)
///   2. Build PolicyManifest with graph + wasm_hash + validation report
///   3. Upload raw evidence to configured storage backend
///   4. Return content-addressed pointer
use chrononode_core::{
    policy::{PolicyGraph, PolicyManifest, ValidationReport, serialize_policy_graph},
    Result, CoreError,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::path::Path;

/// Storage backends supported for policy archiving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveBackend {
    LocalFs,
    Ipfs,
    Pinata,
    Arweave,
    S3Compatible,
}

impl ArchiveBackend {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "local" | "localfs" => Some(Self::LocalFs),
            "ipfs" => Some(Self::Ipfs),
            "pinata" => Some(Self::Pinata),
            "arweave" => Some(Self::Arweave),
            "s3" => Some(Self::S3Compatible),
            _ => None,
        }
    }
}

/// Trait for policy archive storage backends.
#[async_trait::async_trait]
pub trait PolicyArchiveBackend: Send + Sync {
    async fn store(&self, key: &str, data: &[u8]) -> Result<String>;
    async fn retrieve(&self, pointer: &str) -> Result<Vec<u8>>;
    async fn exists(&self, pointer: &str) -> Result<bool>;
}

/// Local filesystem archive backend.
pub struct LocalFsArchive {
    base_path: String,
}

impl LocalFsArchive {
    pub fn new(base_path: impl Into<String>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }
}

#[async_trait::async_trait]
impl PolicyArchiveBackend for LocalFsArchive {
    async fn store(&self, key: &str, data: &[u8]) -> Result<String> {
        let dir = Path::new(&self.base_path).join("policies");
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| CoreError::Storage(format!("Failed to create archive dir: {}", e)))?;

        let file_path = dir.join(format!("{}.policy", key));
        tokio::fs::write(&file_path, data)
            .await
            .map_err(|e| CoreError::Storage(format!("Failed to write policy: {}", e)))?;

        Ok(file_path.to_string_lossy().to_string())
    }

    async fn retrieve(&self, pointer: &str) -> Result<Vec<u8>> {
        tokio::fs::read(pointer)
            .await
            .map_err(|e| CoreError::Storage(format!("Failed to read policy: {}", e)))
    }

    async fn exists(&self, pointer: &str) -> Result<bool> {
        let path = Path::new(pointer);
        Ok(path.exists())
    }
}

/// Archive a compiled policy graph + WASM to the configured backend.
pub async fn archive_policy(
    graph: &PolicyGraph,
    compiled_wasm: &[u8],
    backend: &dyn PolicyArchiveBackend,
) -> Result<PolicyManifest> {
    let _graph_json = serialize_policy_graph(graph)
        .map_err(|e| CoreError::Storage(format!("Failed to serialize graph: {}", e)))?;

    let wasm_hash = hex::encode(sha2::Sha256::digest(compiled_wasm));
    let wasm_pointer = format!("wasm:{}", wasm_hash);

    let archive_key = format!("policy-{}-v{}", graph.id, graph.version.replace('.', "-"));

    // Validate
    let errors = match graph.validate() {
        Ok(()) => vec![],
        Err(e) => e,
    };

    let max_depth = graph.nodes.iter().map(|n| {
        let outgoing = graph.edges.iter().filter(|e| e.source == n.id).count();
        outgoing
    }).max().unwrap_or(0);

    let manifest = PolicyManifest {
        graph: graph.clone(),
        compiled_wasm_hash: Some(wasm_hash.clone()),
        compiled_wasm_pointer: Some(wasm_pointer),
        schema_version: "1.0.0".to_string(),
        archive_timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        archive_storage_backend: "localfs".to_string(),
        archive_storage_pointer: archive_key.clone(),
        validation_report: ValidationReport {
            passed: errors.is_empty(),
            errors,
            warnings: vec![],
            node_count: graph.nodes.len(),
            edge_count: graph.edges.len(),
            max_depth,
        },
    };

    let manifest_bytes = serde_json::to_vec(&manifest)
        .map_err(|e| CoreError::Storage(format!("Failed to encode manifest: {}", e)))?;

    // Upload manifest to storage
    let storage_pointer = backend.store(&archive_key, &manifest_bytes).await?;

    // Also store the wasm separately
    let wasm_key = format!("wasm-{}", wasm_hash);
    backend.store(&wasm_key, compiled_wasm).await?;

    Ok(PolicyManifest {
        archive_storage_pointer: storage_pointer,
        ..manifest
    })
}

/// Retrieve and decompile an archived policy manifest.
pub async fn retrieve_policy_manifest(
    pointer: &str,
    backend: &dyn PolicyArchiveBackend,
) -> Result<PolicyManifest> {
    let data = backend.retrieve(pointer).await?;
    let manifest: PolicyManifest = serde_json::from_slice(&data)
        .map_err(|e| CoreError::Storage(format!("Failed to deserialize manifest: {}", e)))?;
    Ok(manifest)
}

/// Generate a governance proposal payload for a policy update.
/// This output can be submitted to the Resurgence DAO for a vote.
pub fn generate_governance_proposal(
    manifest: &PolicyManifest,
    proposer: &str,
) -> serde_json::Value {
    serde_json::json!({
        "proposal_type": "policy_update",
        "title": format!("Update reward policy: {}", manifest.graph.name),
        "description": format!(
            "Proposes updating the '{}' reward policy (v{}).\n\n{}\n\nValidation: {}",
            manifest.graph.name,
            manifest.graph.version,
            manifest.graph.description,
            if manifest.validation_report.passed { "PASSED" } else { "FAILED" }
        ),
        "proposer": proposer,
        "archive_pointer": manifest.archive_storage_pointer,
        "policy_graph_id": manifest.graph.id,
        "policy_version": manifest.graph.version,
        "wasm_hash": manifest.compiled_wasm_hash.clone().unwrap_or_default(),
        "validation_passed": manifest.validation_report.passed,
        "node_count": manifest.validation_report.node_count,
        "edge_count": manifest.validation_report.edge_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrononode_core::policy::*;
    use std::collections::HashMap;

    fn sample_graph() -> PolicyGraph {
        let mut graph = PolicyGraph::new("test-policy".to_string(), "Test Policy".to_string());
        graph.add_node(PolicyNode {
            id: "in".to_string(), label: "Input".to_string(),
            node_type: NodeType::Input, params: HashMap::new(),
            position_x: 0.0, position_y: 0.0, description: "".to_string(),
        });
        graph.add_node(PolicyNode {
            id: "out".to_string(), label: "Output".to_string(),
            node_type: NodeType::Output, params: HashMap::new(),
            position_x: 100.0, position_y: 0.0, description: "".to_string(),
        });
        graph.add_edge(PolicyEdge {
            id: "e1".to_string(), source: "in".to_string(), target: "out".to_string(),
            label: None, weight: 1.0,
        });
        graph
    }

    #[tokio::test]
    async fn test_archive_and_retrieve() {
        let graph = sample_graph();
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFsArchive::new(dir.path().to_string_lossy().to_string());

        let manifest = archive_policy(&graph, b"fake_wasm_bytes", &backend).await.unwrap();
        assert!(manifest.archive_storage_pointer.contains("test-policy"));

        let retrieved = retrieve_policy_manifest(&manifest.archive_storage_pointer, &backend).await.unwrap();
        assert_eq!(retrieved.graph.id, graph.id);
        assert_eq!(retrieved.graph.name, graph.name);
    }

    #[tokio::test]
    async fn test_local_fs_store_retrieve() {
        let dir = tempfile::tempdir().unwrap();
        let backend = LocalFsArchive::new(dir.path().to_string_lossy().to_string());

        let pointer = backend.store("test-key", b"test-data").await.unwrap();
        assert!(backend.exists(&pointer).await.unwrap());

        let data = backend.retrieve(&pointer).await.unwrap();
        assert_eq!(data, b"test-data");
    }

    #[test]
    fn test_governance_proposal() {
        let graph = sample_graph();
        let manifest = PolicyManifest {
            graph: graph.clone(),
            compiled_wasm_hash: Some("abc123".to_string()),
            compiled_wasm_pointer: Some("wasm:abc123".to_string()),
            schema_version: "1.0.0".to_string(),
            archive_timestamp: 0,
            archive_storage_backend: "localfs".to_string(),
            archive_storage_pointer: "/tmp/policies/test.policy".to_string(),
            validation_report: ValidationReport {
                passed: true,
                errors: vec![],
                warnings: vec![],
                node_count: 2,
                edge_count: 1,
                max_depth: 1,
            },
        };

        let proposal = generate_governance_proposal(&manifest, "0xabcd");
        assert_eq!(proposal["proposal_type"], "policy_update");
        assert_eq!(proposal["policy_graph_id"], "test-policy");
        assert_eq!(proposal["proposer"], "0xabcd");
    }
}
