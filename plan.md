# ChronoNode — Development Plan

**Project**: ChronoNode — Verifiable Archival Layer for Blockchain and App-Ledger History
**Last updated**: 2026-05-23
**Role in ecosystem**: Archives BaaLS + EVM + Bitcoin/DOGE blocks; will provide dormancy proofs for Resurgence Protocol

---

## Current State

| Area | Status | Notes |
|------|--------|-------|
| Core archive pipeline | ✅ Done | `archive_block()`, content-addressed storage, compression |
| Storage backends | ✅ Done | local_fs, IPFS, Pinata, Arweave, S3 via `object_store` |
| BaaLS adapter | ✅ Done | HTTP fetch from BaaLS `/api/v1/blocks/{index}` |
| Bitcoin adapter | ✅ Done | Full block ingestion via bitcoind JSON-RPC (requires full node) |
| Ethereum adapter | ✅ Done | Full block ingestion via ETH JSON-RPC |
| Mock adapter | ✅ Done | Testing |
| Checkpoint + Merkle proofs | ✅ Done | `CheckpointBuilder`, `/v1/proofs/verify`, signed checkpoints |
| HTTP REST API | ✅ Done | Axum — blocks, proofs, checkpoints, chains, GraphQL, JSON-RPC |
| CLI | ✅ Done | `ingest`, `query`, `prove`, `verify`, `serve` |
| Follow mode | ✅ Done | `--follow` flag polls chain every 5s |
| Address watch list | ✅ Done | SQLite `watched_addresses` table, `chrononode watch add/list/remove` CLI, REST endpoint |
| Dormancy detection | ✅ Done | `chrononode dormancy scan` CLI, REST endpoints, signed proofs |
| Lightweight BTC adapter | ✅ Done | Blockstream API, config `mode = "light"` |
| Dogecoin adapter | ✅ Done | BlockCypher API for DOGE blocks + txs |
| BaaLS attestation submitter | ✅ Done | `BaalsSubmitter` POSTs dormancy proof as BaaLS `Data` tx |
| EVM submitter | ✅ Done | keccak256 function selector + Solidity ABI encoding for RewardDistributor |
| Systemd service | ✅ Deployed | Verified live on VPS (2026-05-23): `chrononode.service`, `chrononode-ingest@bitcoin-light`, `chrononode-ingest@dogecoin` |

---

## Phase 1 — Address Watch List + Activity Index

Enable ChronoNode to monitor specific wallet addresses rather than ingesting entire chain history.
This is the foundational requirement for the Resurgence dormancy oracle.

| # | Task | Status |
|---|------|--------|
| 1.1 | Add `WatchList` store — SQLite table `watched_addresses(chain_id, address, added_at_block)` | ✅ |
| 1.2 | Add `ActivityIndex` store — `last_seen(chain_id, address, block_height, tx_hash)` | ✅ |
| 1.3 | Update `archive_block()` to scan tx outputs/inputs against watch list and write to ActivityIndex | ✅ |
| 1.4 | CLI: `chrononode watch add --chain bitcoin --address <addr>` | ✅ |
| 1.5 | CLI: `chrononode watch list --chain bitcoin` | ✅ |
| 1.6 | REST: `GET /v1/chains/{chain_id}/addresses/{address}/last-seen` | ✅ |
| 1.7 | Tests: watch list round-trip, activity index populated from mock blocks | ✅ |

---

## Phase 2 — Lightweight Bitcoin and DOGE Adapters

Replace the full-node dependency with API-based adapters so ChronoNode can monitor
BTC/DOGE addresses without running 600GB+ full nodes locally.

| # | Task | Status |
|---|------|--------|
| 2.1 | `BitcoinLightAdapter` — Blockstream REST API (`https://blockstream.info/api`) for block + address data | ✅ |
| 2.2 | `DogeAdapter` — BlockCypher API for DOGE address activity | ✅ |
| 2.3 | Feature-flag existing `BitcoinAdapter` (full-node) vs `BitcoinLightAdapter` (API) in config | ✅ |
| 2.4 | Rate-limit + retry logic for API adapters (respect API limits) | ✅ |
| 2.5 | Config: `[adapters.bitcoin] mode = "light" api_url = "https://blockstream.info/api"` | ✅ |
| 2.6 | Tests: mock Blockstream responses, verify address activity indexed correctly | ✅ |

---

## Phase 3 — Dormancy Detection + Proof Generation

Build the dormancy determination layer on top of the activity index.
A wallet is dormant if it has had no outbound activity for at least `dormancy_threshold` blocks.

| # | Task | Status |
|---|------|--------|
| 3.1 | `DormancyConfig` — configurable threshold per chain (e.g., BTC: 26280 blocks ≈ 5 years) | ✅ |
| 3.2 | `DormancyIndex` — `dormant_since(chain_id, address, block_height, threshold_blocks)` | ✅ |
| 3.3 | Background job: scan ActivityIndex, write DormancyIndex for qualifying addresses | ✅ |
| 3.4 | `DormancyProof` struct — chain_id, address, dormant_since_block, current_block, threshold | ✅ |
| 3.5 | `generate_dormancy_proof(chain_id, address)` — produces signed DormancyProof | ✅ |
| 3.6 | REST: `GET /v1/chains/{chain_id}/addresses/{address}/dormancy` | ✅ |
| 3.7 | REST: `GET /v1/chains/{chain_id}/addresses/{address}/dormancy/proof` | ✅ |
| 3.8 | Operator keypair (ed25519) signs DormancyProof — verifiable by EVM contracts | ✅ |
| 3.9 | Tests: address with no activity past threshold marked dormant; active address not marked | ✅ |

---

## Phase 4 — BaaLS Attestation Submitter

When a dormancy proof is generated, submit it as a transaction to the BaaLS chain
so the determination is immutably recorded before touching EVM.

| # | Task | Status |
|---|------|--------|
| 4.1 | `BaalsSubmitter` — connects to BaaLS HTTP API, holds a signing keypair | ✅ |
| 4.2 | On new DormancyProof: submit BaaLS transaction with proof as payload | ✅ |
| 4.3 | Config: `[attestation] baals_api_url = "http://localhost:18080" baals_key_path = "/etc/chrononode/baals.key"` | ✅ |
| 4.4 | Idempotency — do not resubmit if BaaLS already has attestation for address+block | ✅ |
| 4.5 | REST: `POST /v1/attestations/submit` — trigger manual attestation submission | ✅ |
| 4.6 | Tests: mock BaaLS HTTP, verify correct transaction payload submitted | ✅ |

---

## Phase 5 — Resurgence Protocol Integration

Wire ChronoNode dormancy proofs into the Resurgence EVM oracle consumer.

| # | Task | Status |
|---|------|--------|
| 5.1 | EVM submitter — read BaaLS-attested proofs, post signed proof to Resurgence RewardDistributor | ✅ keccak256 function selector + proper Solidity ABI encoding |
| 5.2 | Authorize ChronoNode operator address in RewardDistributor as trusted oracle | ⏳ commands documented in resurgence-protocol/plan.md Phase 11 wiring section |
| 5.3 | Systemd service for ChronoNode on VPS (`chrononode serve --follow` for BTC/DOGE/BaaLS) | ✅ deployed and active (verified 2026-05-23) |
| 5.4 | Watch list pre-populated — CLI `chrononode watch import --file` for bulk import | ⏳ deployment is live, but watch lists are still empty on VPS and must be imported |
| 5.5 | End-to-end test: dormant BTC wallet → ChronoNode proof → BaaLS attestation → EVM oracle → RESURGE mint | ✅ scripts/deployAndTestE2E.js — local Hardhat node, full flow verified |

---

## Phase 6 — Operational Hardening

| # | Task | Status |
|---|------|--------|
| 6.1 | Prometheus metrics: blocks ingested/s, watch list size, dormancy detections, BaaLS submission failures | ✅ |
| 6.2 | Caddy route for ChronoNode REST API at `chrono.baals.network` | ✅ deploy/caddy/Caddyfile — reverse proxy + JSON logs + TLS |
| 6.3 | Structured logging (JSON) — use `RUST_LOG=info` with `tracing-subscriber` json feature | ✅ |
| 6.4 | fail2ban jail for ChronoNode API (rate limit abusive proof requests) | ✅ deploy/fail2ban/ — filter + jail config |

---

## Architecture

```
Bitcoin / DOGE / BaaLS chain
        ↓ (Blockstream API / BaaLS HTTP)
  ChronoNode ingest --follow
        ↓
  WatchList filter → ActivityIndex
        ↓
  DormancyIndex (threshold exceeded)
        ↓
  DormancyProof (ed25519 signed)
        ↓
  BaalsSubmitter → BaaLS chain (immutable record)
        ↓
  EVMSubmitter → Resurgence RewardDistributor
        ↓
  RESURGE minted to staker
```

---

## Quality Gates

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-features
```

---

## Remaining Work (Current)

Open work:
- L6. Optional MongoDB/Scylla index backends (feature-gated)

Recently completed:
- L1. Property-based tests (`proptest`) for Merkle/proof/signing invariants
- L2. Criterion benchmark harness for Merkle root/proof generation/proof verification
- L3. API rate limiter hardened with atomic token-bucket state + deterministic unit tests
- L4. Pagination for list endpoints (`page`/`per_page`) with API coverage
- L5. Adapter config hot reload with test coverage
- L7. Grafana dashboard JSON in `deploy/grafana/chrononode-dashboard.json`
