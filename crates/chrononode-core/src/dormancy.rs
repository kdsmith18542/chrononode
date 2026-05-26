use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DormancyStatus {
    Active,
    Dormant,
    Unknown,
}

fn default_proof_type() -> String {
    "ed25519".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DormancyProof {
    pub version: String,
    pub chain_id: String,
    pub address: String,
    pub dormant_since_block: u64,
    pub current_block: u64,
    pub threshold_blocks: u64,
    pub signer_pubkey: Option<String>,
    pub signature: Option<String>,
    pub evm_wallet: Option<String>,
    #[serde(default = "default_proof_type")]
    pub proof_type: String, // "ed25519" or "sp1_groth16"
    pub zk_proof: Option<String>,      // Hex-encoded SP1 Groth16 proof
    pub public_inputs: Option<String>, // Hex-encoded public inputs/commitments
}

impl DormancyProof {
    pub fn message_to_sign(&self) -> Vec<u8> {
        let mut msg = Vec::new();
        msg.extend_from_slice(self.version.as_bytes());
        msg.push(b':');
        msg.extend_from_slice(self.chain_id.as_bytes());
        msg.push(b':');
        msg.extend_from_slice(self.address.as_bytes());
        msg.push(b':');
        msg.extend_from_slice(&self.dormant_since_block.to_be_bytes());
        msg.extend_from_slice(&self.current_block.to_be_bytes());
        msg.extend_from_slice(&self.threshold_blocks.to_be_bytes());
        msg
    }

    pub fn sign(&mut self, keypair: &crate::signing::OperatorKeypair) {
        let msg = self.message_to_sign();
        let sig = keypair.sign(&msg);
        self.signer_pubkey = Some(hex::encode(keypair.verifying_key_bytes()));
        self.signature = Some(hex::encode(sig.to_bytes()));
    }

    pub fn verify(&self) -> bool {
        let (pubkey_hex, sig_hex) = match (&self.signer_pubkey, &self.signature) {
            (Some(p), Some(s)) => (p, s),
            _ => return false,
        };
        let pubkey_bytes = match hex::decode(pubkey_hex) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let sig_bytes = match hex::decode(sig_hex) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let pubkey_arr: [u8; 32] = match pubkey_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let sig_arr: [u8; 64] = match sig_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let msg = self.message_to_sign();
        crate::signing::verify_signature(&pubkey_arr, &sig_arr, &msg)
    }
}
