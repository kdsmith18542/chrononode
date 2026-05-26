# ChronoNode — Agent Context

## Project Overview

ChronoNode is a verifiable archival layer for blockchain and app-ledger history.
It ingests blocks from multiple chains, stores them content-addressed, generates
Merkle checkpoints, and exposes a REST/GraphQL/JSON-RPC API for proof verification.

**Primary role in ecosystem**: Live dormancy detection oracle for Resurgence Protocol.
Monitors BTC/DOGE wallets, signs `DormancyProof`s (ed25519 or SP1 Groth16 zkVM),
and POSTs them to BaaLS for attestation. BaaLS EVMSubmitter then calls
`RewardDistributor.submitDormancyProof()` on Arbitrum Sepolia.
Full pipeline verified end-to-end 2026-05-24. All phases (1–8) merged to `master` 2026-05-26.

## Workspace Structure

```
chrononode/           ← repo root
├── crates/
│   ├── chrononode-core/          # Shared models, config, proofs, signing, zkvm types, error types
│   ├── chrononode-adapter-baals/ # BaaLS HTTP adapter (WORKING)
│   ├── chrononode-adapter-bitcoin-light/ # Blockstream/Esplora REST-based BTC adapter
│   ├── chrononode-adapter-doge/  # BlockCypher DOGE adapter
│   ├── chrononode-adapter-bitcoin/  # Full-block bitcoind JSON-RPC (requires full node)
│   ├── chrononode-adapter-ethereum/ # Full-block ETH JSON-RPC
│   ├── chrononode-adapter-mock/     # Testing
│   ├── chrononode-cli/              # CLI binary + HTTP API + index backends + storage
│   ├── chrononode-sdk/              # Client SDK crate
│   └── chrononode-zkvm-program/     # SP1 guest program (workspace exclude — builds via sp1 toolchain only)
├── explorer/         # Next.js block explorer frontend (19 pages)
├── proto/            # Protobuf definitions (ChronoBlock)
├── deploy/           # systemd, caddy, fail2ban, Grafana, Arweave anchor timers, and helper scripts
├── sdk/              # Python + TypeScript SDKs
└── tests/
```

> **Workspace note**: `chrononode-zkvm-program` is in `[workspace.exclude]`, not `members`.
> It is an SP1 `#![no_main]` guest binary that only runs inside the SP1 zkVM. Build it
> via `sp1 build`, not `cargo build`.

## Key Types

- `ChainAdapter` trait — `fetch_block(height)`, `latest_height()`, `chain_id()`
- `ArchivePipeline::archive_block(height)` — fetch → serialize → compress → store → index
- `StoragePointer` — `{backend}:{key}` (backends: local_fs, ipfs, pinata, arweave, s3)
- `CheckpointResult` — `{chain_id}-{start}-{end}`, root_hash `[u8;32]`, optional ed25519 sig
- `ProofJson` — chain_id, height, block_hash, storage_pointer, checkpoint, merkle siblings

## Commands

```bash
# Build
cargo build --release

# Test (161 tests, all pass)
cargo test --all-features

# Ingest BaaLS blocks (follow mode)
chrononode ingest --chain baals --follow

# Ingest Bitcoin (requires bitcoind or Blockstream API adapter — see plan.md Phase 2)
chrononode ingest --chain bitcoin --from 0 --follow

# Generate checkpoint over block range
chrononode checkpoint create --chain baals --from 100 --to 200

# Verify a proof
chrononode verify proof.json

# Generate SP1 zkVM proof for a dormant address
chrononode prove --zkvm sp1 --address <addr> [--mock]

# Start API server
chrononode serve --port 8080
```

## REST API

| Endpoint | Description |
|----------|-------------|
| `GET /v1/chains` | List available chains |
| `GET /v1/chains/{chain_id}/blocks/{height}` | Fetch archived block |
| `GET /v1/chains/{chain_id}/addresses/{address}/last-seen` | Last activity |
| `GET /v1/chains/{chain_id}/addresses/{address}/dormancy` | Dormancy status |
| `GET /v1/chains/{chain_id}/addresses/{address}/dormancy/proof` | Signed dormancy proof |
| `GET /v1/proofs/{address}/sp1` | SP1 Groth16 zkVM proof |
| `GET /v1/chains/{chain_id}/checkpoints/{height}/anchor` | Arweave anchor TX ID |
| `GET /v1/attestations?chain_id={chain_id}` | List BaaLS attestations |
| `POST /v1/chains/{chain_id}/checkpoints` | Create Merkle checkpoint |
| `POST /v1/proofs/verify` | Verify a proof — returns `{"valid": bool}` |
| `POST /v1/attestations/submit` | Submit dormancy attestation to BaaLS |
| `POST /graphql` | GraphQL endpoint (stats, queries) |

## What Works vs What Doesn't

| Feature | Status |
|---------|--------|
| Block ingestion (BaaLS, BTC, ETH) | ✅ Works — full block archival |
| Content-addressed storage | ✅ Works — local_fs, IPFS, Pinata, Arweave, S3 |
| Merkle checkpoints + proof verify | ✅ Works |
| Address watch list | ✅ Works |
| Last-activity tracking | ✅ Works |
| Dormancy detection + proof generation | ✅ Works |
| Lightweight BTC adapter (Blockstream/Esplora) | ✅ Works |
| Dogecoin adapter (BlockCypher API) | ✅ Works |
| BaaLS attestation submitter | ✅ Works |
| EVM submitter (Resurgence) | ✅ Works (keccak256 + ABI encoding) |
| SP1 Groth16 zkVM proofs | ✅ Works (trustless dormancy verification) |
| Arweave checkpoint anchoring | ✅ Works (weekly systemd timer) |
| Next.js block explorer (19 pages) | ✅ Works — `chrono.baals.network` |
| Quality gates (fmt/clippy/test) | ✅ All pass — 161 tests, 0 failures |

## Mode Selection: Bitcoin Full Node vs Light

The current `BitcoinAdapter` calls `bitcoind` JSON-RPC — requires a ~600GB full node.
**Do not attempt to run full Bitcoin ingestion on the VPS (450GB total NVMe).**
Phase 2 added `BitcoinLightAdapter` (Blockstream/Esplora REST API, no local node needed).
The VPS runs the dedicated `bitcoin-light` chain adapter:
```toml
[adapters.bitcoin-light]
api_url = "https://blockstream.info"
```

## Ecosystem Integration

- **BaaLS** (VPS 198.71.49.148:18080): ChronoNode ingests BaaLS blocks and POSTs signed `DormancyProof`s to `POST /api/v1/oracle/attest`. Ed25519 signing key at `/etc/chrononode/baals.key`. BaaLS EVMSubmitter picks up attested proofs (with `evm_wallet` set) and submits to Arbitrum Sepolia every 30s. Pipeline live since 2026-05-24.
- **Resurgence Protocol** (resurge.baals.network): `RewardDistributor.submitDormancyProof()` is live — `DORMANCY_ORACLE_ROLE` held by BaaLS EVMSubmitter `0x201624cBa366250D08bCdA95e6eF64151687A447`. Full round-trip confirmed tx `0x904d948f...` (1000 RESURGE minted) ✅
- **ChronoNode API** (chrono.baals.network): HTTPS reverse proxy to `localhost:8080` via Caddy. CORS enabled for browser-facing frontend calls.

## VPS Deployment (live — verified 2026-05-24)

```bash
# Active services on VPS
chrononode.service                              # API server, port 8080
chrononode-ingest@bitcoin-light.service         # continuous block ingestion
chrononode-ingest@dogecoin.service              # continuous block ingestion
chrononode-dormancy-scan@bitcoin-light.timer    # dormancy scan every 6h (OnUnitActiveSec=6h)
chrononode-dormancy-scan@dogecoin.timer         # dormancy scan every 6h
baalsd.service                                  # BaaLS node + EVMSubmitter + RelayWatcher
baalsd-node2.service
caddy.service
```

ChronoNode serves `bitcoin-light` on port `8080` behind `https://chrono.baals.network`.
Dormancy scan timers fire every 6h, submit signed proofs to BaaLS `POST /api/v1/oracle/attest`.

**Watch list (as of 2026-05-24)**:
- 28 `bitcoin-light` addresses (BTC genesis block + known dormant wallets) — all with `evm_wallet = 0x42060A5Fc138ee019BC3F777B51c6490A1b881f0`
- 5 `dogecoin` addresses (DDogeparty-burn + whales + early-miner) — all with `evm_wallet`
- Add new addresses: `chrononode watch add --chain <chain> --address <addr> --evm-wallet <0x...>`

**`evm_wallet` field**: watched_addresses DB column that maps a non-EVM address to the EVM wallet that should receive RESURGE. BaaLS EVMSubmitter only processes attestations where `evm_wallet` is set. Set via `--evm-wallet` flag on `watch add` or by direct DB update.

For one-time contract wiring and watch-list bootstrap tasks, use scripts in `deploy/scripts/`.

## Storage Budget

- BaaLS block archival: ~50–200MB/year (small blocks, 5s interval)
- BTC/DOGE watch-list mode: address index only, not full blocks — minimal storage
- Arweave anchoring: checkpoint roots only (~1KB/week per chain)
- VPS has 450GB NVMe; Graph Node + BaaLS use ~5GB currently; plenty of headroom

## Explorer Frontend

Next.js app at `explorer/` with 19 pages:
- Dashboard with chain selector and live pipeline status
- Block detail, transaction detail, address lookup
- Dormancy status and last-seen activity
- Attestation list and detail views
- Checkpoint explorer, proof verification UI
- SP1 zkVM proof viewer
- Health status page
- Storage pointer lookup

Deploy: `chrono.baals.network` via Caddy reverse proxy. Falls back to mock data when API is offline.
