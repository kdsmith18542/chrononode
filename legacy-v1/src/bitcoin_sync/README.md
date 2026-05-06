# Bitcoin Synchronization Module

This module implements full blockchain synchronization for Bitcoin in the ChronoNode Archival Client.

## Features

- Full blockchain synchronization from a Bitcoin Core node
- Efficient block and transaction storage using RocksDB
- UTXO set management
- Configurable batch processing for optimal performance
- Support for reorgs and chain reorganization
- Progress tracking and state persistence

## Architecture

### Components

1. **BitcoinSyncClient**: Main entry point for synchronization
2. **BitcoinRpcClient**: Handles communication with the Bitcoin Core RPC
3. **Database Layer**: Manages storage of blocks, transactions, and UTXO set
4. **State Management**: Tracks synchronization progress and chain state

### Data Flow

1. The sync process starts by querying the current chain tip from the Bitcoin node
2. For each block to be processed:
   - Fetch block data from the Bitcoin node
   - Validate block structure and proof of work
   - Update UTXO set (spend inputs, add new outputs)
   - Store block and transaction data
   - Update chain state

## Usage

```rust
use chrononode_archival_client::{
    bitcoin_sync::BitcoinSyncClient,
    config::Config,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize configuration
    let config = Config::load()?;
    
    // Create and start the sync client
    let mut client = BitcoinSyncClient::new(config).await?;
    
    // Start synchronization
    client.sync_blocks().await?;
    
    Ok(())
}
```

## Configuration

The module can be configured using the following settings in `config.toml`:

```toml
[bitcoin]
rpc_url = "http://localhost:8332"
rpc_username = "your_rpc_username"
rpc_password = "your_rpc_password"
data_dir = "./data/bitcoin_blocks"
parallel_blocks = 4
batch_size = 1000

[logging]
level = "info"
log_file = "./logs/bitcoin_sync.log"
```

## Testing

Run the tests with:

```bash
cargo test --package chrononode_archival_client --lib bitcoin_sync
```

## Performance Considerations

- **Batch Processing**: Configure `batch_size` based on available memory
- **Parallelism**: Adjust `parallel_blocks` based on CPU cores
- **Storage**: Use fast SSDs for better performance with large blockchains
- **Memory**: Allocate sufficient memory for the UTXO set

## Error Handling

The module provides detailed error types and messages for common failure scenarios, including:

- RPC communication errors
- Block validation failures
- Database errors
- Chain reorganization handling

## License

This project is licensed under either of:

 * Apache License, Version 2.0
 * MIT license

at your option.
