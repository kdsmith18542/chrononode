//! Configuration management for ChronoNode Archival Client.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during configuration loading and validation
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    
    #[error("Invalid configuration: {0}")]
    Validation(String),
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Bitcoin-specific configuration
    pub bitcoin: BitcoinConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
    
    /// Metrics configuration
    #[serde(default = "MetricsConfig::default")]
    pub metrics: MetricsConfig,
}

/// Bitcoin-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinConfig {
    /// URL of the Bitcoin Core RPC server
    pub rpc_url: String,
    
    /// RPC username for authentication
    pub rpc_username: Option<String>,
    
    /// RPC password for authentication
    pub rpc_password: Option<String>,
    
    /// Path to store Bitcoin blockchain data
    pub data_dir: PathBuf,
    
    /// Number of blocks to process in parallel during initial sync
    #[serde(default = "default_parallel_blocks")]
    pub parallel_blocks: usize,
    
    /// Batch size for database writes
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetricsConfig {
    /// Enable or disable metrics collection
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    /// Address and port for the metrics HTTP server
    #[serde(default = "default_metrics_address")]
    pub bind_address: String,
    
    /// Path for the metrics endpoint
    #[serde(default = "default_metrics_path")]
    pub metrics_path: String,
    
    /// Path for the health check endpoint
    #[serde(default = "default_health_path")]
    pub health_path: String,
    
    /// Enable/disable process metrics collection
    #[serde(default = "default_true")]
    pub process_metrics: bool,
    
    /// Enable/disable system metrics collection
    #[serde(default = "default_false")]
    pub system_metrics: bool,
    
    /// Metrics collection interval in seconds
    #[serde(default = "default_collection_interval")]
    pub collection_interval: u64,
    
    /// Custom labels to add to all metrics
    #[serde(default)]
    pub labels: std::collections::HashMap<String, String>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        let mut labels = std::collections::HashMap::new();
        labels.insert("environment".to_string(), "development".to_string());
        labels.insert("service".to_string(), "chrononode-archival".to_string());
        
        Self {
            enabled: true,
            bind_address: default_metrics_address(),
            metrics_path: default_metrics_path(),
            health_path: default_health_path(),
            process_metrics: true,
            system_metrics: false,
            collection_interval: default_collection_interval(),
            labels,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (error, warn, info, debug, trace)
    #[serde(default = "default_log_level")]
    pub level: String,
    
    /// Path to log file (if not set, logs to stderr)
    pub log_file: Option<PathBuf>,
}

// Default values
fn default_parallel_blocks() -> usize {
    4
}

fn default_batch_size() -> usize {
    1000
}

fn default_log_level() -> String {
    "info".to_string()
}

// Default values for metrics configuration
fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_metrics_address() -> String {
    "0.0.0.0:9090".to_string()
}

fn default_metrics_path() -> String {
    "/metrics".to_string()
}

fn default_health_path() -> String {
    "/health".to_string()
}

fn default_collection_interval() -> u64 {
    15
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bitcoin: BitcoinConfig {
                rpc_url: "http://localhost:8332".to_string(),
                rpc_username: None,
                rpc_password: None,
                data_dir: std::env::current_dir().unwrap_or_default().join("data/bitcoin"),
                parallel_blocks: default_parallel_blocks(),
                batch_size: default_batch_size(),
            },
            logging: LoggingConfig {
                level: default_log_level(),
            },
            metrics: MetricsConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let config_content = std::fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&config_content)?;
        
        // Validate configuration
        if config.bitcoin.rpc_url.is_empty() {
            return Err(ConfigError::Validation("Bitcoin RPC URL cannot be empty".into()));
        }
        
        // Create data directory if it doesn't exist
        if !config.bitcoin.data_dir.exists() {
            std::fs::create_dir_all(&config.bitcoin.data_dir)?;
        }
        
        Ok(config)
    }
    
    /// Load configuration from default locations or use defaults
    pub fn load() -> Result<Self, ConfigError> {
        // Try to load from default config path
        let default_path = "./config.toml";
        if Path::new(default_path).exists() {
            return Self::from_file(default_path);
        }
        
        // Fall back to default configuration
        Ok(Self::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.bitcoin.rpc_url, "http://localhost:8332");
        assert_eq!(config.bitcoin.parallel_blocks, 4);
        assert_eq!(config.bitcoin.batch_size, 1000);
        assert_eq!(config.logging.level, "info");
    }
    
    #[test]
    fn test_load_from_file() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let config_content = r#"
            [bitcoin]
            rpc_url = "http://localhost:18443"
            data_dir = "/tmp/bitcoin_data"
            
            [logging]
            level = "debug"
        "#;
        
        std::fs::write(&config_path, config_content).unwrap();
        
        let config = Config::from_file(&config_path).unwrap();
        assert_eq!(config.bitcoin.rpc_url, "http://localhost:18443");
        assert_eq!(config.bitcoin.data_dir, PathBuf::from("/tmp/bitcoin_data"));
        assert_eq!(config.logging.level, "debug");
    }
    
    #[test]
    fn test_invalid_config() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("invalid.toml");
        std::fs::write(&config_path, "invalid toml").unwrap();
        
        let result = Config::from_file(&config_path);
        assert!(result.is_err());
    }
}
