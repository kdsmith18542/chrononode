//! Event processor for handling blockchain events.
//! 
//! This module provides functionality to process blockchain events and
//! publish them to various subscribers.

use crate::{
    events::{
        publisher::{EventPublisher, EventPublisherConfig},
        Event, EventBus, MessageQueuePublisher,
    },
    models::{BlockHeader, StateChange, Transaction},
};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

/// Trait for event processors
#[async_trait]
pub trait EventProcessor: Send + Sync {
    /// Process a new block header
    async fn process_block(&self, block: &BlockHeader) -> Result<()>;
    
    /// Process a new transaction
    async fn process_transaction(&self, tx: &Transaction) -> Result<()>;
    
    /// Process a state change
    async fn process_state_change(&self, change: &StateChange) -> Result<()>;
}

/// Default implementation of the event processor
pub struct DefaultEventProcessor {
    /// Event publisher
    publisher: Arc<dyn EventPublisher>,
    /// Source identifier for events
    source: String,
}

impl DefaultEventProcessor {
    /// Create a new event processor
    pub fn new(publisher: Arc<dyn EventPublisher>, source: &str) -> Self {
        Self {
            publisher,
            source: source.to_string(),
        }
    }
}

#[async_trait]
impl EventProcessor for DefaultEventProcessor {
    async fn process_block(&self, block: &BlockHeader) -> Result<()> {
        debug!("Processing block {} (height: {})", block.hash, block.height);
        
        // Create and publish the event
        let event = Event::new_block(block.clone(), &self.source);
        self.publisher.publish(event).await?;
        
        Ok(())
    }

    async fn process_transaction(&self, tx: &Transaction) -> Result<()> {
        debug!("Processing transaction {}", tx.tx_id);
        
        // Create and publish the event
        let event = Event::new_transaction(tx.clone(), &self.source);
        self.publisher.publish(event).await?;
        
        Ok(())
    }

    async fn process_state_change(&self, change: &StateChange) -> Result<()> {
        debug!(
            "Processing state change for {} at block {}",
            change.address, change.block_number
        );
        
        // Create and publish the event
        let event = Event::new_state_change(change.clone(), &self.source);
        self.publisher.publish(event).await?;
        
        Ok(())
    }
}

/// A no-op event processor for testing
#[cfg(test)]
pub struct NoOpEventProcessor;

#[cfg(test)]
#[async_trait]
impl EventProcessor for NoOpEventProcessor {
    async fn process_block(&self, _block: &BlockHeader) -> Result<()> {
        Ok(())
    }

    async fn process_transaction(&self, _tx: &Transaction) -> Result<()> {
        Ok(())
    }

    async fn process_state_change(&self, _change: &StateChange) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::publisher::NoOpPublisher;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn test_no_op_event_processor() {
        let processor = NoOpEventProcessor;
        let block = BlockHeader {
            height: 1,
            hash: "test".to_string(),
            prev_hash: "prev".to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            extra: Default::default(),
        };

        // Should not panic
        processor.process_block(&block).await.unwrap();
    }
}
