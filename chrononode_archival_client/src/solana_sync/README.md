# Solana Synchronization Module

This module implements full blockchain synchronization for Solana in the ChronoNode Archival Client.

## Features

- Full blockchain synchronization from a Solana RPC endpoint
- Efficient block and transaction storage using RocksDB
- Account state tracking
- Configurable batch processing for optimal performance
- Support for mainnet, testnet, and devnet
- Progress tracking and state persistence

## Architecture

### Components

1. **SolanaSyncClient**: Main entry point for synchronization
2. **RPC Client**: Handles communication with the Solana node
3. **Database Layer**: Manages storage of blocks, transactions, and accounts
4. **State Management**: Tracks synchronization progress and chain state

### Data Flow

1. The sync process starts by querying the current slot from the Solana node
2. For each slot to be processed:
   - Fetch block data from the Solana node
   - Store block data and transactions in the database
   - Update account states
   - Update chain state

## Usage

```rust
use chrononode_archival_client::{
    solana_sync::SolanaSyncClient,
    config::Config,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::init();
    
    // Load configuration
    let config = Config::load()?;
    
    // Create and start the sync client
    let mut client = SolanaSyncClient::new(config).await?;
    
    // Start synchronization
    client.sync_blocks().await?;
    
    Ok(())
}
```

## Configuration

The module can be configured using the following settings in `config.toml`:

```toml
[solana]
rpc_url = "http://localhost:8899"
data_dir = "./data/solana_blocks"
network = "mainnet-beta"  # or "testnet", "devnet", "localnet"
batch_size = 100

[logging]
level = "info"
log_file = "./logs/solana_sync.log"
```

## Database Schema

The module uses the following column families in RocksDB:

- `blocks`: Stores serialized block data by slot number
- `transactions`: Stores transaction data by signature
- `accounts`: Tracks account states
- `slot_metadata`: Stores synchronization state and metadata

## Performance Considerations

- **Batch Processing**: Configure `batch_size` based on available memory
- **Network**: Use a reliable RPC endpoint with high rate limits
- **Storage**: Use fast SSDs for better performance with large blockchains
- **Memory**: Allocate sufficient memory for account state caching

## Error Handling

The module provides detailed error types and messages for common failure scenarios, including:

- RPC communication errors
- Block processing failures
- Database errors
- Chain reorganization handling

## License

This project is licensed under either of:

 * Apache License, Version 2.0
 * MIT license

at your option.
