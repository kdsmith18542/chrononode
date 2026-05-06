use chrononode_core::proof::{verify_proof, MerkleLeaf, MerkleProof, ProofSibling, SiblingPosition};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProofJson {
    pub version: String,
    pub chain_id: String,
    pub height: u64,
    pub block_hash: String,
    pub storage_backend: String,
    pub storage_pointer: String,
    pub checkpoint: CheckpointJson,
    pub proof: Vec<ProofSiblingJson>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckpointJson {
    pub checkpoint_id: String,
    pub start_height: u64,
    pub end_height: u64,
    pub root: String,
    pub signer_pubkey: Option<String>,
    pub signature: Option<String>,
    pub anchored_chain_id: Option<String>,
    pub anchored_tx_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProofSiblingJson {
    pub position: String,
    pub hash: String,
}

pub fn proof_to_json(proof: &MerkleProof, checkpoint_id: &str, start_height: u64) -> ProofJson {
    ProofJson {
        version: "chrononode-proof-v1".to_string(),
        chain_id: proof.leaf.chain_id.clone(),
        height: proof.leaf.height,
        block_hash: hex::encode(&proof.leaf.block_hash),
        storage_backend: proof.leaf.storage_backend.clone(),
        storage_pointer: proof.leaf.storage_pointer.clone(),
        checkpoint: CheckpointJson {
            checkpoint_id: checkpoint_id.to_string(),
            start_height,
            end_height: start_height + proof.tree_size - 1,
            root: hex::encode(proof.checkpoint_root),
            signer_pubkey: None,
            signature: None,
            anchored_chain_id: None,
            anchored_tx_hash: None,
        },
        proof: proof
            .siblings
            .iter()
            .map(|s| ProofSiblingJson {
                position: match s.position {
                    SiblingPosition::Left => "left".to_string(),
                    SiblingPosition::Right => "right".to_string(),
                },
                hash: hex::encode(s.hash),
            })
            .collect(),
    }
}

pub fn verify_proof_json(proof_json: &ProofJson) -> bool {
    let leaf = MerkleLeaf {
        chain_id: proof_json.chain_id.clone(),
        height: proof_json.height,
        block_hash: hex::decode(&proof_json.block_hash).unwrap_or_default(),
        storage_backend: proof_json.storage_backend.clone(),
        storage_pointer: proof_json.storage_pointer.clone(),
    };
    let root = hex::decode(&proof_json.checkpoint.root).unwrap_or_default();
    let root_arr: [u8; 32] = match root.try_into() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let siblings: Vec<ProofSibling> = proof_json
        .proof
        .iter()
        .map(|s| {
            let hash = hex::decode(&s.hash).unwrap_or_default();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&hash);
            ProofSibling {
                position: if s.position == "left" {
                    SiblingPosition::Left
                } else {
                    SiblingPosition::Right
                },
                hash: arr,
            }
        })
        .collect();
    let proof = MerkleProof {
        leaf,
        siblings,
        checkpoint_root: root_arr,
        leaf_index: proof_json.height,
        tree_size: proof_json.checkpoint.end_height - proof_json.checkpoint.start_height + 1,
    };
    verify_proof(&proof)
}
