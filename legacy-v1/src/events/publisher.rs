//! Event publisher service for broadcasting events to multiple subscribers.
//! 
//! This module provides a publisher service that can broadcast events to
//! multiple subscribers, including message queues, webhooks, and other services.

use crate::events::{Event, EventBus, MessageQueuePublisher};
use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

/// Configuration for the event publisher
#[derive(Debug, Clone)]
pub struct EventPublisherConfig {
    /// Whether to enable the message queue
    pub enable_message_queue: bool,
    /// Whether to enable webhook notifications
    pub enable_webhooks: bool,
    /// Webhook URLs to notify
    pub webhook_urls: Vec<String>,
}

impl Default for EventPublisherConfig {
    fn default() -> Self {
        Self {
            enable_message_queue: true,
            enable_webhooks: false,
            webhook_urls: Vec::new(),
        }
    }
}

/// Trait for event subscribers
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Handle an event
    async fn handle_event(&self, event: &Event) -> Result<()>;
}

/// Publisher service that broadcasts events to multiple subscribers
pub struct EventPublisher {
    /// Inner event bus for local subscribers
    event_bus: Arc<EventBus>,
    /// Message queue publisher (optional)
    message_queue: Option<Arc<dyn MessageQueuePublisher>>,
    /// Webhook URLs for notifications (optional)
    webhook_urls: Vec<String>,
    /// HTTP client for webhook notifications
    http_client: reqwest::Client,
}

impl EventPublisher {
    /// Create a new event publisher
    pub fn new(
        event_bus: Arc<EventBus>,
        message_queue: Option<Arc<dyn MessageQueuePublisher>>,
        config: EventPublisherConfig,
    ) -> Self {
        Self {
            event_bus,
            message_queue: if config.enable_message_queue {
                message_queue
            } else {
                None
            },
            webhook_urls: if config.enable_webhooks {
                config.webhook_urls
            } else {
                Vec::new()
            },
            http_client: reqwest::Client::new(),
        }
    }

    /// Publish an event to all subscribers
    pub async fn publish(&self, event: Event) -> Result<()> {
        // Publish to local event bus
        self.event_bus.publish(event.clone())?;

        // Publish to message queue if enabled
        if let Some(mq) = &self.message_queue {
            if let Err(e) = mq.publish_event(&event).await {
                error!("Failed to publish event to message queue: {}", e);
            }
        }

        // Send webhook notifications if enabled
        if !self.webhook_urls.is_empty() {
            self.notify_webhooks(&event).await?;
        }

        Ok(())
    }

    /// Publish a batch of events
    pub async fn publish_batch(&self, events: Vec<Event>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        // Publish to local event bus
        for event in &events {
            if let Err(e) = self.event_bus.publish(event.clone()) {
                error!("Failed to publish event to local bus: {}", e);
            }
        }

        // Publish to message queue if enabled
        if let Some(mq) = &self.message_queue {
            if let Err(e) = mq.publish_batch(&events).await {
                error!("Failed to publish batch to message queue: {}", e);
            }
        }

        // Send webhook notifications if enabled
        if !self.webhook_urls.is_empty() {
            for event in events {
                if let Err(e) = self.notify_webhooks(&event).await {
                    error!("Failed to send webhook notification: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Notify all webhook URLs about an event
    async fn notify_webhooks(&self, event: &Event) -> Result<()> {
        let event_json = serde_json::to_string(event)?;
        let mut tasks = Vec::new();

        for url in &self.webhook_urls {
            let client = self.http_client.clone();
            let url = url.clone();
            let event_json = event_json.clone();

            tasks.push(tokio::spawn(async move {
                match client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .body(event_json)
                    .send()
                    .await
                {
                    Ok(response) => {
                        if !response.status().is_success() {
                            error!(
                                "Webhook {} returned status code: {}",
                                url,
                                response.status()
                            );
                        }
                    }
                    Err(e) => {
                        error!("Failed to send webhook to {}: {}", url, e);
                    }
                }
            }));
        }

        // Wait for all webhook notifications to complete
        for task in tasks {
            if let Err(e) = task.await {
                error!("Webhook task failed: {}", e);
            }
        }

        Ok(())
    }
}

/// A no-op publisher for testing
#[cfg(test)]
pub struct NoOpPublisher;

#[cfg(test)]
impl NoOpPublisher {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl EventSubscriber for NoOpPublisher {
    async fn handle_event(&self, _event: &Event) -> Result<()> {
        // No-op for testing
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BlockHeader, Event, EventType};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn test_no_op_publisher() {
        let event_bus = Arc::new(EventBus::new());
        let publisher = EventPublisher::new(
            event_bus,
            None,
            EventPublisherConfig {
                enable_message_queue: false,
                enable_webhooks: false,
                webhook_urls: Vec::new(),
            },
        );

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
        publisher.publish(event).await.unwrap();
    }
}
