# Ethereum Synchronization Module

This module implements full blockchain synchronization for Ethereum in the ChronoNode Archival Client.

## Features

- Full blockchain synchronization from an Ethereum node
- Efficient block and transaction storage using RocksDB
- Transaction receipt handling
- Configurable batch processing for optimal performance
- Support for mainnet and testnets
- Progress tracking and state persistence

## Architecture

### Components

1. **EthereumSyncClient**: Main entry point for synchronization
2. **Ethereum Provider**: Handles communication with the Ethereum node
3. **Database Layer**: Manages storage of blocks, transactions, and receipts
4. **State Management**: Tracks synchronization progress and chain state

### Data Flow

1. The sync process starts by querying the current block number from the Ethereum node
2. For each block to be processed:
   - Fetch block data from the Ethereum node
   - Fetch transaction receipts for all transactions in the block
   - Store block data, transactions, and receipts in the database
   - Update chain state

## Usage

```rust
use chrononode_archival_client::{
    ethereum_sync::EthereumSyncClient,
    config::Config,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::init();
    
    // Load configuration
    let config = Config::load()?;
    
    // Create and start the sync client
    let mut client = EthereumSyncClient::new(config).await?;
    
    // Start synchronization
    client.sync_blocks().await?;
    
    Ok(())
}
```

## Configuration

The module can be configured using the following settings in `config.toml`:

```toml
[ethereum]
rpc_url = "http://localhost:8545"
data_dir = "./data/ethereum_blocks"
parallel_blocks = 4
batch_size = 100

[logging]
level = "info"
log_file = "./logs/ethereum_sync.log"
```

## Database Schema

The module uses the following column families in RocksDB:

- `blocks`: Stores serialized block data by block number
- `transactions`: Stores transaction data by transaction hash
- `receipts`: Stores transaction receipts by transaction hash
- `state`: Stores synchronization state and metadata

## Testing

Run the tests with:

```bash
cargo test --package chrononode_archival_client --lib ethereum_sync
```

## Performance Considerations

- **Batch Processing**: Configure `batch_size` based on available memory
- **Parallelism**: Adjust `parallel_blocks` based on CPU cores
- **Storage**: Use fast SSDs for better performance with large blockchains
- **Memory**: Allocate sufficient memory for the state trie and caches

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
