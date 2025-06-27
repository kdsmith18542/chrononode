//! Event publishing system for ChronoNode.
//! 
//! This module provides an event publishing system that allows different components
//! to publish and subscribe to blockchain events asynchronously.

// Core event types and structures
mod event_structure;
pub use event_structure::{Event, EventType, EventPayload};

// Event bus implementation
mod event_bus;
pub use event_bus::{EventBus, EventHandler, EventHandlerFn};

// Message queue integration
pub mod message_queue;

// Event publisher
pub mod publisher;

// Event processor
pub mod processor;

// Configuration
pub mod config;

// Chain-specific event handlers
#[cfg(feature = "bitcoin")]
pub mod bitcoin_events;

#[cfg(feature = "ethereum")]
pub mod ethereum_events;

#[cfg(feature = "solana")]
pub mod solana_events;

// Test utilities
#[cfg(test)]
mod test_utils;

// Re-export commonly used types
pub use self::{
    config::EventConfig,
    message_queue::{MessageQueuePublisher, AmqpMessageQueue, NoOpMessageQueue},
    processor::{EventProcessor, DefaultEventProcessor},
    publisher::EventPublisher,
};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::fmt;

/// Types of blockchain events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    NewBlock,
    NewTransaction,
    NewStateChange,
    ChainReorg,
    SyncProgress,
    Error,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventType::NewBlock => write!(f, "NewBlock"),
            EventType::NewTransaction => write!(f, "NewTransaction"),
            EventType::NewStateChange => write!(f, "NewStateChange"),
            EventType::ChainReorg => write!(f, "ChainReorg"),
            EventType::SyncProgress => write!(f, "SyncProgress"),
            EventType::Error => write!(f, "Error"),
        }
    }
}

/// Event payload that can contain different types of data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    Block(Box<BlockHeader>),
    Transaction(Box<Transaction>),
    StateChange(Box<StateChange>),
    Reorg { depth: u64, old_hashes: Vec<String>, new_hashes: Vec<String> },
    Progress { current: u64, total: u64, message: String },
    Error(String),
}

/// Event structure that gets published to subscribers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub event_type: EventType,
    pub timestamp: u64,
    pub source: String,
    pub payload: EventPayload,
}

impl Event {
    /// Create a new event
    pub fn new(event_type: EventType, source: &str, payload: EventPayload) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_type,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            source: source.to_string(),
            payload,
        }
    }

    /// Create a new block event
    pub fn new_block(block: BlockHeader, source: &str) -> Self {
        Self::new(
            EventType::NewBlock,
            source,
            EventPayload::Block(Box::new(block)),
        )
    }

    /// Create a new transaction event
    pub fn new_transaction(tx: Transaction, source: &str) -> Self {
        Self::new(
            EventType::NewTransaction,
            source,
            EventPayload::Transaction(Box::new(tx)),
        )
    }

    /// Create a new state change event
    pub fn new_state_change(change: StateChange, source: &str) -> Self {
        Self::new(
            EventType::NewStateChange,
            source,
            EventPayload::StateChange(Box::new(change)),
        )
    }
}

/// Trait for event handlers
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle_event(&self, event: &Event) -> Result<()>;
}

/// Type alias for event handler functions
pub type EventHandlerFn = Box<dyn Fn(&Event) -> Result<()> + Send + Sync>;

/// Event bus that manages event publishing and subscription
pub struct EventBus {
    sender: Sender<Event>,
    receiver: Arc<Mutex<Receiver<Event>>>,
    handlers: Arc<Mutex<HashMap<EventType, Vec<Arc<dyn EventHandler>>>>>, 
}

impl Default for EventBus {
    fn default() -> Self {
        let (sender, receiver) = bounded(1000); // Bounded channel with capacity 1000
        Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
            handlers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        Self::default()
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: Event) -> Result<()> {
        self.sender.send(event).map_err(|e| anyhow::anyhow!("Failed to publish event: {}", e))?;
        Ok(())
    }

    /// Subscribe to all events
    pub async fn subscribe<F>(&self, event_type: EventType, handler: F) -> Result<()>
    where
        F: Fn(&Event) -> Result<()> + Send + Sync + 'static,
    {
        self.subscribe_boxed(event_type, Box::new(handler))
    }

    /// Subscribe with a boxed handler
    pub fn subscribe_boxed(
        &self,
        event_type: EventType,
        handler: EventHandlerFn,
    ) -> Result<()> {
        let handler = Arc::new(handler) as Arc<dyn EventHandler>;
        let mut handlers = self.handlers.blocking_lock();
        handlers.entry(event_type).or_default().push(handler);
        Ok(())
    }

    /// Start the event loop that processes events and calls handlers
    pub async fn start(&self) {
        let receiver = self.receiver.clone();
        let handlers = self.handlers.clone();
        
        tokio::spawn(async move {
            loop {
                let event = match receiver.lock().await.recv() {
                    Ok(event) => event,
                    Err(_) => {
                        log::error!("Event channel closed, stopping event loop");
                        break;
                    }
                };

                // Get handlers for this event type
                let handlers = {
                    let handlers = handlers.lock().await;
                    handlers.get(&event.event_type)
                        .cloned()
                        .unwrap_or_default()
                };

                // Call all handlers for this event
                for handler in handlers {
                    if let Err(e) = handler.handle_event(&event).await {
                        log::error!("Error in event handler: {}", e);
                    }
                }
            }
        });
    }
}

// Implement EventHandler for Fn(&Event) -> Result<()>
#[async_trait]
impl<F> EventHandler for F
where
    F: Fn(&Event) -> Result<()> + Send + Sync,
{
    async fn handle_event(&self, event: &Event) -> Result<()> {
        self(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_event_bus() {
        let event_bus = EventBus::new();
        let counter = Arc::new(AtomicU32::new(0));
        
        // Clone counter for the handler
        let counter_clone = counter.clone();
        
        // Subscribe to NewBlock events
        event_bus
            .subscribe(EventType::NewBlock, move |event| {
                assert_eq!(event.event_type.to_string(), "NewBlock");
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .await
            .unwrap();

        // Start the event loop
        event_bus.start().await;

        // Publish a test event
        let block = BlockHeader {
            height: 1,
            hash: "test_hash".to_string(),
            prev_hash: "prev_hash".to_string(),
            timestamp: 1234567890,
            extra: Default::default(),
        };
        
        let event = Event::new_block(block, "test_source");
        event_bus.publish(event).unwrap();

        // Give the event loop some time to process
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Check that the handler was called
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
