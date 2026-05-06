//! Configuration for the event system.
//! 
//! This module provides configuration structures and loading functionality
//! for the event publishing system.

use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

/// Event system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventConfig {
    /// Whether to enable the event system
    pub enabled: bool,
    /// Configuration for the message queue
    pub message_queue: MessageQueueConfig,
    /// Configuration for webhook notifications
    pub webhooks: WebhookConfig,
}

/// Message queue configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQueueConfig {
    /// Whether to enable the message queue
    pub enabled: bool,
    /// AMQP connection URL
    pub amqp_url: String,
    /// Exchange name
    pub exchange_name: String,
    /// Queue name
    pub queue_name: String,
    /// Routing key
    pub routing_key: String,
}

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Whether to enable webhook notifications
    pub enabled: bool,
    /// List of webhook URLs to notify
    pub urls: Vec<String>,
    /// Timeout for webhook requests in seconds
    pub timeout_secs: u64,
    /// Maximum number of retries for failed webhooks
    pub max_retries: u32,
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            message_queue: MessageQueueConfig {
                enabled: true,
                amqp_url: "amqp://guest:guest@localhost:5672/%2f".to_string(),
                exchange_name: "chrononode_events".to_string(),
                queue_name: "chrononode_events_queue".to_string(),
                routing_key: "events".to_string(),
            },
            webhooks: WebhookConfig {
                enabled: false,
                urls: Vec::new(),
                timeout_secs: 10,
                max_retries: 3,
            },
        }
    }
}

impl EventConfig {
    /// Load configuration from a file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let content = fs::read_to_string(path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_serialization() {
        let config = EventConfig::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let _deserialized: EventConfig = toml::from_str(&serialized).unwrap();
    }

    #[test]
    fn test_config_file_io() {
        let config = EventConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        
        // Test saving
        config.to_file(path).unwrap();
        
        // Test loading
        let loaded = EventConfig::from_file(path).unwrap();
        
        assert_eq!(config.enabled, loaded.enabled);
        assert_eq!(config.message_queue.enabled, loaded.message_queue.enabled);
        assert_eq!(config.message_queue.amqp_url, loaded.message_queue.amqp_url);
        assert_eq!(config.webhooks.enabled, loaded.webhooks.enabled);
    }
}
