//! Bitcoin-specific event handlers for the ChronoNode Archival Client.

use crate::{
    events::{BlockEvent, Event, EventBus, EventPayload, StateChangeEvent, TransactionEvent},
    models::BlockHeader,
};
use bitcoin::{
    blockdata::block::Block, hashes::Hash as BitcoinHash, BlockHash, Transaction, TxIn, TxOut,
};
use std::sync::Arc;

/// Bitcoin event handler that processes Bitcoin-specific events and publishes them
/// through the event system.
pub struct BitcoinEventHandler {
    event_bus: Arc<EventBus>,
    network: BitcoinNetwork,
}

/// Bitcoin network type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitcoinNetwork {
    Mainnet,
    Testnet,
    Regtest,
    Signet,
}

impl BitcoinNetwork {
    /// Get the network magic bytes
    pub fn magic_bytes(&self) -> u32 {
        match self {
            BitcoinNetwork::Mainnet => 0xD9B4BEF9,
            BitcoinNetwork::Testnet => 0x0709110B,
            BitcoinNetwork::Regtest => 0xDAB5BFFA,
            BitcoinNetwork::Signet => 0x40CF030A,
        }
    }
}

impl BitcoinEventHandler {
    /// Create a new Bitcoin event handler
    pub fn new(event_bus: Arc<EventBus>, network: BitcoinNetwork) -> Self {
        Self { event_bus, network }
    }

    /// Process a new Bitcoin block
    pub async fn process_block(
        &self,
        block: &Block,
        height: u64,
        chain_tip: bool,
    ) -> anyhow::Result<()> {
        let block_hash = block.block_hash();
        
        // Create block event
        let block_event = BlockEvent {
            chain_id: self.network.magic_bytes() as u64,
            block_number: height,
            block_hash: block_hash.to_string(),
            parent_hash: block.header.prev_blockhash.to_string(),
            timestamp: block.header.time as u64,
            transaction_count: block.txdata.len() as u64,
            size: block.get_size() as usize,
            difficulty: block.header.difficulty() as u64,
            total_difficulty: None, // Not directly available in Bitcoin
            chain_tip,
            ..Default::default()
        };

        // Publish block event
        self.event_bus
            .publish(Event::new(EventPayload::Block(block_event)))
            .await?;

        // Process coinbase transaction separately
        if let Some((tx_index, tx)) = block.txdata.first().map(|tx| (0, tx)) {
            self.process_transaction(tx, tx_index, height, block_hash, block.header.time as u64, true)
                .await?;
        }

        // Process regular transactions
        for (tx_index, tx) in block.txdata.iter().skip(1).enumerate() {
            self.process_transaction(tx, tx_index + 1, height, block_hash, block.header.time as u64, false)
                .await?;
        }

        Ok(())
    }

    /// Process a Bitcoin transaction
    async fn process_transaction(
        &self,
        tx: &Transaction,
        tx_index: usize,
        block_height: u64,
        block_hash: BlockHash,
        block_time: u64,
        is_coinbase: bool,
    ) -> anyhow::Result<()> {
        let tx_hash = tx.txid();
        
        // Calculate total input value (excluding coinbase)
        let input_value = if !is_coinbase {
            tx.input
                .iter()
                .map(|input: &TxIn| input.witness.total_size() as u64) // Simplified for example
                .sum()
        } else {
            0
        };

        // Calculate total output value
        let output_value = tx.output.iter().map(|output: &TxOut| output.value).sum();

        // Calculate fees (for non-coinbase transactions)
        let fee = if !is_coinbase {
            input_value.checked_sub(output_value)
        } else {
            Some(0)
        };

        let tx_event = TransactionEvent {
            chain_id: self.network.magic_bytes() as u64,
            tx_hash: tx_hash.to_string(),
            block_number: block_height,
            block_hash: Some(block_hash.to_string()),
            transaction_index: Some(tx_index as u64),
            from: None, // Bitcoin doesn't have explicit senders
            to: None,   // Bitcoin can have multiple outputs
            value: output_value.to_string(),
            fee: fee.map(|f| f.to_string()),
            fee_rate: None, // Would need to calculate based on tx size
            input_count: tx.input.len() as u32,
            output_count: tx.output.len() as u32,
            size: tx.get_size() as u64,
            version: tx.version as i32,
            lock_time: tx.lock_time.to_consensus_u32(),
            is_coinbase,
            timestamp: block_time,
            ..Default::default()
        };

        // Publish transaction event
        self.event_bus
            .publish(Event::new(EventPayload::Transaction(tx_event)))
            .await?;

        // Process UTXO state changes
        self.process_utxo_changes(tx, block_height, block_hash, block_time, is_coinbase)
            .await?;

        Ok(())
    }

    /// Process UTXO state changes from a transaction
    async fn process_utxo_changes(
        &self,
        tx: &Transaction,
        block_height: u64,
        block_hash: BlockHash,
        block_time: u64,
        is_coinbase: bool,
    ) -> anyhow::Result<()> {
        // Process inputs (spent UTXOs)
        if !is_coinbase {
            for input in &tx.input {
                let state_change = StateChangeEvent {
                    chain_id: self.network.magic_bytes() as u64,
                    address: None, // Would need to extract from script
                    block_number: block_height,
                    block_hash: Some(block_hash.to_string()),
                    transaction_hash: Some(tx.txid().to_string()),
                    log_index: None,
                    event_type: "utxo_spent".to_string(),
                    key: format!("{}:{}", input.previous_output.txid, input.previous_output.vout),
                    old_value: None, // We don't have the previous value here
                    new_value: None,
                    timestamp: block_time,
                    ..Default::default()
                };

                self.event_bus
                    .publish(Event::new(EventPayload::StateChange(state_change)))
                    .await?;
            }
        }

        // Process outputs (new UTXOs)
        for (vout, output) in tx.output.iter().enumerate() {
            let state_change = StateChangeEvent {
                chain_id: self.network.magic_bytes() as u64,
                address: None, // Would need to extract from script
                block_number: block_height,
                block_hash: Some(block_hash.to_string()),
                transaction_hash: Some(tx.txid().to_string()),
                log_index: Some(vout as u64),
                event_type: "utxo_created".to_string(),
                key: format!("{}:{}", tx.txid(), vout),
                old_value: None,
                new_value: Some(serde_json::json!({
                    "value": output.value,
                    "script_pubkey": hex::encode(&output.script_pubkey[..])
                })),
                timestamp: block_time,
                ..Default::default()
            };

            self.event_bus
                .publish(Event::new(EventPayload::StateChange(state_change)))
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{
        blockdata::block::BlockHeader as BitcoinBlockHeader, hashes::Hash, Block, BlockHash,
        Transaction, TxIn, TxOut,
    };
    use std::str::FromStr;

    fn create_test_block(height: u64) -> (Block, u64) {
        let header = BitcoinBlockHeader {
            version: 0x20000000,
            prev_blockhash: BlockHash::hash(b"prev_block"),
            merkle_root: BlockHash::hash(b"merkle_root"),
            time: 1234567890,
            bits: 0x1d00ffff,
            nonce: 12345,
        };

        let coinbase_tx = Transaction {
            version: 1,
            lock_time: 0,
            input: vec![TxIn::coinbase(Default::default())],
            output: vec![TxOut {
                value: 50 * 100_000_000, // 50 BTC in satoshis
                script_pubkey: Default::default(),
            }],
        };

        let block = Block {
            header,
            txdata: vec![coinbase_tx],
        };

        (block, height)
    }

    #[tokio::test]
    async fn test_process_block() {
        // Setup test
        let event_bus = Arc::new(EventBus::new());
        let test_handler = Arc::new(TestEventHandler::new());
        
        // Subscribe to block events
        let handler_clone = test_handler.clone();
        event_bus
            .subscribe(EventType::Block, move |event| {
                let mut handler = handler_clone.lock().unwrap();
                handler.handle_event(event);
                Ok(())
            })
            .await
            .unwrap();

        // Start the event bus
        event_bus.start().await;

        // Create test block
        let (block, height) = create_test_block(12345);
        
        // Process block
        let handler = BitcoinEventHandler::new(event_bus, BitcoinNetwork::Regtest);
        handler.process_block(&block, height, true).await.unwrap();

        // Give the event loop some time to process the event
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify events were published
        let events = test_handler.lock().unwrap().events();
        assert!(!events.is_empty());
        
        // Should have at least a block event and a transaction event
        assert!(events.len() >= 2);
        assert!(matches!(
            events[0].payload,
            EventPayload::Block(_)
        ));
    }
}
