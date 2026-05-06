use sha2::{Digest, Sha256};

use crate::block::ChronoBlock;

const TAG: &[u8] = b"chrononode:v1:block";

#[derive(Debug, Clone)]
pub struct MerkleLeaf {
    pub chain_id: String,
    pub height: u64,
    pub block_hash: Vec<u8>,
    pub storage_backend: String,
    pub storage_pointer: String,
}

impl MerkleLeaf {
    pub fn from_block(block: &ChronoBlock, backend: &str, pointer: &str) -> Self {
        Self {
            chain_id: block.chain_id.clone(),
            height: block.height,
            block_hash: block.block_hash.clone(),
            storage_backend: backend.to_string(),
            storage_pointer: pointer.to_string(),
        }
    }

    pub fn leaf_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::default();
        hasher.update((TAG.len() as u16).to_be_bytes());
        hasher.update(TAG);
        hasher.update((self.chain_id.len() as u16).to_be_bytes());
        hasher.update(self.chain_id.as_bytes());
        hasher.update(self.height.to_be_bytes());
        hasher.update(&self.block_hash);
        hasher.update((self.storage_backend.len() as u16).to_be_bytes());
        hasher.update(self.storage_backend.as_bytes());
        hasher.update((self.storage_pointer.len() as u16).to_be_bytes());
        hasher.update(self.storage_pointer.as_bytes());
        hasher.finalize().into()
    }

    pub fn leaf_hash_hex(&self) -> String {
        hex::encode(self.leaf_hash())
    }
}

#[derive(Debug, Clone)]
pub struct MerkleProof {
    pub leaf: MerkleLeaf,
    pub siblings: Vec<ProofSibling>,
    pub checkpoint_root: [u8; 32],
    pub leaf_index: u64,
    pub tree_size: u64,
}

#[derive(Debug, Clone)]
pub struct ProofSibling {
    pub position: SiblingPosition,
    pub hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq)]
pub enum SiblingPosition {
    Left,
    Right,
}

fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::default();
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn build_tree_levels(leaves: &[[u8; 32]]) -> Vec<Vec<[u8; 32]>> {
    if leaves.is_empty() {
        return vec![];
    }
    let mut levels = vec![leaves.to_vec()];
    while levels.last().map_or(false, |l| l.len() > 1) {
        let prev = levels.last().unwrap();
        let mut next = Vec::with_capacity((prev.len() + 1) / 2);
        for chunk in prev.chunks(2) {
            let left = &chunk[0];
            let right = if chunk.len() > 1 { &chunk[1] } else { &chunk[0] };
            next.push(hash_pair(left, right));
        }
        levels.push(next);
    }
    levels
}

pub fn merkle_root(leaves: &[MerkleLeaf]) -> Option<[u8; 32]> {
    if leaves.is_empty() {
        return None;
    }
    let hashes: Vec<[u8; 32]> = leaves.iter().map(|l| l.leaf_hash()).collect();
    let levels = build_tree_levels(&hashes);
    levels.last().and_then(|l| l.first().copied())
}

pub fn generate_proof(leaves: &[MerkleLeaf], target_index: usize) -> Option<MerkleProof> {
    if target_index >= leaves.len() || leaves.is_empty() {
        return None;
    }
    let root = merkle_root(leaves)?;
    let leaf = leaves[target_index].clone();
    let hashes: Vec<[u8; 32]> = leaves.iter().map(|l| l.leaf_hash()).collect();
    let levels = build_tree_levels(&hashes);
    let mut siblings = Vec::new();
    let mut idx = target_index;
    for level in &levels[..levels.len() - 1] {
        if idx % 2 == 0 {
            if idx + 1 < level.len() {
                siblings.push(ProofSibling {
                    position: SiblingPosition::Right,
                    hash: level[idx + 1],
                });
            } else if level.len() % 2 == 1 {
                // Odd-length level: last element is self-duplicated.
                // Include itself as the sibling so the verifier computes hash(h, h).
                siblings.push(ProofSibling {
                    position: SiblingPosition::Right,
                    hash: level[idx],
                });
            }
        } else {
            siblings.push(ProofSibling {
                position: SiblingPosition::Left,
                hash: level[idx - 1],
            });
        }
        idx /= 2;
    }
    Some(MerkleProof {
        leaf,
        siblings,
        checkpoint_root: root,
        leaf_index: target_index as u64,
        tree_size: leaves.len() as u64,
    })
}

pub fn verify_proof(proof: &MerkleProof) -> bool {
    let mut current = proof.leaf.leaf_hash();
    let mut idx = proof.leaf_index;
    for sibling in &proof.siblings {
        current = match sibling.position {
            SiblingPosition::Left => hash_pair(&sibling.hash, &current),
            SiblingPosition::Right => hash_pair(&current, &sibling.hash),
        };
        idx /= 2;
    }
    current == proof.checkpoint_root
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_leaf(height: usize) -> MerkleLeaf {
        MerkleLeaf {
            chain_id: "test".to_string(),
            height: height as u64,
            block_hash: vec![height as u8; 32],
            storage_backend: "local_fs".to_string(),
            storage_pointer: format!("blocks/test/{}.block", height),
        }
    }

    #[test]
    fn test_single_leaf_tree() {
        let leaves = vec![make_leaf(0)];
        let root = merkle_root(&leaves).unwrap();
        assert_eq!(root, leaves[0].leaf_hash());
    }

    #[test]
    fn test_two_leaf_tree() {
        let leaves = vec![make_leaf(0), make_leaf(1)];
        let root = merkle_root(&leaves).unwrap();
        let expected = hash_pair(&leaves[0].leaf_hash(), &leaves[1].leaf_hash());
        assert_eq!(root, expected);
    }

    #[test]
    fn test_proof_verification_roundtrip() {
        for size in [1usize, 2, 3, 10, 100, 101] {
            let leaves: Vec<MerkleLeaf> = (0..size).map(make_leaf).collect();
            for i in [0, 1, size / 2, size - 1] {
                let idx = i as usize;
                if idx < size {
                    let proof = generate_proof(&leaves, idx).unwrap();
                    assert!(verify_proof(&proof), "size={} index={} failed", size, idx);
                }
            }
        }
    }

    #[test]
    fn test_proof_rejects_wrong_leaf() {
        let leaves: Vec<MerkleLeaf> = (0..10).map(make_leaf).collect();
        let mut proof = generate_proof(&leaves, 0).unwrap();
        proof.leaf.height = 999;
        assert!(!verify_proof(&proof));
    }

    #[test]
    fn test_domain_separation_no_collision() {
        let a = MerkleLeaf {
            chain_id: "chain1".to_string(),
            height: 123,
            block_hash: vec![0; 32],
            storage_backend: "fs".to_string(),
            storage_pointer: "p1".to_string(),
        };
        let b = MerkleLeaf {
            chain_id: "c".to_string(),
            height: 1123123,
            block_hash: vec![0; 32],
            storage_backend: "ha".to_string(),
            storage_pointer: "in1p1".to_string(),
        };
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn test_odd_size_trees() {
        for size in [1usize, 3, 5, 7, 99, 101] {
            let leaves: Vec<MerkleLeaf> = (0..size).map(make_leaf).collect();
            let root = merkle_root(&leaves).unwrap();
            for i in 0..size {
                let proof = generate_proof(&leaves, i).unwrap();
                assert!(verify_proof(&proof), "odd size {} index {}", size, i);
            }
        }
    }
}
