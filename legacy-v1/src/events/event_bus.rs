//! Event bus implementation for asynchronous event publishing and subscription.

use crate::events::{Event, EventType};
use anyhow::Result;
use async_trait::async_trait;
use crossbeam_channel::{bounded, Sender, Receiver};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use crate::metrics;

/// Trait for event handlers
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an event
    async fn handle_event(&self, event: &Event) -> Result<()>;
}

/// Type alias for event handler functions
pub type EventHandlerFn = Box<dyn Fn(&Event) -> Result<()> + Send + Sync>;

/// Wrapper for async event handlers
struct AsyncHandler<F: Fn(Event) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> + Send + Sync> {
    handler: F,
}

#[async_trait]
impl<F> EventHandler for AsyncHandler<F>
where
    F: Fn(Event) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> + Send + Sync,
{
    async fn handle_event(&self, event: &Event) -> Result<()> {
        (self.handler)(event.clone()).await
    }
}

/// Event bus that manages event publishing and subscription
pub struct EventBus {
    /// Channel for sending events to the event loop
    event_tx: Sender<Event>,
    /// Channel for receiving events in the event loop
    event_rx: Arc<Mutex<Receiver<Event>>>,
    /// Registered event handlers by event type
    handlers: Arc<Mutex<HashMap<EventType, Vec<Arc<dyn EventHandler>>>>>>,
}

impl Default for EventBus {
    fn default() -> Self {
        let (tx, rx) = bounded(1000);
        Self {
            event_tx: tx,
            event_rx: Arc::new(Mutex::new(rx)),
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
        self.event_tx.send(event)?;
        Ok(())
    }

    /// Subscribe to events of a specific type with a handler function
    pub fn subscribe<F>(&self, event_type: EventType, handler: F) -> Result<()>
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
        let handler = Arc::new(handler);
        let mut handlers = self.handlers.lock().unwrap();
        let handlers_for_type = handlers.entry(event_type).or_insert_with(Vec::new);
        handlers_for_type.push(handler);
        Ok(())
    }

    /// Subscribe with an async handler
    pub fn subscribe_async<F, Fut>(
        &self,
        event_type: EventType,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let handler = Arc::new(AsyncHandler { handler });
        let mut handlers = self.handlers.lock().unwrap();
        let handlers_for_type = handlers.entry(event_type).or_insert_with(Vec::new);
        handlers_for_type.push(handler);
        Ok(())
    }

    /// Start the event loop that processes events and calls handlers
    pub async fn start(&self) {
        let rx = self.event_rx.clone();
        let handlers = self.handlers.clone();

        tokio::spawn(async move {
            while let Ok(event) = rx.recv() {
                let event_type = event.event_type.to_string();
                let start_time = Instant::now();

                // Update queue size metric before processing
                metrics::update_queue_size("event_bus", rx.len());

                let handlers = handlers.lock().await;
                if let Some(handlers) = handlers.get(&event.event_type) {
                    let handler_count = handlers.len();

                    // Process each handler
                    for (i, handler) in handlers.iter().enumerate() {
                        let handler_start = Instant::now();
                        let result = handler.handle_event(&event).await;

                        // Record handler processing time
                        let handler_duration = handler_start.elapsed();
                        metrics::record_event_processing(
                            &format!("handler_{}_{}", event_type, i),
                            if result.is_ok() { "success" } else { "error" },
                            handler_start
                        );

                        if let Err(e) = result {
                            log::error!("Error handling event: {}", e);
                            metrics::record_event_processing_error("handler_error");
                        }
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
        (self)(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn test_event_bus() {
        let event_bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        
        // Subscribe to all events
        let counter_clone = counter.clone();
        event_bus
            .subscribe(EventType::NewBlock, move |_| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .unwrap();
        
        // Start the event bus
        event_bus.start().await;
        
        // Publish an event
        let event = Event::new_block(
            crate::models::BlockHeader::default(),
            "test",
        );
        event_bus.publish(event).unwrap();
        
        // Give the event loop some time to process the event
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Check that the handler was called
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
    
    #[tokio::test]
    async fn test_async_handler() {
        let event_bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        
        // Subscribe with an async handler
        let counter_clone = counter.clone();
        event_bus
            .subscribe_async(EventType::NewBlock, move |_| {
                let counter = counter_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, anyhow::Error>(())
                }
            })
            .unwrap();
        
        // Start the event bus
        event_bus.start().await;
        
        // Publish an event
        let event = Event::new_block(
            crate::models::BlockHeader::default(),
            "test",
        );
        event_bus.publish(event).unwrap();
        
        // Give the event loop some time to process the event
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Check that the handler was called
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
