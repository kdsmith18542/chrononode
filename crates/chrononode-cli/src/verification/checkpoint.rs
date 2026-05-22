use chrononode_core::proof::{self, MerkleLeaf};
use chrononode_core::{ChronoBlock, CoreConfig, OperatorKeypair, Result, StoragePointer};

pub struct CheckpointBuilder {
    config: CoreConfig,
    keypair: Option<OperatorKeypair>,
}

impl CheckpointBuilder {
    pub fn new(config: CoreConfig) -> Self {
        Self {
            config,
            keypair: None,
        }
    }

    pub fn with_keypair(mut self, keypair: OperatorKeypair) -> Self {
        self.keypair = Some(keypair);
        self
    }

    pub fn build_checkpoint(
        &self,
        blocks: &[(ChronoBlock, StoragePointer)],
        chain_id: &str,
        start_height: u64,
    ) -> Result<CheckpointResult> {
        let leaves: Vec<MerkleLeaf> = blocks
            .iter()
            .map(|(b, p)| MerkleLeaf::from_block(b, &p.backend, &p.to_string()))
            .collect();
        let root = proof::merkle_root(&leaves)
            .ok_or_else(|| chrononode_core::CoreError::Proof("empty checkpoint".to_string()))?;

        let (signer_pubkey, signature) = if let Some(ref kp) = self.keypair {
            let sig = kp.sign(&root);
            (Some(kp.verifying_key_bytes()), Some(sig.to_bytes()))
        } else {
            (None, None)
        };

        Ok(CheckpointResult {
            checkpoint_id: format!(
                "{}-{}-{}",
                chain_id,
                start_height,
                start_height + blocks.len() as u64 - 1
            ),
            chain_id: chain_id.to_string(),
            start_height,
            end_height: start_height + blocks.len() as u64 - 1,
            root_hash: root,
            leaf_count: leaves.len() as u64,
            leaves,
            signer_pubkey,
            signature,
        })
    }

    pub fn checkpoint_size(&self) -> u64 {
        self.config.checkpoint_size
    }
}

pub struct CheckpointResult {
    pub checkpoint_id: String,
    pub chain_id: String,
    pub start_height: u64,
    pub end_height: u64,
    pub root_hash: [u8; 32],
    pub leaf_count: u64,
    pub leaves: Vec<MerkleLeaf>,
    pub signer_pubkey: Option<[u8; 32]>,
    pub signature: Option<[u8; 64]>,
}
