# ChronoNode Archival Client

A high-performance blockchain archival client for the ChronoNode ecosystem, supporting multiple blockchains with a focus on data integrity and availability.

## Features

- **Bitcoin Synchronization**
  - Full blockchain synchronization
  - UTXO set management
  - Efficient block and transaction storage
  - RPC client integration

- **Ethereum Support** (coming soon)
- **Solana Support** (coming soon)

## Getting Started

### Prerequisites

- Rust (latest stable version)
- RocksDB development libraries
- Bitcoin Core node (for Bitcoin synchronization)

### Installation

```bash
# Clone the repository
git clone https://github.com/chrononode/chrononode-archival-client.git
cd chrononode-archival-client

# Build in release mode
cargo build --release
```

### Configuration

Create a `config.toml` file in the project root:

```toml
[bitcoin]
rpc_url = "http://localhost:8332"
rpc_username = "your_rpc_username"
rpc_password = "your_rpc_password"
data_dir = "./data/bitcoin_blocks"

[logging]
level = "info"
log_file = "./logs/chrononode.log"
```

### Running the Client

```bash
# Run with default configuration
cargo run --release

# Run with custom config
RUST_LOG=info cargo run --release -- --config ./path/to/config.toml
```

## Architecture

The client is built with a modular architecture to support multiple blockchains:

- **Core**: Common utilities and traits
- **Bitcoin Module**: Bitcoin-specific synchronization logic
- **Storage**: Database and file system integration
- **RPC**: JSON-RPC client implementation

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy -- -D warnings
```

## License

Licensed under either of:

 * Apache License, Version 2.0
 * MIT license

at your option.
