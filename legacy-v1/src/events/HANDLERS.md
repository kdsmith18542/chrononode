# Chain-Specific Event Handlers

This document provides an overview of the chain-specific event handlers available in ChronoNode for processing blockchain events from different networks.

## Available Handlers

### 1. Bitcoin Event Handler (`bitcoin_events.rs`)

Handles Bitcoin blockchain events including:
- New blocks and transactions
- UTXO state changes
- Coinbase transactions

**Features:**
- Processes Bitcoin blocks and transactions
- Tracks UTXO creation and spending
- Handles testnet and mainnet
- Supports regtest and signet for testing

**Usage:**
```rust
use chrononode_archival_client::events::{
    bitcoin_events::{BitcoinEventHandler, BitcoinNetwork},
    EventBus,
};
use std::sync::Arc;

// Create an event bus
let event_bus = Arc::new(EventBus::new());

// Create a Bitcoin event handler for mainnet
let bitcoin_handler = BitcoinEventHandler::new(event_bus.clone(), BitcoinNetwork::Mainnet);

// Process a Bitcoin block
bitcoin_handler.process_block(&block, height, true).await?;
```

### 2. Ethereum Event Handler (`ethereum_events.rs`)

Handles Ethereum blockchain events including:
- New blocks and transactions
- Contract events and logs
- ERC-20/721 token transfers

**Features:**
- Processes Ethereum blocks and transactions
- Extracts and decodes contract events
- Handles mainnet and testnets
- Supports WebSocket and HTTP providers

**Usage:**
```rust
use chrononode_archival_client::events::{
    ethereum_events::EthereumEventHandler,
    EventBus,
};
use std::sync::Arc;

// Create an event bus
let event_bus = Arc::new(EventBus::new());

// Create an Ethereum event handler for mainnet (chain_id = 1)
let ethereum_handler = EthereumEventHandler::new(event_bus.clone(), 1);

// Process an Ethereum block
ethereum_handler.process_block(&block, &receipts).await?;
```

### 3. Solana Event Handler (`solana_events.rs`)

Handles Solana blockchain events including:
- New slots and transactions
- Token balance changes
- Program executions

**Features:**
- Processes Solana slots and transactions
- Tracks token balances and transfers
- Handles mainnet, testnet, and devnet
- Supports JSON-RPC and WebSocket connections

**Usage:**
```rust
use chrononode_archival_client::events::{
    solana_events::{SolanaEventHandler, SolanaCluster},
    EventBus,
};
use std::sync::Arc;

// Create an event bus
let event_bus = Arc::new(EventBus::new());

// Create a Solana event handler for mainnet-beta
let solana_handler = SolanaEventHandler::new(event_bus.clone(), SolanaCluster::MainnetBeta);

// Process a Solana slot
solana_handler.process_slot(slot, &block, block_time, true).await?;
```

## Common Patterns

### Subscribing to Events

All event handlers publish events to the event bus. You can subscribe to these events:

```rust
use chrononode_archival_client::events::{EventBus, EventType};
use std::sync::Arc;

let event_bus = Arc::new(EventBus::new());

// Subscribe to block events
event_bus.subscribe(EventType::Block, |event| {
    println!("New block: {:?}", event);
    Ok(())
}).await?;

// Subscribe to transaction events
event_bus.subscribe(EventType::Transaction, |event| {
    println!("New transaction: {:?}", event);
    Ok(())
}).await?;

// Start the event bus
event_bus.start().await;
```

### Error Handling

All handler methods return `Result<()>` and should be properly handled:

```rust
if let Err(e) = handler.process_block(block).await {
    log::error!("Failed to process block: {}", e);
    // Handle error (e.g., retry, skip, etc.)
}
```

## Configuration

Each handler can be configured with chain-specific parameters:

```rust
// Bitcoin configuration
let bitcoin_handler = BitcoinEventHandler::new(
    event_bus.clone(),
    BitcoinNetwork::Mainnet,
);

// Ethereum configuration
let ethereum_handler = EthereumEventHandler::new(
    event_bus.clone(),
    1, // chain_id
);

// Solana configuration
let solana_handler = SolanaEventHandler::new(
    event_bus.clone(),
    SolanaCluster::MainnetBeta,
);
```

## Testing

Each handler includes unit tests that can be run with:

```bash
# Run all tests
cargo test

# Run Bitcoin tests
cargo test --features="bitcoin"

# Run Ethereum tests
cargo test --features="ethereum"

# Run Solana tests
cargo test --features="solana"
```

## Dependencies

- `bitcoin`: For Bitcoin data structures and serialization
- `ethers`: For Ethereum interaction and ABI decoding
- `solana-sdk`: For Solana account and transaction handling
- `tokio`: For async runtime
- `anyhow`: For error handling
- `log`: For logging

## Notes

- All handlers are designed to be chain-agnostic where possible
- Event formats are standardized across chains
- Handlers are designed to be composable and extensible
- Thread-safe by default (uses `Arc` for shared state)
