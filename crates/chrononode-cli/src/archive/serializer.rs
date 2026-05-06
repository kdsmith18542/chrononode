use chrononode_core::{ChronoBlock, Result};

pub fn serialize_block(block: &ChronoBlock) -> Result<Vec<u8>> {
    let proto = block_to_proto(block);
    use prost::Message;
    let mut buf = Vec::with_capacity(proto.encoded_len());
    proto.encode(&mut buf)?;
    Ok(buf)
}

pub fn deserialize_block(bytes: &[u8]) -> Result<ChronoBlock> {
    use prost::Message;
    let proto = crate::proto::ChronoBlock::decode(bytes)?;
    proto_to_block(&proto)
}

fn block_to_proto(block: &ChronoBlock) -> crate::proto::ChronoBlock {
    crate::proto::ChronoBlock {
        schema_version: block.schema_version,
        chain_id: block.chain_id.clone(),
        height: block.height,
        block_hash: block.block_hash.clone(),
        prev_hash: block.prev_hash.clone(),
        timestamp: block.timestamp,
        block_model: block.block_model.clone(),
        hash_algorithm: block.hash_algorithm.clone(),
        transactions: block.transactions.iter().map(|tx| crate::proto::ChronoTx {
            tx_hash: tx.tx_hash.clone(),
            sender: tx.sender.clone(),
            recipient: tx.recipient.clone(),
            amount: tx.amount,
            nonce: tx.nonce,
            payload: tx.payload.clone(),
            gas_limit: tx.gas_limit,
            gas_used: tx.gas_used,
            extra_data: tx.extra_data.clone(),
        }).collect(),
        events: block.events.iter().map(|ev| crate::proto::ChronoEvent {
            event_type: ev.event_type.clone(),
            emitter: ev.emitter.clone(),
            tx_index: ev.tx_index,
            event_index: ev.event_index,
            payload: ev.payload.clone(),
        }).collect(),
        extra_data: block.extra_data.clone(),
    }
}

fn proto_to_block(proto: &crate::proto::ChronoBlock) -> Result<ChronoBlock> {
    Ok(ChronoBlock {
        schema_version: proto.schema_version,
        chain_id: proto.chain_id.clone(),
        height: proto.height,
        block_hash: proto.block_hash.clone(),
        prev_hash: proto.prev_hash.clone(),
        timestamp: proto.timestamp,
        block_model: proto.block_model.clone(),
        hash_algorithm: proto.hash_algorithm.clone(),
        transactions: proto.transactions.iter().map(|tx| chrononode_core::ChronoTx {
            tx_hash: tx.tx_hash.clone(),
            sender: tx.sender.clone(),
            recipient: tx.recipient.clone(),
            amount: tx.amount,
            nonce: tx.nonce,
            payload: tx.payload.clone(),
            gas_limit: tx.gas_limit,
            gas_used: tx.gas_used,
            extra_data: tx.extra_data.clone(),
        }).collect(),
        events: proto.events.iter().map(|ev| chrononode_core::ChronoEvent {
            event_type: ev.event_type.clone(),
            emitter: ev.emitter.clone(),
            tx_index: ev.tx_index,
            event_index: ev.event_index,
            payload: ev.payload.clone(),
        }).collect(),
        extra_data: proto.extra_data.clone(),
    })
}
