use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChronoBlock {
    pub schema_version: u32,
    pub chain_id: String,
    pub height: u64,
    pub block_hash: Vec<u8>,
    pub prev_hash: Vec<u8>,
    pub timestamp: u64,
    pub block_model: String,
    pub hash_algorithm: String,
    pub transactions: Vec<ChronoTx>,
    pub events: Vec<ChronoEvent>,
    pub extra_data: Vec<u8>,
}

impl ChronoBlock {
    pub fn block_hash_hex(&self) -> String {
        hex::encode(&self.block_hash)
    }

    pub fn prev_hash_hex(&self) -> String {
        hex::encode(&self.prev_hash)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChronoTx {
    pub tx_hash: Vec<u8>,
    pub sender: Vec<u8>,
    pub recipient: Vec<u8>,
    pub amount: u64,
    pub nonce: u64,
    pub payload: Vec<u8>,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub extra_data: Vec<u8>,
}

impl ChronoTx {
    pub fn tx_hash_hex(&self) -> String {
        hex::encode(&self.tx_hash)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChronoEvent {
    pub event_type: String,
    pub emitter: Vec<u8>,
    pub tx_index: u64,
    pub event_index: u64,
    pub payload: Vec<u8>,
}
