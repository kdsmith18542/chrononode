//! Message queue integration for event publishing.
//! 
//! This module provides a message queue implementation that can be used to publish
//! events to external systems using AMQP.

use crate::events::Event;
use anyhow::{Context, Result};
use async_trait::async_trait;
use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

/// Configuration for the message queue
#[derive(Debug, Clone)]
pub struct MessageQueueConfig {
    pub amqp_url: String,
    pub exchange_name: String,
    pub queue_name: String,
    pub routing_key: String,
}

impl Default for MessageQueueConfig {
    fn default() -> Self {
        Self {
            amqp_url: "amqp://guest:guest@localhost:5672/%2f".to_string(),
            exchange_name: "chrononode_events".to_string(),
            queue_name: "chrononode_events_queue".to_string(),
            routing_key: "events".to_string(),
        }
    }
}

/// Trait for message queue publishers
#[async_trait]
pub trait MessageQueuePublisher: Send + Sync {
    /// Publish an event to the message queue
    async fn publish_event(&self, event: &Event) -> Result<()>;
    
    /// Publish a batch of events
    async fn publish_batch(&self, events: &[Event]) -> Result<()>;
}

/// AMQP-based message queue implementation
pub struct AmqpMessageQueue {
    config: MessageQueueConfig,
    connection: Arc<Mutex<Option<Connection>>>,
}

impl AmqpMessageQueue {
    /// Create a new AMQP message queue with the given configuration
    pub fn new(config: MessageQueueConfig) -> Self {
        Self {
            config,
            connection: Arc::new(Mutex::new(None)),
        }
    }

    /// Connect to the message queue
    pub async fn connect(&self) -> Result<()> {
        let conn = Connection::connect(
            &self.config.amqp_url,
            ConnectionProperties::default(),
        )
        .await
        .context("Failed to connect to AMQP server")?;

        info!("Connected to AMQP server at {}", self.config.amqp_url);

        // Store the connection
        let mut conn_guard = self.connection.lock().await;
        *conn_guard = Some(conn);

        Ok(())
    }

    /// Ensure the exchange and queue are declared
    pub async fn ensure_topology(&self) -> Result<()> {
        let conn_guard = self.connection.lock().await;
        let conn = conn_guard.as_ref().context("Not connected to AMQP server")?;

        let channel = conn.create_channel().await?;

        // Declare the exchange
        channel
            .exchange_declare(
                &self.config.exchange_name,
                lapin::ExchangeKind::Topic,
                ExchangeDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .context("Failed to declare exchange")?;

        // Declare the queue
        let _queue = channel
            .queue_declare(
                &self.config.queue_name,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .context("Failed to declare queue")?;

        // Bind the queue to the exchange
        channel
            .queue_bind(
                &self.config.queue_name,
                &self.config.exchange_name,
                &self.config.routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .context("Failed to bind queue to exchange")?;

        info!(
            "Declared exchange '{}' and queue '{}' with routing key '{}'",
            self.config.exchange_name, self.config.queue_name, self.config.routing_key
        );

        Ok(())
    }

    /// Serialize an event to JSON
    fn serialize_event(event: &Event) -> Result<Vec<u8>> {
        serde_json::to_vec(event).context("Failed to serialize event")
    }
}

#[async_trait]
impl MessageQueuePublisher for AmqpMessageQueue {
    async fn publish_event(&self, event: &Event) -> Result<()> {
        let payload = Self::serialize_event(event)?;
        let conn_guard = self.connection.lock().await;
        let conn = conn_guard.as_ref().context("Not connected to AMQP server")?;

        let channel = conn.create_channel().await?;

        channel
            .basic_publish(
                &self.config.exchange_name,
                &self.config.routing_key,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default(),
            )
            .await?
            .await?; // Wait for the confirmation

        debug!("Published event {} to message queue", event.id);
        Ok(())
    }

    async fn publish_batch(&self, events: &[Event]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let conn_guard = self.connection.lock().await;
        let conn = conn_guard.as_ref().context("Not connected to AMQP server")?;
        let channel = conn.create_channel().await?;

        // Start a transaction
        channel.tx_select(FieldTable::default()).await?;

        for event in events {
            let payload = Self::serialize_event(event)?;
            
            channel
                .basic_publish(
                    &self.config.exchange_name,
                    &self.config.routing_key,
                    BasicPublishOptions::default(),
                    &payload,
                    BasicProperties::default(),
                )
                .await?;
        }

        // Commit the transaction
        channel.tx_commit().await?;
        
        debug!("Published batch of {} events to message queue", events.len());
        Ok(())
    }
}

/// A no-op message queue implementation for testing
#[cfg(test)]
pub struct NoOpMessageQueue;

#[cfg(test)]
#[async_trait]
impl MessageQueuePublisher for NoOpMessageQueue {
    async fn publish_event(&self, _event: &Event) -> Result<()> {
        // No-op for testing
        Ok(())
    }

    async fn publish_batch(&self, _events: &[Event]) -> Result<()> {
        // No-op for testing
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BlockHeader, Event};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn test_no_op_message_queue() {
        let queue = NoOpMessageQueue;
        let event = Event::new(
            EventType::NewBlock,
            "test",
            EventPayload::Block(Box::new(BlockHeader {
                height: 1,
                hash: "test".to_string(),
                prev_hash: "prev".to_string(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                extra: Default::default(),
            })),
        );

        // Should not panic
        queue.publish_event(&event).await.unwrap();
        queue.publish_batch(&[event]).await.unwrap();
    }
}
