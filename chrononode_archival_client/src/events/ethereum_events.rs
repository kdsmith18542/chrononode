//! Ethereum-specific event handlers for the ChronoNode Archival Client.

use crate::{
    events::{
        BlockEvent, Event, EventBus, EventPayload, EventProcessor, StateChangeEvent, TransactionEvent,
    },
    models::BlockHeader,
};
use ethers::{
    core::types::{Block, Transaction, TransactionReceipt, H256},
    utils::to_checksum,
};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Ethereum event handler that processes Ethereum-specific events and publishes them
/// through the event system.
pub struct EthereumEventHandler {
    event_bus: Arc<EventBus>,
    chain_id: u64,
}

impl EthereumEventHandler {
    /// Create a new Ethereum event handler
    pub fn new(event_bus: Arc<EventBus>, chain_id: u64) -> Self {
        Self { event_bus, chain_id }
    }

    /// Process a new Ethereum block
    pub async fn process_block(
        &self,
        block: Block<H256>,
        receipts: &[TransactionReceipt],
    ) -> anyhow::Result<()> {
        let block_number = block
            .number
            .ok_or_else(|| anyhow::anyhow!("Block number is None"))?
            .as_u64();

        // Create block event
        let block_event = BlockEvent {
            chain_id: self.chain_id,
            block_number,
            block_hash: format!("0x{:x}", block.hash.ok_or_else(|| anyhow::anyhow!("Block hash is None"))?),
            parent_hash: format!("0x{:x}", block.parent_hash),
            timestamp: block.timestamp.as_u64(),
            transaction_count: block.transactions.len() as u64,
            size: block.size.unwrap_or_default().as_u64() as usize,
            difficulty: block.difficulty.as_u64(),
            total_difficulty: block.total_difficulty.map(|d| d.as_u128()),
            ..Default::default()
        };

        // Publish block event
        self.event_bus
            .publish(Event::new(EventPayload::Block(block_event)))
            .await?;

        // Process transactions
        if let Some(transactions) = block.transactions {
            for (tx_index, (tx, receipt)) in transactions.into_iter().zip(receipts).enumerate() {
                self.process_transaction(&tx, tx_index, block_number, block.timestamp.as_u64(), receipt)
                    .await?;
            }
        }

        Ok(())
    }

    /// Process an Ethereum transaction
    async fn process_transaction(
        &self,
        tx: &Transaction,
        tx_index: usize,
        block_number: u64,
        block_timestamp: u64,
        receipt: &TransactionReceipt,
    ) -> anyhow::Result<()> {
        let tx_event = TransactionEvent {
            chain_id: self.chain_id,
            tx_hash: format!("0x{:x}", tx.hash),
            block_number,
            block_hash: tx.block_hash.map(|h| format!("0x{:x}", h)),
            transaction_index: Some(tx_index as u64),
            from: tx.from.map(|a| to_checksum(&a, None)),
            to: tx.to.map(|a| to_checksum(&a, None)),
            value: tx.value.to_string(),
            nonce: tx.nonce.as_u64(),
            gas_price: tx.gas_price.map(|p| p.to_string()),
            gas_limit: tx.gas.as_u64(),
            gas_used: receipt.gas_used.map(|g| g.as_u64()),
            input: tx.input.to_string(),
            status: receipt.status.map(|s| s.as_u64() == 1),
            timestamp: block_timestamp,
            ..Default::default()
        };

        // Publish transaction event
        self.event_bus
            .publish(Event::new(EventPayload::Transaction(tx_event)))
            .await?;

        // Process state changes from logs
        if let Some(logs) = &receipt.logs {
            for log in logs {
                self.process_log(log, block_number, block_timestamp, &tx.hash)
                    .await?;
            }
        }

        Ok(())
    }

    /// Process an Ethereum log entry as a state change
    async fn process_log(
        &self,
        log: &ethers::types::Log,
        block_number: u64,
        block_timestamp: u64,
        tx_hash: &H256,
    ) -> anyhow::Result<()> {
        let state_change = StateChangeEvent {
            chain_id: self.chain_id,
            address: to_checksum(&log.address, None),
            block_number,
            block_hash: log.block_hash.map(|h| format!("0x{:x}", h)),
            transaction_hash: Some(format!("0x{:x}", tx_hash)),
            log_index: log.log_index.map(|i| i.as_u64()),
            event_signature: log.topics.get(0).map(|t| format!("0x{:x}", t)),
            data: log.data.to_string(),
            timestamp: block_timestamp,
            ..Default::default()
        };

        // Publish state change event
        self.event_bus
            .publish(Event::new(EventPayload::StateChange(state_change)))
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::test_utils::TestEventHandler;
    use ethers::{
        core::types::{
            Address, Block as EthBlock, BlockNumber as EthBlockNumber, Transaction as EthTransaction,
            TransactionReceipt as EthReceipt, H256, U256, U64,
        },
        types::Log,
    };
    use std::str::FromStr;

    #[tokio::test]
    async fn test_process_block() {
        // Setup test
        let event_bus = Arc::new(EventBus::new());
        let test_handler = Arc::new(Mutex::new(TestEventHandler::new()));
        
        // Subscribe to events
        let handler_clone = test_handler.clone();
        event_bus
            .subscribe(crate::events::EventType::Block, move |event| {
                let mut handler = handler_clone.lock().unwrap();
                handler.handle_event(event);
                Ok(())
            })
            .await
            .unwrap();

        // Start the event bus
        event_bus.start().await;

        // Create test data
        let block = EthBlock {
            number: Some(U64::from(12345)),
            hash: Some(H256::from_low_u64_be(1)),
            parent_hash: H256::from_low_u64_be(2),
            timestamp: U256::from(1625097600),
            transactions: vec![EthTransaction::default()],
            ..Default::default()
        };

        let receipts = vec![EthReceipt::default()];

        // Process block
        let handler = EthereumEventHandler::new(event_bus, 1);
        handler.process_block(block, &receipts).await.unwrap();

        // Verify events were published
        let events = test_handler.lock().unwrap().events();
        assert_eq!(events.len(), 2); // Block + Transaction events
        assert!(matches!(
            events[0].payload,
            EventPayload::Block(_)
        ));
    }
}
