/// Phase 5 — CanvasContracts Policy Engine.
///
/// Defines the reward policy graph data model:
/// - PolicyNode: individual computation/decision node
/// - PolicyEdge: directed edge connecting nodes
/// - PolicyGraph: complete directed acyclic graph (DAG) defining a policy
/// - PolicyManifest: compiled + archived policy artifact
/// - PolicyEngine: runtime evaluator
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a policy node.
pub type NodeId = String;

/// Supported node types in the policy graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// Entry point — receives a DormancyEvidence or LegacyClaim
    Input,
    /// Filter — gates on conditions (chain_id, min_balance, min_dormancy, etc.)
    Filter,
    /// Confidence tier assigner — maps evidence source → tier
    ConfidenceAssigner,
    /// Multiplier — scales the reward (e.g., 1.10x for burn)
    Multiplier,
    /// Dormancy multiplier — scales by years dormant
    DormancyMultiplier,
    /// Cap — enforces a maximum (per-claim, per-wallet, per-chain)
    Cap,
    /// Weighted sum — combines multiple inputs
    WeightedSum,
    /// Custom WASM — user-defined logic compiled to WASM
    CustomWasm,
    /// Output — terminal node that produces final reward amount
    Output,
}

/// Parameter values for node configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeParam {
    String(String),
    Uint(u64),
    Int(i64),
    Float(f64),
    Bool(bool),
    StringList(Vec<String>),
    UintRange { min: u64, max: u64 },
    ConfidenceTier(u8),
    Selector { options: Vec<String>, selected: Option<String> },
}

/// A single node in the policy graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyNode {
    pub id: NodeId,
    pub label: String,
    pub node_type: NodeType,
    pub params: HashMap<String, NodeParam>,
    pub position_x: f64,
    pub position_y: f64,
    pub description: String,
}

/// A directed edge connecting two policy nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEdge {
    pub id: String,
    pub source: NodeId,
    pub target: NodeId,
    pub label: Option<String>,
    pub weight: f64,
}

/// The complete policy graph definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyGraph {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub nodes: Vec<PolicyNode>,
    pub edges: Vec<PolicyEdge>,
    pub created_at: u64,
    pub updated_at: u64,
    pub tags: Vec<String>,
}

impl PolicyGraph {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            description: String::new(),
            version: "0.1.0".to_string(),
            author: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            updated_at: 0,
            tags: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: PolicyNode) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: PolicyEdge) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.edges.push(edge);
    }

    pub fn get_node(&self, id: &str) -> Option<&PolicyNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn get_outputs(&self) -> Vec<&PolicyNode> {
        self.nodes.iter().filter(|n| n.node_type == NodeType::Output).collect()
    }

    pub fn get_inputs(&self) -> Vec<&PolicyNode> {
        self.nodes.iter().filter(|n| n.node_type == NodeType::Input).collect()
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.nodes.is_empty() {
            errors.push("Graph must have at least one node".to_string());
        }

        let inputs = self.get_inputs();
        if inputs.is_empty() {
            errors.push("Graph must have at least one Input node".to_string());
        }

        let outputs = self.get_outputs();
        if outputs.is_empty() {
            errors.push("Graph must have at least one Output node".to_string());
        }

        for node in &self.nodes {
            let incoming = self.edges.iter().filter(|e| e.target == node.id).count();
            if node.node_type != NodeType::Input && incoming == 0 {
                errors.push(format!("Node '{}' ({}) has no incoming edges", node.label, node.id));
            }
            let outgoing = self.edges.iter().filter(|e| e.source == node.id).count();
            if node.node_type != NodeType::Output && outgoing == 0 {
                errors.push(format!("Node '{}' ({}) has no outgoing edges", node.label, node.id));
            }
        }

        // Check for cycles (simple DFS)
        if self.has_cycle() {
            errors.push("Graph contains a cycle; must be a DAG".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn has_cycle(&self) -> bool {
        let adj: HashMap<&str, Vec<&str>> = self
            .edges
            .iter()
            .fold(HashMap::new(), |mut acc, e| {
                acc.entry(e.source.as_str()).or_default().push(e.target.as_str());
                acc
            });

        let mut visited = HashMap::new();
        for node in &self.nodes {
            if self.dfs_cycle(&adj, node.id.as_str(), &mut visited) {
                return true;
            }
        }
        false
    }

    fn dfs_cycle<'a>(
        &self,
        adj: &HashMap<&'a str, Vec<&'a str>>,
        node: &'a str,
        visited: &mut HashMap<&'a str, bool>,
    ) -> bool {
        if visited.contains_key(node) {
            return *visited.get(node).unwrap_or(&false) == false;
        }
        visited.insert(node, false);
        if let Some(neighbors) = adj.get(node) {
            for &next in neighbors {
                if self.dfs_cycle(adj, next, visited) {
                    return true;
                }
            }
        }
        visited.insert(node, true);
        false
    }
}

/// A compiled policy artifact ready for archiving.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyManifest {
    pub graph: PolicyGraph,
    pub compiled_wasm_hash: Option<String>,
    pub compiled_wasm_pointer: Option<String>,
    pub schema_version: String,
    pub archive_timestamp: u64,
    pub archive_storage_backend: String,
    pub archive_storage_pointer: String,
    pub validation_report: ValidationReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub passed: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub node_count: usize,
    pub edge_count: usize,
    pub max_depth: usize,
}

/// Input context passed to the policy engine for evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyContext {
    pub chain_id: String,
    pub claim_type: u8,
    pub confidence_tier: u8,
    pub dormancy_seconds: u64,
    pub reward_amount: u64,
    pub campaign_id: u64,
    pub source_chain_reputation: u8,
    pub additional_params: HashMap<String, String>,
}

/// Output from evaluating a policy graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEvaluation {
    pub final_reward: u64,
    pub applied_multipliers: Vec<MultiplierApplication>,
    pub capped: bool,
    pub confidence_tier_used: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiplierApplication {
    pub node_id: String,
    pub node_label: String,
    pub multiplier: f64,
    pub input_value: u64,
    pub output_value: u64,
}

/// Policy engine that walks the graph DAG and evaluates nodes.
#[derive(Debug)]
pub struct PolicyEngine;

impl PolicyEngine {
    pub fn evaluate(graph: &PolicyGraph, context: &PolicyContext) -> Result<PolicyEvaluation, String> {
        graph.validate().map_err(|e| format!("Invalid graph: {:?}", e))?;

        let adj: HashMap<&str, Vec<(&str, f64)>> = self::build_adjacency(graph);
        let topo = self::topological_sort(graph, &adj)?;

        let mut node_values: HashMap<&str, u64> = HashMap::new();
        let mut multipliers: Vec<MultiplierApplication> = Vec::new();

        for node_id in &topo {
            let node = graph.get_node(node_id).ok_or("Node not found")?;
            let value = match node.node_type {
                NodeType::Input => {
                    context.reward_amount
                }
                NodeType::Filter => {
                    let val = Self::evaluate_filter(node, context)?;
                    node_values.get(node_id).copied().unwrap_or(val)
                }
                NodeType::Multiplier => {
                    let input_val = Self::get_input_value(node_id, &adj, &node_values);
                    let multiplier = Self::get_multiplier(node, context);
                    let output = (input_val as f64 * multiplier) as u64;
                    multipliers.push(MultiplierApplication {
                        node_id: node.id.clone(),
                        node_label: node.label.clone(),
                        multiplier,
                        input_value: input_val,
                        output_value: output,
                    });
                    output
                }
                NodeType::DormancyMultiplier => {
                    let input_val = Self::get_input_value(node_id, &adj, &node_values);
                    let multiplier = Self::dormancy_multiplier(context.dormancy_seconds);
                    let output = (input_val as f64 * multiplier) as u64;
                    multipliers.push(MultiplierApplication {
                        node_id: node.id.clone(),
                        node_label: node.label.clone(),
                        multiplier,
                        input_value: input_val,
                        output_value: output,
                    });
                    output
                }
                NodeType::ConfidenceAssigner => {
                    context.confidence_tier as u64
                }
                NodeType::Cap => {
                    let input_val = Self::get_input_value(node_id, &adj, &node_values);
                    let cap = Self::get_cap(node, context);
                    input_val.min(cap)
                }
                NodeType::WeightedSum => {
                    let mut sum = 0u64;
                    for (src, weight) in adj.get(&**node_id).unwrap_or(&vec![]) {
                        if let Some(&v) = node_values.get(src) {
                            sum = (sum as f64 + v as f64 * weight) as u64;
                        }
                    }
                    sum
                }
                NodeType::CustomWasm => {
                    return Err("CustomWasm node evaluation requires BaaLS WASM runtime (use policy_runtime.rs)".into());
                }
                NodeType::Output => {
                    Self::get_input_value(node_id, &adj, &node_values)
                }
            };
            node_values.insert(node_id, value);
        }

        let final_value = graph.get_outputs().first()
            .and_then(|o| node_values.get(o.id.as_str()))
            .copied()
            .unwrap_or(0);

        Ok(PolicyEvaluation {
            final_reward: final_value,
            applied_multipliers: multipliers,
            capped: false,
            confidence_tier_used: context.confidence_tier,
        })
    }

    fn get_input_value(node_id: &str, adj: &HashMap<&str, Vec<(&str, f64)>>, values: &HashMap<&str, u64>) -> u64 {
        adj.iter()
            .filter(|(_, edges)| edges.iter().any(|(t, _)| *t == node_id))
            .filter_map(|(src, _)| values.get(src))
            .copied()
            .next()
            .unwrap_or(0)
    }

    fn evaluate_filter(node: &PolicyNode, context: &PolicyContext) -> Result<u64, String> {
        for (key, param) in &node.params {
            match key.as_ref() {
                "min_confidence_tier" => {
                    if let NodeParam::ConfidenceTier(min_tier) = param {
                        if context.confidence_tier > *min_tier {
                            return Err(format!("Confidence tier {} below minimum {}", context.confidence_tier, min_tier));
                        }
                    }
                }
                "min_dormancy_seconds" => {
                    if let NodeParam::Uint(min_secs) = param {
                        if context.dormancy_seconds < *min_secs {
                            return Err(format!("Dormancy {}s below minimum {}s", context.dormancy_seconds, min_secs));
                        }
                    }
                }
                "allowed_claim_types" => {
                    if let NodeParam::Selector { options, selected } = param {
                        if let Some(sel) = selected {
                            let ct = context.claim_type.to_string();
                            if *sel != ct && !options.contains(&ct) {
                                return Err(format!("Claim type {} not in allowed set", context.claim_type));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(context.reward_amount)
    }

    fn get_multiplier(node: &PolicyNode, context: &PolicyContext) -> f64 {
        for (key, param) in &node.params {
            match key.as_ref() {
                "multiplier" => {
                    if let NodeParam::Float(m) = param {
                        return *m;
                    }
                    if let NodeParam::Uint(m) = param {
                        return *m as f64;
                    }
                }
                "claim_type_multipliers" => {
                    if let NodeParam::StringList(list) = param {
                        for entry in list {
                            let parts: Vec<&str> = entry.split(':').collect();
                            if parts.len() == 2 {
                                if let Ok(ct) = parts[0].parse::<u8>() {
                                    if ct == context.claim_type {
                                        if let Ok(m) = parts[1].parse::<f64>() {
                                            return m;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        1.0
    }

    pub fn dormancy_multiplier(dormancy_seconds: u64) -> f64 {
        let years = dormancy_seconds as f64 / 31_536_000.0;
        if years >= 10.0 { 3.0 }
        else if years >= 5.0 { 2.0 }
        else if years >= 3.0 { 1.5 }
        else if years >= 1.0 { 1.0 }
        else { 0.5 }
    }

    fn get_cap(node: &PolicyNode, _context: &PolicyContext) -> u64 {
        for (key, param) in &node.params {
            match key.as_ref() {
                "max_value" => {
                    if let NodeParam::Uint(max) = param {
                        return *max;
                    }
                }
                _ => {}
            }
        }
        u64::MAX
    }
}

fn build_adjacency(graph: &PolicyGraph) -> HashMap<&str, Vec<(&str, f64)>> {
    let mut adj: HashMap<&str, Vec<(&str, f64)>> = HashMap::new();
    for edge in &graph.edges {
        adj.entry(edge.source.as_str())
            .or_default()
            .push((edge.target.as_str(), edge.weight));
    }
    adj
}

fn topological_sort<'a>(
    graph: &'a PolicyGraph,
    adj: &HashMap<&'a str, Vec<(&'a str, f64)>>,
) -> Result<Vec<&'a str>, String> {
    let mut visited: HashMap<&str, bool> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();

    fn dfs<'a>(
        node: &'a str,
        adj: &HashMap<&'a str, Vec<(&'a str, f64)>>,
        visited: &mut HashMap<&'a str, bool>,
        order: &mut Vec<&'a str>,
    ) {
        if visited.contains_key(node) {
            return;
        }
        visited.insert(node, true);
        if let Some(neighbors) = adj.get(node) {
            for (next, _) in neighbors {
                dfs(next, adj, visited, order);
            }
        }
        order.push(node);
    }

    for node in &graph.nodes {
        if !visited.contains_key(node.id.as_str()) {
            dfs(node.id.as_str(), adj, &mut visited, &mut order);
        }
    }

    order.reverse();
    Ok(order)
}

/// Serializes a policy graph to JSON for archiving.
pub fn serialize_policy_graph(graph: &PolicyGraph) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(graph)
}

/// Deserializes a policy graph from JSON.
pub fn deserialize_policy_graph(json: &str) -> Result<PolicyGraph, serde_json::Error> {
    serde_json::from_str(json)
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_graph() -> PolicyGraph {
        let mut graph = PolicyGraph::new("policy-001".to_string(), "Basic Burn Reward".to_string());

        graph.add_node(PolicyNode {
            id: "input-1".to_string(),
            label: "Claim Input".to_string(),
            node_type: NodeType::Input,
            params: HashMap::new(),
            position_x: 100.0,
            position_y: 200.0,
            description: "Receives the raw claim".to_string(),
        });

        graph.add_node(PolicyNode {
            id: "filter-1".to_string(),
            label: "Min Confidence".to_string(),
            node_type: NodeType::Filter,
            params: HashMap::from([
                ("min_confidence_tier".to_string(), NodeParam::ConfidenceTier(2)),
            ]),
            position_x: 300.0,
            position_y: 200.0,
            description: "Requires confidence tier ≤ 2".to_string(),
        });

        graph.add_node(PolicyNode {
            id: "mult-1".to_string(),
            label: "Burn Multiplier".to_string(),
            node_type: NodeType::Multiplier,
            params: HashMap::from([
                ("multiplier".to_string(), NodeParam::Float(1.10)),
            ]),
            position_x: 500.0,
            position_y: 200.0,
            description: "10% boost for burn proof".to_string(),
        });

        graph.add_node(PolicyNode {
            id: "output-1".to_string(),
            label: "Final Reward".to_string(),
            node_type: NodeType::Output,
            params: HashMap::new(),
            position_x: 700.0,
            position_y: 200.0,
            description: "Terminal output".to_string(),
        });

        graph.add_edge(PolicyEdge {
            id: "e1".to_string(),
            source: "input-1".to_string(),
            target: "filter-1".to_string(),
            label: Some("pass".to_string()),
            weight: 1.0,
        });
        graph.add_edge(PolicyEdge {
            id: "e2".to_string(),
            source: "filter-1".to_string(),
            target: "mult-1".to_string(),
            label: None,
            weight: 1.0,
        });
        graph.add_edge(PolicyEdge {
            id: "e3".to_string(),
            source: "mult-1".to_string(),
            target: "output-1".to_string(),
            label: None,
            weight: 1.0,
        });

        graph
    }

    #[test]
    fn test_graph_validation() {
        let graph = sample_graph();
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn test_policy_evaluation_burn() {
        let graph = sample_graph();
        let context = PolicyContext {
            chain_id: "bitcoin".to_string(),
            claim_type: 2,
            confidence_tier: 1,
            dormancy_seconds: 157_680_000,
            reward_amount: 1000,
            campaign_id: 0,
            source_chain_reputation: 100,
            additional_params: HashMap::new(),
        };

        let result = PolicyEngine::evaluate(&graph, &context).unwrap();
        assert_eq!(result.final_reward, 1100); // 1000 * 1.10
        assert_eq!(result.applied_multipliers.len(), 1);
        assert!((result.applied_multipliers[0].multiplier - 1.10).abs() < 0.001);
    }

    #[test]
    fn test_dormancy_multiplier_logic() {
        assert!((PolicyEngine::dormancy_multiplier(0) - 0.5).abs() < 0.001);
        assert!((PolicyEngine::dormancy_multiplier(31_536_000) - 1.0).abs() < 0.001);
        assert!((PolicyEngine::dormancy_multiplier(94_608_000) - 1.5).abs() < 0.001);
        assert!((PolicyEngine::dormancy_multiplier(157_680_000) - 2.0).abs() < 0.001);
        assert!((PolicyEngine::dormancy_multiplier(315_360_000) - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let graph = sample_graph();
        let json = serialize_policy_graph(&graph).unwrap();
        let deserialized = deserialize_policy_graph(&json).unwrap();
        assert_eq!(deserialized.name, graph.name);
        assert_eq!(deserialized.nodes.len(), graph.nodes.len());
        assert_eq!(deserialized.edges.len(), graph.edges.len());
    }

    #[test]
    fn test_invalid_graph_no_input() {
        let mut graph = PolicyGraph::new("bad".to_string(), "Bad".to_string());
        graph.add_node(PolicyNode {
            id: "out".to_string(),
            label: "Output".to_string(),
            node_type: NodeType::Output,
            params: HashMap::new(),
            position_x: 0.0,
            position_y: 0.0,
            description: "".to_string(),
        });
        assert!(graph.validate().is_err());
    }

    #[test]
    fn test_cap_node() {
        let mut graph = PolicyGraph::new("cap-test".to_string(), "Cap Test".to_string());
        graph.add_node(PolicyNode {
            id: "i".to_string(), label: "Input".to_string(),
            node_type: NodeType::Input, params: HashMap::new(),
            position_x: 0.0, position_y: 0.0, description: "".to_string(),
        });
        graph.add_node(PolicyNode {
            id: "c".to_string(), label: "Cap".to_string(),
            node_type: NodeType::Cap,
            params: HashMap::from([("max_value".to_string(), NodeParam::Uint(500))]),
            position_x: 100.0, position_y: 0.0, description: "".to_string(),
        });
        graph.add_node(PolicyNode {
            id: "o".to_string(), label: "Out".to_string(),
            node_type: NodeType::Output, params: HashMap::new(),
            position_x: 200.0, position_y: 0.0, description: "".to_string(),
        });
        graph.add_edge(PolicyEdge { id: "e1".to_string(), source: "i".to_string(), target: "c".to_string(), label: None, weight: 1.0 });
        graph.add_edge(PolicyEdge { id: "e2".to_string(), source: "c".to_string(), target: "o".to_string(), label: None, weight: 1.0 });

        let context = PolicyContext {
            chain_id: "btc".to_string(), claim_type: 0, confidence_tier: 1,
            dormancy_seconds: 0, reward_amount: 1000, campaign_id: 0,
            source_chain_reputation: 50, additional_params: HashMap::new(),
        };

        let result = PolicyEngine::evaluate(&graph, &context).unwrap();
        assert_eq!(result.final_reward, 500);
    }
}
