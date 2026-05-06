//! Solana-specific event handlers for the ChronoNode Archival Client.

use crate::{
    events::{BlockEvent, Event, EventBus, EventPayload, StateChangeEvent, TransactionEvent},
    models::BlockHeader,
};
use solana_sdk::{
    clock::UnixTimestamp,
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction as SolanaTransaction,
};
use solana_transaction_status::{
    EncodedConfirmedBlock, EncodedTransactionWithStatusMeta, UiTransactionEncoding,
};
use std::sync::Arc;

/// Solana event handler that processes Solana-specific events and publishes them
/// through the event system.
pub struct SolanaEventHandler {
    event_bus: Arc<EventBus>,
    cluster: SolanaCluster,
}

/// Solana cluster type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolanaCluster {
    MainnetBeta,
    Testnet,
    Devnet,
    Localnet,
    Custom(&'static str),
}

impl SolanaCluster {
    /// Get the cluster name
    pub fn name(&self) -> &str {
        match self {
            SolanaCluster::MainnetBeta => "mainnet-beta",
            SolanaCluster::Testnet => "testnet",
            SolanaCluster::Devnet => "devnet",
            SolanaCluster::Localnet => "localnet",
            SolanaCluster::Custom(name) => name,
        }
    }

    /// Get the chain ID
    pub fn chain_id(&self) -> u64 {
        match self {
            SolanaCluster::MainnetBeta => 101,
            SolanaCluster::Testnet => 102,
            SolanaCluster::Devnet => 103,
            SolanaCluster::Localnet => 104,
            SolanaCluster::Custom(_) => 999,
        }
    }
}

impl SolanaEventHandler {
    /// Create a new Solana event handler
    pub fn new(event_bus: Arc<EventBus>, cluster: SolanaCluster) -> Self {
        Self { event_bus, cluster }
    }

    /// Process a new Solana slot (block)
    pub async fn process_slot(
        &self,
        slot: u64,
        block: &EncodedConfirmedBlock,
        block_time: Option<UnixTimestamp>,
        is_confirmed: bool,
    ) -> anyhow::Result<()> {
        let block_hash = block.blockhash.clone();
        let parent_slot = block.parent_slot;
        let parent_hash = block.previous_blockhash.clone();
        let transaction_count = block.transactions.len() as u64;

        // Create block event
        let block_event = BlockEvent {
            chain_id: self.cluster.chain_id(),
            block_number: slot,
            block_hash: block_hash.clone(),
            parent_hash,
            timestamp: block_time.unwrap_or(0) as u64,
            transaction_count,
            size: 0, // Would need to calculate from block data
            difficulty: 1, // Solana uses PoH, not PoW
            total_difficulty: None,
            chain_tip: is_confirmed,
            ..Default::default()
        };

        // Publish block event
        self.event_bus
            .publish(Event::new(EventPayload::Block(block_event)))
            .await?;

        // Process transactions
        for (tx_index, tx_with_meta) in block.transactions.iter().enumerate() {
            if let Some(meta) = &tx_with_meta.meta {
                self.process_transaction(
                    &tx_with_meta.transaction,
                    tx_index,
                    slot,
                    &block_hash,
                    block_time.unwrap_or(0) as u64,
                    meta,
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Process a Solana transaction
    async fn process_transaction(
        &self,
        tx: &EncodedTransactionWithStatusMeta,
        tx_index: usize,
        slot: u64,
        block_hash: &str,
        block_time: u64,
        meta: &solana_transaction_status::UiTransactionStatusMeta,
    ) -> anyhow::Result<()> {
        let signature = match &tx.signatures.get(0) {
            Some(sig) => sig.to_string(),
            None => return Ok(()), // Skip if no signature
        };

        // Get transaction details
        let message = match &tx.transaction.message {
            solana_transaction_status::UiTransaction::Encoded(encoded) => {
                // Decode the base64-encoded message
                let bytes = base64::decode(encoded)?;
                solana_sdk::message::Message::try_from(bytes.as_slice())?
            }
            _ => return Ok(()), // Skip if we can't decode the message
        };

        // Get fee information
        let fee = meta.fee;

        // Get involved accounts
        let signer = message.account_keys.first().map(|k| k.to_string());
        let program_ids: Vec<String> = message
            .instructions
            .iter()
            .filter_map(|ix| message.account_keys.get(ix.program_id_index as usize))
            .map(|k| k.to_string())
            .collect();

        // Create transaction event
        let tx_event = TransactionEvent {
            chain_id: self.cluster.chain_id(),
            tx_hash: signature,
            block_number: slot,
            block_hash: Some(block_hash.to_string()),
            transaction_index: Some(tx_index as u64),
            from: signer,
            to: None, // Solana transactions can have multiple destinations
            value: "0".to_string(), // Would need to calculate from instructions
            fee: Some(fee.to_string()),
            fee_rate: None,
            input_count: message.account_keys.len() as u32,
            output_count: 0, // Not directly applicable in Solana
            size: 0, // Would need to calculate from tx data
            version: 0,
            lock_time: 0,
            is_coinbase: false, // Not applicable in Solana
            timestamp: block_time,
            ..Default::default()
        };

        // Publish transaction event
        self.event_bus
            .publish(Event::new(EventPayload::Transaction(tx_event)))
            .await?;

        // Process token transfers and other state changes
        self.process_token_transfers(&signature, slot, block_hash, meta, block_time)
            .await?;

        Ok(())
    }

    /// Process token transfers and other state changes from transaction metadata
    async fn process_token_transfers(
        &self,
        signature: &str,
        slot: u64,
        block_hash: &str,
        meta: &solana_transaction_status::UiTransactionStatusMeta,
        block_time: u64,
    ) -> anyhow::Result<()> {
        // Process token balances
        if let Some(pre_token_balances) = &meta.pre_token_balances {
            if let Some(post_token_balances) = &meta.post_token_balances {
                for (pre, post) in pre_token_balances.iter().zip(post_token_balances) {
                    if pre.mint == post.mint
                        && pre.owner == post.owner
                        && pre.token_amount.ui_amount_string != post.token_amount.ui_amount_string
                    {
                        let state_change = StateChangeEvent {
                            chain_id: self.cluster.chain_id(),
                            address: Some(pre.owner.clone()),
                            block_number: slot,
                            block_hash: Some(block_hash.to_string()),
                            transaction_hash: Some(signature.to_string()),
                            log_index: None,
                            event_type: "token_balance_change".to_string(),
                            key: format!("token:{}", pre.mint),
                            old_value: Some(serde_json::json!({
                                "amount": pre.token_amount.ui_amount_string,
                                "decimals": pre.token_amount.decimals,
                            })),
                            new_value: Some(serde_json::json!({
                                "amount": post.token_amount.ui_amount_string,
                                "decimals": post.token_amount.decimals,
                            })),
                            timestamp: block_time,
                            ..Default::default()
                        };

                        self.event_bus
                            .publish(Event::new(EventPayload::StateChange(state_change)))
                            .await?;
                    }
                }
            }
        }

        // Process SOL balance changes
        if let Some(pre_balances) = &meta.pre_balances {
            if let Some(post_balances) = &meta.post_balances {
                for (i, (pre, post)) in pre_balances.iter().zip(post_balances).enumerate() {
                    if pre != post {
                        if let Some(account) = meta.pre_token_balances.as_ref()
                            .and_then(|bals| bals.get(i))
                            .map(|bal| &bal.owner)
                            .or_else(|| meta.post_token_balances.as_ref()
                                .and_then(|bals| bals.get(i))
                                .map(|bal| &bal.owner)
                            )
                        {
                            let state_change = StateChangeEvent {
                                chain_id: self.cluster.chain_id(),
                                address: Some(account.clone()),
                                block_number: slot,
                                block_hash: Some(block_hash.to_string()),
                                transaction_hash: Some(signature.to_string()),
                                log_index: None,
                                event_type: "sol_balance_change".to_string(),
                                key: "sol_balance".to_string(),
                                old_value: Some(serde_json::json!(*pre as f64 / 1_000_000_000.0)), // lamports to SOL
                                new_value: Some(serde_json::json!(*post as f64 / 1_000_000_000.0)),
                                timestamp: block_time,
                                ..Default::default()
                            };

                            self.event_bus
                                .publish(Event::new(EventPayload::StateChange(state_change)))
                                .await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::hash::Hash;
    use solana_transaction_status::{
        EncodedTransaction, UiCompiledInstruction, UiMessage, UiMessageHeader,
        UiTransactionStatusMeta, UiParsedInstruction,
    };

    fn create_test_block(slot: u64) -> (EncodedConfirmedBlock, UnixTimestamp) {
        let block_time = 1234567890;
        
        let block = EncodedConfirmedBlock {
            previous_blockhash: Hash::new_unique().to_string(),
            blockhash: Hash::new_unique().to_string(),
            parent_slot: slot.saturating_sub(1),
            transactions: vec![],
            rewards: vec![],
            block_time: Some(block_time),
            block_height: Some(slot / 432_000), // Approx. # of slots per epoch
        };

        (block, block_time)
    }

    #[tokio::test]
    async fn test_process_slot() {
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
        let (block, block_time) = create_test_block(12345);
        
        // Process slot
        let handler = SolanaEventHandler::new(event_bus, SolanaCluster::Devnet);
        handler.process_slot(block.parent_slot + 1, &block, Some(block_time), true).await.unwrap();

        // Give the event loop some time to process the event
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify events were published
        let events = test_handler.lock().unwrap().events();
        assert!(!events.is_empty());
        
        // Should have at least a block event
        assert!(matches!(
            events[0].payload,
            EventPayload::Block(_)
        ));
    }
}
