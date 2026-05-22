use chrononode_core::proof::{
    MerkleProof, ProofSiblingJson, SiblingPosition,
};
pub use chrononode_core::proof::{ProofJson, CheckpointJson, verify_proof_json};

pub fn proof_to_json(
    proof: &MerkleProof,
    checkpoint_id: &str,
    start_height: u64,
    signer_pubkey: Option<[u8; 32]>,
    signature: Option<[u8; 64]>,
    anchored_chain_id: Option<String>,
    anchored_tx_hash: Option<String>,
) -> ProofJson {
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
            signer_pubkey: signer_pubkey.map(hex::encode),
            signature: signature.map(hex::encode),
            anchored_chain_id,
            anchored_tx_hash,
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
