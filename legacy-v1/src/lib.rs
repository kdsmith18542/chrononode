//! ChronoNode Archival Client
//! 
//! A high-performance blockchain archival client for the ChronoNode ecosystem.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod api;
pub mod bitcoin_rpc;
pub mod bitcoin_sync;
pub mod config;
pub mod errors;
pub mod ethereum_sync;
pub mod events;
pub mod metrics;
pub mod models;
pub mod solana_sync;
pub mod storage;
pub mod validation;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/chrononode.rs"));
}

// Re-export commonly used types
pub use events::{
    config::{EventConfig, MessageQueueConfig, WebhookConfig},
    message_queue::MessageQueuePublisher,
    processor::{EventProcessor, DefaultEventProcessor},
    publisher::{EventPublisher, EventPublisherConfig},
    Event, EventBus, EventType, EventPayload,
};

// Re-export metrics types
pub use metrics::{
    init_metrics,
    record_event_processing,
    record_event_processing_error,
    start_event_processing_timer,
    update_queue_size,
    gather_metrics,
    server::MetricsServer,
};

use std::sync::Arc;
use anyhow::Result;

/// Initialize the event system
pub async fn init_event_system(config: EventConfig) -> Result<(
    Arc<EventBus>,
    Arc<dyn EventProcessor>,
    Arc<dyn MessageQueuePublisher>,
)> {
    // Create the event bus
    let event_bus = Arc::new(EventBus::new());
    
    // Create the message queue if enabled
    let message_queue: Arc<dyn MessageQueuePublisher> = if config.message_queue.enabled {
        let mq = events::message_queue::AmqpMessageQueue::new(events::message_queue::MessageQueueConfig {
            amqp_url: config.message_queue.amqp_url,
            exchange_name: config.message_queue.exchange_name,
            queue_name: config.message_queue.queue_name,
            routing_key: config.message_queue.routing_key,
        });
        
        // Connect to the message queue
        mq.connect().await?;
        mq.ensure_topology().await?;
        
        Arc::new(mq)
    } else {
        Arc::new(events::message_queue::NoOpMessageQueue)
    };
    
    // Create the event publisher
    let publisher_config = EventPublisherConfig {
        enable_message_queue: config.message_queue.enabled,
        enable_webhooks: config.webhooks.enabled,
        webhook_urls: config.webhooks.urls,
    };
    
    let publisher = Arc::new(EventPublisher::new(
        event_bus.clone(),
        Some(message_queue.clone()),
        publisher_config,
    ));
    
    // Create the event processor
    let processor = Arc::new(DefaultEventProcessor::new(
        publisher,
        "chrononode_archival_client",
    ));
    
    // Start the event bus
    event_bus.start().await;
    
    Ok((event_bus, processor, message_queue))
}

/// Re-export commonly used types
pub use config::{Config, ConfigError};
pub use bitcoin_rpc::{BitcoinRpcClient, BitcoinRpcError};
pub use bitcoin_sync::BitcoinSyncClient;
pub use config::{Config, ConfigError};
pub use errors::BlockchainSyncError;
pub use ethereum_sync::{EthereumSyncClient, EthereumSyncState};
pub use events::{
    config::EventConfig,
    message_queue::{MessageQueueConfig, MessageQueuePublisher},
    processor::{EventProcessor, DefaultEventProcessor},
    publisher::{EventPublisher, EventPublisherConfig},
    Event, EventBus, EventType, EventPayload,
};
pub use models::*;
pub use solana_sync::{SolanaSyncClient, SolanaSyncState};
pub use validation::*;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::bitcoin_rpc::*;
    pub use crate::bitcoin_sync::*;
    pub use crate::config::*;
    pub use crate::errors::*;
    pub use crate::ethereum_sync::*;
    pub use crate::models::*;
    pub use crate::solana_sync::*;
    pub use crate::validation::*;
}
