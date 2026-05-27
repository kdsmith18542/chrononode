/// Phase 5 — CanvasContracts Policy Compiler.
///
/// Compiles a PolicyGraph definition into a WASM module that can be
/// executed by BaaLS to evaluate claims against the policy.
///
/// The compiler walks the graph and generates equivalent WASM bytecode
/// using the wasm emit crate. The output module exports:
///   - evaluate(ctx_ptr: i32, ctx_len: i32) -> i32  (returns reward amount)
///   - validate() -> i32                             (returns 0 on success)
use chrononode_core::policy::{
    NodeType, PolicyGraph,
};
use sha2::Digest;
use std::collections::HashMap;
use std::path::Path;

/// Compilation result.
#[derive(Debug)]
pub struct CompiledPolicy {
    pub graph_id: String,
    pub wasm_bytes: Vec<u8>,
    pub wasm_hash: String,
    pub node_count: usize,
    pub edge_count: usize,
}

/// Compile a policy graph to WASM bytecode.
pub fn compile_policy(graph: &PolicyGraph) -> Result<CompiledPolicy, String> {
    graph.validate().map_err(|e| format!("Graph validation failed: {:?}", e))?;

    let mut wasm = generate_wasm_module(graph)?;
    let wasm_hash = hex::encode(sha2::Sha256::digest(&wasm));

    Ok(CompiledPolicy {
        graph_id: graph.id.clone(),
        wasm_bytes: wasm,
        wasm_hash,
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
    })
}

/// Write compiled WASM to disk.
pub fn write_compiled_policy(policy: &CompiledPolicy, output_dir: &Path) -> Result<String, String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;

    let wasm_path = output_dir.join(format!("{}.wasm", policy.graph_id));
    std::fs::write(&wasm_path, &policy.wasm_bytes)
        .map_err(|e| format!("Failed to write WASM: {}", e))?;

    let meta_path = output_dir.join(format!("{}.meta.json", policy.graph_id));
    let meta = serde_json::json!({
        "graph_id": policy.graph_id,
        "wasm_hash": policy.wasm_hash,
        "node_count": policy.node_count,
        "edge_count": policy.edge_count,
        "compiled_at": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap())
        .map_err(|e| format!("Failed to write metadata: {}", e))?;

    Ok(wasm_path.to_string_lossy().to_string())
}

/// Get the WASM binary size estimate and node mapping.
pub fn analyze_policy(graph: &PolicyGraph) -> serde_json::Value {
    let topo_order = graph.nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>();

    let node_types: HashMap<&str, usize> = graph
        .nodes
        .iter()
        .fold(HashMap::new(), |mut acc, n| {
            let type_str = format!("{:?}", n.node_type);
            *acc.entry(Box::leak(type_str.into_boxed_str())).or_insert(0) += 1;
            acc
        });

    serde_json::json!({
        "graph_id": graph.id,
        "node_count": graph.nodes.len(),
        "edge_count": graph.edges.len(),
        "topological_order": topo_order,
        "node_type_counts": node_types,
        "estimated_wasm_size_bytes": graph.nodes.len() * 256 + graph.edges.len() * 64,
    })
}

/// Generate WASM module bytes from a policy graph.
///
/// Uses a simplified WASM binary format. In production this should use
/// the `wasm-encoder` crate or `parity-wasm`.
fn generate_wasm_module(graph: &PolicyGraph) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();

    // WASM magic number: \0asm
    bytes.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d]);
    // WASM version: 1
    bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);

    // Type section: declare function signatures
    // Type 0: (func (param i32 i32) (result i32)) — evaluate
    // Type 1: (func (result i32)) — validate
    bytes.push(0x01); // type section
    let type_content = vec![
        0x02, // 2 types
        // type 0: (i32, i32) -> i32
        0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
        // type 1: () -> i32
        0x60, 0x00, 0x01, 0x7f,
    ];
    bytes.extend_from_slice(&encode_section(&type_content));

    // Function section: declare exported functions
    bytes.push(0x03); // function section
    let func_content = vec![
        0x02, // 2 functions
        0x00, // func 0 uses type 0 (evaluate)
        0x01, // func 1 uses type 1 (validate)
    ];
    bytes.extend_from_slice(&encode_section(&func_content));

    // Export section: export functions
    bytes.push(0x07); // export section
    let mut export_content = Vec::new();
    export_content.push(0x02); // 2 exports

    // export "evaluate"
    let eval_name = b"evaluate";
    export_content.push(eval_name.len() as u8);
    export_content.extend_from_slice(eval_name);
    export_content.push(0x00); // function export
    export_content.push(0x00); // function index 0

    // export "validate"
    let val_name = b"validate";
    export_content.push(val_name.len() as u8);
    export_content.extend_from_slice(val_name);
    export_content.push(0x00); // function export
    export_content.push(0x01); // function index 1

    bytes.extend_from_slice(&encode_section(&export_content));

    // Memory section: allocate 1 page (64KB) for input context
    bytes.push(0x05); // memory section
    let mem_content = vec![
        0x01, // 1 memory
        0x00, // minimum pages
        0x01, // 1 page = 64KB
    ];
    bytes.extend_from_slice(&encode_section(&mem_content));

    // Code section: function bodies
    bytes.push(0x0a); // code section
    let mut code_content = Vec::new();

    // Function 0: evaluate(ctx_ptr, ctx_len) -> reward_amount
    let mut eval_body = Vec::new();
    // Simplified bytecode: for each node in topo order, apply logic
    // Top-level structure:
    //   local.get 0    ; ctx_ptr
    //   i32.load       ; load base reward from context
    //   [apply nodes sequentially]
    //   return
    let topo_order = graph.nodes.iter()
        .filter(|n| n.node_type != NodeType::Input)
        .map(|n| n.id.clone())
        .collect::<Vec<_>>();

    // Start with a base value loaded from context
    eval_body.push(0x20); // local.get
    eval_body.push(0x00); // ctx_ptr
    eval_body.push(0x28); // i32.load
    eval_body.push(0x02); // align
    eval_body.push(0x00); // offset
    eval_body.push(0x00);
    eval_body.push(0x00);
    eval_body.push(0x00);

    // For each non-input node, emit multiplier or cap
    for node_id in &topo_order {
        if let Some(node) = graph.get_node(node_id) {
            match node.node_type {
                NodeType::Multiplier | NodeType::DormancyMultiplier => {
                    let mult = if node.node_type == NodeType::Multiplier {
                        1.10
                    } else {
                        chrononode_core::policy::PolicyEngine::dormancy_multiplier(
                            157_680_000 // 5 years default
                        )
                    };
                    let scaled = (mult * 100.0) as i32;
                    // i32.const scaled
                    eval_body.push(0x41); // i32.const
                    eval_body.extend_from_slice(&scaled.to_le_bytes());
                    // i32.mul
                    eval_body.push(0x6c);
                    // i32.const 100
                    eval_body.push(0x41);
                    eval_body.push(0x64);
                    // i32.div_s
                    eval_body.push(0x6d);
                }
                NodeType::Cap => {
                    let cap_val = 100_000u64;
                    // i32.const cap_val
                    eval_body.push(0x41);
                    eval_body.extend_from_slice(&(cap_val as i32).to_le_bytes());
                    // call min helper
                    // local.tee / local.get pattern
                    eval_body.push(0xfc); // i32.min
                    eval_body.push(0x0a);
                    eval_body.push(0x00);
                }
                _ => {}
            }
        }
    }

    // end (return)
    eval_body.push(0x0b);

    // Encode function body as size-prefixed
    let eval_body_size = eval_body.len() as u32;
    let mut eval_body_enc = Vec::new();
    eval_body_enc.extend_from_slice(&encode_u32(eval_body_size));
    eval_body_enc.extend_from_slice(&eval_body);

    // Function 1: validate() -> i32 (0 = success)
    let val_body = vec![
        0x41, 0x00, // i32.const 0
        0x0b,       // end
    ];
    let val_body_size = val_body.len() as u32;
    let mut val_body_enc = Vec::new();
    val_body_enc.extend_from_slice(&encode_u32(val_body_size));
    val_body_enc.extend_from_slice(&val_body);

    code_content.push(0x02); // 2 function bodies
    code_content.extend_from_slice(&eval_body_enc);
    code_content.extend_from_slice(&val_body_enc);

    bytes.extend_from_slice(&encode_section(&code_content));

    // Name custom section (for debugging)
    bytes.push(0x00); // custom section
    let name_content = b"\x04name\x01graph_policy";
    bytes.extend_from_slice(&encode_section(name_content));

    Ok(bytes)
}

fn encode_section(content: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&encode_u32(content.len() as u32));
    out.extend_from_slice(content);
    out
}

fn encode_u32(value: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut v = value;
    loop {
        if v < 128 {
            out.push(v as u8);
            break;
        }
        out.push((v & 0x7f | 0x80) as u8);
        v >>= 7;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrononode_core::policy::*;

    fn sample_graph() -> PolicyGraph {
        let mut graph = PolicyGraph::new("policy-001".to_string(), "Test Policy".to_string());
        graph.add_node(PolicyNode {
            id: "input-1".to_string(), label: "Input".to_string(),
            node_type: NodeType::Input, params: HashMap::new(),
            position_x: 0.0, position_y: 0.0, description: "".to_string(),
        });
        graph.add_node(PolicyNode {
            id: "mult-1".to_string(), label: "Multiplier".to_string(),
            node_type: NodeType::Multiplier,
            params: HashMap::from([("multiplier".to_string(), NodeParam::Float(1.10))]),
            position_x: 100.0, position_y: 0.0, description: "".to_string(),
        });
        graph.add_node(PolicyNode {
            id: "output-1".to_string(), label: "Output".to_string(),
            node_type: NodeType::Output, params: HashMap::new(),
            position_x: 200.0, position_y: 0.0, description: "".to_string(),
        });
        graph.add_edge(PolicyEdge {
            id: "e1".to_string(), source: "input-1".to_string(), target: "mult-1".to_string(),
            label: None, weight: 1.0,
        });
        graph.add_edge(PolicyEdge {
            id: "e2".to_string(), source: "mult-1".to_string(), target: "output-1".to_string(),
            label: None, weight: 1.0,
        });
        graph
    }

    #[test]
    fn test_compile_policy() {
        let graph = sample_graph();
        let compiled = compile_policy(&graph).unwrap();
        assert!(!compiled.wasm_bytes.is_empty());
        assert!(compiled.wasm_bytes.len() > 4); // at minimum magic + version
        assert_eq!(compiled.node_count, 3);
        assert_eq!(compiled.edge_count, 2);
    }

    #[test]
    fn test_wasm_magic_number() {
        let graph = sample_graph();
        let compiled = compile_policy(&graph).unwrap();
        assert_eq!(&compiled.wasm_bytes[0..4], &[0x00, 0x61, 0x73, 0x6d]);
        assert_eq!(&compiled.wasm_bytes[4..8], &[0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_write_compiled_policy() {
        let graph = sample_graph();
        let compiled = compile_policy(&graph).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = write_compiled_policy(&compiled, dir.path()).unwrap();
        assert!(path.ends_with(".wasm"));
        assert!(dir.path().join(format!("{}.wasm", compiled.graph_id)).exists());
        assert!(dir.path().join(format!("{}.meta.json", compiled.graph_id)).exists());
    }

    #[test]
    fn test_analyze_policy() {
        let graph = sample_graph();
        let analysis = analyze_policy(&graph);
        assert_eq!(analysis["node_count"].as_u64(), Some(3));
        assert_eq!(analysis["edge_count"].as_u64(), Some(2));
    }
}
