use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TxSummary {
    pub sender: String,
    pub recipient: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockSummary {
    pub height: u64,
    pub block_hash: String,
    pub prev_hash: String,
    pub transactions: Vec<TxSummary>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GuestInput {
    pub chain_id: String,
    pub address: String,
    pub dormant_since_block: u64,
    pub current_block: u64,
    pub threshold_blocks: u64,
    pub blocks: Vec<BlockSummary>,
}

/// Helper function to convert raw byte representation of sender/recipient addresses to hex or UTF8 string.
pub fn bytes_to_address(chain_id: &str, bytes: &[u8]) -> String {
    if chain_id == "bitcoin" || chain_id == "dogecoin" {
        String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| hex::encode(bytes))
    } else if chain_id == "ethereum" {
        let h = hex::encode(bytes);
        if h.is_empty() {
            "".to_string()
        } else if h.starts_with("0x") {
            h
        } else {
            format!("0x{}", h)
        }
    } else {
        // Default hex encoding (BaaLS / others)
        hex::encode(bytes)
    }
}

/// Run SP1 Prover to generate a Groth16 zk-proof for the guest program.
/// Gated under `zkvm` feature of sp1-sdk.
pub fn generate_sp1_proof(
    elf: &[u8],
    input: &GuestInput,
    mock: bool,
) -> std::result::Result<(String, String), crate::CoreError> {
    use sp1_sdk::{ProverClient, SP1Stdin};

    let mut stdin = SP1Stdin::new();
    stdin.write(input);

    let client = ProverClient::new();

    if mock {
        // In mock mode, we execute in the VM and get public values without full Groth16 proving
        let (public_values, _) = client
            .execute(elf, stdin)
            .run()
            .map_err(|e| crate::CoreError::Adapter(format!("SP1 execution failed: {}", e)))?;
        
        let public_inputs_hex = hex::encode(public_values.as_slice());
        let mock_proof_hex = hex::encode(b"MOCK_SP1_GROTH16_PROOF_BYTES");
        Ok((mock_proof_hex, public_inputs_hex))
    } else {
        let (pk, _) = client.setup(elf);
        let proof = client
            .prove(&pk, stdin)
            .groth16()
            .run()
            .map_err(|e| crate::CoreError::Adapter(format!("SP1 proof generation failed: {}", e)))?;

        let proof_bytes = proof.bytes();
        let public_inputs_bytes = proof.public_values.as_slice();

        Ok((
            hex::encode(proof_bytes),
            hex::encode(public_inputs_bytes),
        ))
    }
}
