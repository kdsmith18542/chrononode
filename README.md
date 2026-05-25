# ChronoNode

ChronoNode is an independent verifiable archival layer for blockchain and app-ledger history.

It archives blocks and events from supported networks into content-addressed storage, stores compact metadata locally, and serves historical queries with Merkle proofs.

## Quick Start

```bash
docker compose up
```

That starts two services on one VPS:
- `chrononode-ingest` (continuous ingest loop)
- `chrononode-api` (HTTP API on `:8080`)

If you want local IPFS on the same VPS:

```bash
docker compose --profile ipfs up
```

Or build from source:

```bash
cargo build --release --workspace
./target/release/chrononode-cli init
./target/release/chrononode-cli ingest --chain mock --from 0
./target/release/chrononode-cli query block --chain mock --height 0
./target/release/chrononode-cli prove --chain mock --height 0
```

### Storage Backend Selection

ChronoNode uses `local_fs` by default. You can switch to IPFS or Pinata with environment variables:

```bash
# local filesystem (default)
CHRONONODE_STORAGE_BACKEND=local_fs

# local IPFS node
CHRONONODE_STORAGE_BACKEND=ipfs
CHRONONODE_IPFS_API_URL=http://127.0.0.1:5001

# Pinata
CHRONONODE_STORAGE_BACKEND=pinata
CHRONONODE_PINATA_JWT=your_jwt_here
# Optional overrides:
# CHRONONODE_PINATA_API_BASE=https://api.pinata.cloud
# CHRONONODE_PINATA_GATEWAY_BASE=https://gateway.pinata.cloud
```

### Index Backend Selection

Set `CHRONONODE_INDEX_BACKEND` to select index persistence:

```bash
CHRONONODE_INDEX_BACKEND=sqlite   # default
CHRONONODE_INDEX_BACKEND=postgres
# Feature-gated compatibility aliases:
CHRONONODE_INDEX_BACKEND=mongodb
CHRONONODE_INDEX_BACKEND=scylla
```

## First Supported Network

BaaLS is the first reference adapter. ChronoNode is not limited to BaaLS; additional networks can be supported through the ChainAdapter interface.

## One VPS Now, Easy Scale Later

Run ingest and API as separate processes (or containers) even on one VPS. This keeps scale-out simple:
- Move `chrononode-ingest` to a second VPS later.
- Keep `chrononode-api` on the first VPS.
- Keep storage backend env config stable (`local_fs`, `ipfs`, or `pinata`).

Systemd templates for this topology are included in:

```text
deploy/systemd/
```

## MVP Flow

```
BaaLS block -> ChronoBlock protobuf -> IPFS/local storage -> SQLite index -> query API -> Merkle proof
```

## Project Structure

```
crates/
|-- chrononode-core/           # Models, traits, proof logic, error types (no I/O)
|-- chrononode-adapter-sdk/    # Adapter registry and utilities
|-- chrononode-adapter-mock/   # Deterministic mock chain for testing
|-- chrononode-adapter-baals/  # BaaLS JSON-RPC adapter
|-- chrononode-adapter-localfile/  # Local file import adapter
`-- chrononode-cli/            # CLI binary: adapters, storage, index, API, verification
proto/                         # Protobuf schema (ChronoBlock, ChronoTx, ChronoEvent)
tests/                         # Integration tests
docs/                          # Architecture and design docs
```

## CLI Commands

```bash
# Initialization
chrononode init                          # Create config, keypair, directories
chrononode config show                   # Show current configuration

# Ingestion
chrononode ingest --chain mock --from 0  # Ingest blocks from height 0
chrononode ingest --chain mock --follow  # Follow new blocks continuously
chrononode ingest --chain local-file --from 0  # Import from local JSON files

# Queries
chrononode query block --chain mock --height 0
chrononode query txs-by-sender --chain mock --sender <hex>
chrononode query txs-by-recipient --chain mock --recipient <hex>
chrononode query events-by-type --chain mock --event-type <type>

# Proofs & Checkpoints
chrononode prove --chain mock --height 0 --out proof.json
chrononode verify proof.json
chrononode checkpoint create --chain mock --from 0 --to 999
chrononode export-checkpoint --id mock-0-999 --out checkpoint.json

# Maintenance
chrononode repair --chain mock --height 500
chrononode verify-archive --chain mock --from 0 --to 1000
chrononode stats --chain mock
chrononode backup --chain mock --out backup.db
chrononode restore --chain mock --from backup.db

# API Server
chrononode serve --port 8080 --chain mock

# Adapters
chrononode adapters                        # List registered adapters
```

## HTTP API

Start the API server with `chrononode serve`, then:

```bash
GET  /health
GET  /v1/chains
GET  /v1/chains/{chain_id}/blocks/{height}
GET  /v1/chains/{chain_id}/blocks?from=0&to=100
GET  /v1/chains/{chain_id}/blocks/hash/{hash}
POST /v1/chains/{chain_id}/checkpoints     # Create checkpoint
GET  /v1/checkpoints/{checkpoint_id}
GET  /v1/chains/{chain_id}/proofs/block/{height}
POST /v1/proofs/verify
GET  /v1/chains/{chain_id}/txs/sender/{sender}
GET  /v1/chains/{chain_id}/txs/recipient/{recipient}
GET  /v1/chains/{chain_id}/events/{event_type}
GET  /metrics                              # Prometheus metrics
GET  /api-docs                             # OpenAPI/Swagger UI
```

## Status

Active development with live VPS deployment for API + ingest services.
Core archival, proof, dormancy, attestation, and EVM submission flows are implemented and covered by passing tests.

## License

MIT OR Apache-2.0
