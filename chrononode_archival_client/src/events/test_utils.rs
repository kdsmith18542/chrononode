//! Test utilities for event handling

use crate::events::{Event, EventPayload};
use std::sync::{Arc, Mutex};

/// A test event handler that records all events it receives
pub struct TestEventHandler {
    events: Mutex<Vec<Event>>,
}

impl TestEventHandler {
    /// Create a new test event handler
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    /// Handle an event (for use with event bus subscription)
    pub fn handle_event(&self, event: Event) {
        self.events.lock().unwrap().push(event);
    }

    /// Get all received events
    pub fn events(&self) -> Vec<Event> {
        self.events.lock().unwrap().clone()
    }

    /// Get the count of received events
    pub fn event_count(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    /// Get all events of a specific type
    pub fn filter_events(&self, event_type: EventPayload) -> Vec<Event> {
        self.events()
            .into_iter()
            .filter(|e| e.payload.event_type() == event_type.event_type())
            .collect()
    }

    /// Clear all recorded events
    pub fn clear_events(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl Default for TestEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{BlockEvent, EventType};

    #[test]
    fn test_test_event_handler() {
        let handler = TestEventHandler::new();
        
        // Test initial state
        assert_eq!(handler.event_count(), 0);
        assert!(handler.events().is_empty());
        
        // Test event handling
        let event = Event::new(EventPayload::Block(BlockEvent::default()));
        handler.handle_event(event.clone());
        
        // Verify event was recorded
        assert_eq!(handler.event_count(), 1);
        let events = handler.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::Block);
        
        // Test filtering
        let block_events = handler.filter_events(EventPayload::Block(BlockEvent::default()));
        assert_eq!(block_events.len(), 1);
        
        // Test clearing
        handler.clear_events();
        assert_eq!(handler.event_count(), 0);
    }
}
