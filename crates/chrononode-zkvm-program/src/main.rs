#![no_main]
sp1_zkvm::entrypoint!(main);

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

pub fn main() {
    // Read input from the host
    let input: GuestInput = sp1_zkvm::io::read();

    // Verify dormancy window is valid
    assert!(
        input.current_block >= input.dormant_since_block,
        "current_block must be >= dormant_since_block"
    );
    let diff = input.current_block - input.dormant_since_block;
    assert!(
        diff >= input.threshold_blocks,
        "dormancy window does not satisfy threshold"
    );

    // Verify contiguous block chain validation
    if !input.blocks.is_empty() {
        assert_eq!(
            input.blocks[0].height,
            input.dormant_since_block,
            "First block height must match dormant_since_block"
        );
        assert_eq!(
            input.blocks[input.blocks.len() - 1].height,
            input.current_block,
            "Last block height must match current_block"
        );

        for i in 1..input.blocks.len() {
            assert_eq!(
                input.blocks[i].height,
                input.blocks[i - 1].height + 1,
                "Block heights must be contiguous"
            );
            assert_eq!(
                input.blocks[i].prev_hash,
                input.blocks[i - 1].block_hash,
                "Block hash linkage must form a valid chain"
            );
        }
    }

    // Verify no activity from the target address
    for block in &input.blocks {
        for tx in &block.transactions {
            // Check that the watched address is neither sender nor receiver of any transaction
            assert!(
                tx.sender != input.address,
                "Outbound transaction found at block {}",
                block.height
            );
            assert!(
                tx.recipient != input.address,
                "Inbound transaction found at block {}",
                block.height
            );
        }
    }

    // Commit public output commitments to the proof output
    sp1_zkvm::io::commit(&input.chain_id);
    sp1_zkvm::io::commit(&input.address);
    sp1_zkvm::io::commit(&input.dormant_since_block);
    sp1_zkvm::io::commit(&input.current_block);
    sp1_zkvm::io::commit(&input.threshold_blocks);
}
