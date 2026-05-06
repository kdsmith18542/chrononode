# ChronoNode

ChronoNode is an independent verifiable archival layer for blockchain and app-ledger history.

It archives blocks and events from supported networks into content-addressed storage, stores compact metadata locally, and serves historical queries with Merkle proofs.

## Quick Start

```bash
docker compose up
```

Or build from source:

```bash
cargo build --release --workspace
./target/release/chrononode-cli init
./target/release/chrononode-cli ingest --chain mock --from 0
./target/release/chrononode-cli query block --chain mock --height 0
./target/release/chrononode-cli prove --chain mock --height 0
```

## First Supported Network

BaaLS is the first reference adapter. ChronoNode is not limited to BaaLS; additional networks can be supported through the ChainAdapter interface.

## MVP Flow

```
BaaLS block → ChronoBlock protobuf → IPFS/local storage → SQLite index → query API → Merkle proof
```

## Project Structure

```
crates/
├── chrononode-core/    # Models, traits, proof logic, error types (no I/O)
└── chrononode-cli/     # CLI binary: adapters, storage, index, API, verification
proto/                  # Protobuf schema (ChronoBlock, ChronoTx, ChronoEvent)
tests/                  # Integration tests
docs/                   # Architecture and design docs
```

## Status

Early design/build phase. Not production-ready.

## License

MIT OR Apache-2.0
