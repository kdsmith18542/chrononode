# Event System

The event system provides a flexible and extensible way to handle and distribute blockchain events
across the ChronoNode Archival Client. It supports multiple event types, message queuing, and webhook
notifications.

## Architecture

```
+----------------+     +----------------+     +------------------+
| Event Producers| --> | Event Bus     | --> | Event Consumers  |
| (Blockchain    |     | (Local pub/sub)|     | (Message Queues, |
|  sync modules) |     +----------------+     |  Webhooks, etc.) |
+----------------+     | Event Processor|     +------------------+
                       +----------------+
```

## Components

### 1. Event Bus
- Local in-memory event bus for pub/sub
- Handles event routing to subscribers
- Thread-safe and async-compatible

### 2. Message Queue
- AMQP-based message queue for reliable event delivery
- Supports multiple consumers and load balancing
- Configurable exchange and queue topologies

### 3. Webhook Notifications
- HTTP callbacks for external integrations
- Configurable retries and timeouts
- Batch processing support

### 4. Event Types
- `NewBlock`: New block detected
- `NewTransaction`: New transaction detected
- `NewStateChange`: State change detected
- `ChainReorg`: Chain reorganization detected
- `SyncProgress`: Synchronization progress update
- `Error`: Error event

## Usage

### Initialization

```rust
use chrononode_archival_client::{
    init_event_system,
    events::config::EventConfig,
    EventBus, EventProcessor, MessageQueuePublisher,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize with default config
    let config = EventConfig::default();
    
    // Initialize the event system
    let (event_bus, event_processor, message_queue) = init_event_system(config).await?;
    
    // Use the components...
    
    Ok(())
}
```

### Publishing Events

```rust
// In your blockchain sync module
async fn on_new_block(&self, block: BlockHeader) -> Result<()> {
    // Process the block...
    
    // Publish the event
    self.event_processor.process_block(&block).await?;
    
    Ok(())
}
```

### Subscribing to Events

```rust
// Subscribe to block events
event_bus.subscribe(EventType::NewBlock, |event| {
    if let EventPayload::Block(block) = &event.payload {
        println!("New block: {} (height: {})", block.hash, block.height);
    }
    Ok(())
}).await?;

// Start the event bus
event_bus.start().await;
```

### Webhook Configuration

```toml
[webhooks]
enabled = true
timeout_secs = 10
max_retries = 3
urls = [
    "https://example.com/api/events",
    "https://another-service.com/webhook"
]
```

## Configuration

### Environment Variables

- `EVENT_QUEUE_ENABLED`: Enable/disable message queue (true/false)
- `AMQP_URL`: AMQP connection URL (default: "amqp://guest:guest@localhost:5672/%2f")
- `WEBHOOK_ENABLED`: Enable/disable webhooks (true/false)
- `WEBHOOK_URLS`: Comma-separated list of webhook URLs

### Configuration File

```toml
[message_queue]
enabled = true
amqp_url = "amqp://guest:guest@localhost:5672/%2f"
exchange_name = "chrononode_events"
queue_name = "chrononode_events_queue"
routing_key = "events"

[webhooks]
enabled = false
timeout_secs = 10
max_retries = 3
urls = []
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::publisher::NoOpPublisher;
    
    #[tokio::test]
    async fn test_event_processing() {
        let publisher = Arc::new(NoOpPublisher::new());
        let processor = DefaultEventProcessor::new(publisher, "test");
        
        // Test block processing
        let block = BlockHeader { /* ... */ };
        processor.process_block(&block).await.unwrap();
    }
}
```

## Error Handling

All event-related functions return `Result<(), anyhow::Error>` to handle potential failures.
Common error cases include:

- Connection failures to message brokers
- Invalid event data
- Webhook delivery failures
- Serialization/deserialization errors

## Performance Considerations

- Use batch processing for high-throughput scenarios
- Consider message queue persistence for critical events
- Monitor queue lengths and consumer lag
- Use appropriate prefetch counts for message consumers
