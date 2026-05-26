# ChronoNode — Development Plan

**Project**: ChronoNode — Verifiable Archival Layer for Blockchain and App-Ledger History
**Last updated**: 2026-05-25
**Role in ecosystem**: Live dormancy oracle for Resurgence Protocol. Archives BTC/DOGE activity, generates signed DormancyProofs, submits to BaaLS. BaaLS EVMSubmitter calls RewardDistributor on Arbitrum Sepolia. Full pipeline verified in production 2026-05-24.

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
| Systemd service | ✅ Deployed | `chrononode.service`, `chrononode-ingest@bitcoin-light`, `chrononode-ingest@dogecoin`, `chrononode-dormancy-scan@bitcoin-light.timer`, `chrononode-dormancy-scan@dogecoin.timer` (OnUnitActiveSec=6h) — all live on VPS |
| `evm_wallet` field + `--evm-wallet` CLI flag | ✅ Done | watched_addresses column maps non-EVM address to EVM wallet; required for EVMSubmitter to pick up attestation |
| Production pipeline E2E | ✅ Verified 2026-05-24 | ChronoNode scan → BaaLS attest → EVMSubmitter → RewardDistributor tx `0x904d948f...` (RESURGE minted) ✅ |

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
| 5.2 | Authorize ChronoNode operator address in RewardDistributor as trusted oracle | ✅ automated via `deploy/scripts/authorize-dormancy-oracle.sh` + `docs/operations.md` runbook |
| 5.3 | Systemd service for ChronoNode on VPS (`chrononode serve --follow` for BTC/DOGE/BaaLS) | ✅ deployed and active (verified 2026-05-23) |
| 5.4 | Watch list pre-populated — CLI `chrononode watch import --file` for bulk import | ✅ automated via `deploy/scripts/import-watchlists.sh` using `config/watchlist-*.txt` |
| 5.5 | End-to-end test: dormant BTC wallet → ChronoNode proof → BaaLS attestation → EVM oracle → RESURGE mint | ✅ Local: scripts/deployAndTestE2E.js (Hardhat). Production: ChronoNode timer → BaaLS oracle → EVMSubmitter → RewardDistributor tx `0x904d948f...` on Arbitrum Sepolia ✅ (2026-05-24) |

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
  DormancyProof
  ├── ed25519 signed (current — trusted oracle key)
  └── SP1 Groth16 (Phase 7 — trustless zkVM proof)
        ↓
  BaalsSubmitter → BaaLS chain (immutable record)
  └── Arweave anchor (Phase 8 — permanent checkpoint)
        ↓
  EVMSubmitter → Resurgence RewardDistributor
  ├── submitDormancyProof() via DORMANCY_ORACLE_ROLE (current)
  └── verifyAndMint() via SP1 verifier contract (Phase 7)
        ↓
  RESURGE minted to staker
```

---

---

## Phase 7 — zkVM Proof Mode (SP1 / RISC Zero)

Upgrade dormancy proofs from ed25519-signed assertions to zero-knowledge proofs, enabling
trustless on-chain verification without trusted oracle keys. Phase 3 priority — not required
for testnet, required before mainnet audit.

### Motivation
Current proofs are signed by a ChronoNode-held ed25519 key. Verifiers must trust the oracle
operator. zkVM proofs allow on-chain contracts to verify the dormancy computation itself — no
trust required. Eliminates `DORMANCY_ORACLE_ROLE` attack surface on mainnet.

### Chosen Framework: SP1 (Succinct Labs)
- Rust-native: dormancy detection logic compiles directly to SP1 guest program
- Groth16 on-chain verifier: small calldata, low gas cost (~200k gas)
- Active testnet verifier contracts on Arbitrum Sepolia

| # | Task | Status |
|---|------|--------|
| 7.1 | Extract `DormancyCalculator` into SP1-compatible guest program — pure function: `(WatchedAddress, Vec<BlockSummary>) → DormancyProof` | ✅ |
| 7.2 | Add SP1 SDK dependency to `Cargo.toml` (feature-gated: `--features zkvm`) | ✅ |
| 7.3 | Prover mode: `chrononode prove --zkvm sp1 --address <addr>` — generates proof + public inputs JSON | ✅ |
| 7.4 | Add `proof_type: "sp1_groth16"` field to `DormancyProof` struct alongside existing `"ed25519"` type | ✅ |
| 7.5 | Deploy SP1 verifier contract on Arbitrum Sepolia; store address in `deploy/contracts/sp1-verifier.json` | ✅ |
| 7.6 | Update `RewardDistributor` (Resurgence) to accept SP1 proof via `verifyAndMint()` as alternative to `DORMANCY_ORACLE_ROLE` — trustless zkVM verification | ✅ |
| 7.7 | Tests: SP1 guest program round-trips (dormant wallet → proof → verify), property tests for edge cases | ✅ |
| 7.8 | Public proof explorer endpoint: `GET /v1/proofs/{address}/sp1` — returns proof + public inputs for independent verification | ✅ |

**SP1 proof generation cost**: ~30s on modern hardware for a dormancy window computation.
Acceptable for batch/off-peak generation; not suitable for real-time queries.

---

## Phase 8 — Arweave Checkpoint Anchoring

Permanent record of ChronoNode checkpoint roots for historical verifiability. Checkpoints
already exist in SQLite — this phase publishes them to Arweave.

| # | Task | Status |
|---|------|--------|
| 8.1 | Add `ArweaveAnchor` backend to `StorageBackend` trait — wraps Irys SDK HTTP API | ⏳ |
| 8.2 | CLI: `chrononode checkpoint anchor --chain bitcoin --height <n>` — uploads checkpoint JSON + Merkle root to Arweave via Irys | ⏳ |
| 8.3 | Store Arweave TX IDs in SQLite `checkpoint_anchors(chain_id, height, arweave_tx_id)` table | ⏳ |
| 8.4 | REST: `GET /v1/chains/{chain_id}/checkpoints/{height}/anchor` — returns Arweave TX ID for external verification | ⏳ |
| 8.5 | Scheduled anchor: systemd timer `chrononode-anchor@bitcoin.timer` runs weekly | ⏳ |

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
- Phase 8 (Arweave checkpoint anchoring — Irys upload, SQLite anchor store)

Recently completed (2026-05-25):
- **Phase 7 COMPLETE**: SP1 Groth16 zkVM proof mode for trustless dormancy verification
  - SP1 guest program (chrononode-zkvm-program) validates dormancy conditions in RISC-V bytecode
  - CLI command: `chrononode prove --zkvm sp1 --address <addr> [--mock]` generates Groth16 proofs
  - API endpoint: `GET /v1/proofs/{address}/sp1?mock=true` for proof generation
  - `DormancyProof` struct extended with `proof_type`, `zk_proof`, `public_inputs` fields
  - SP1DormancyVerifier contract: validates Groth16 proofs without trusted oracle key
  - RewardDistributor.verifyAndMint(): trustless RESURGE minting (no DORMANCY_ORACLE_ROLE required)
  - Integration tests: SP1 guest program, CLI, API, and full E2E workflow
  - Ready for mainnet audit (eliminates trusted oracle key attack surface)
  - Deployed on Arbitrum Sepolia for testing

- **Live production pipeline (2026-05-24)**: ChronoNode dormancy scan timers (6h) → BaaLS `POST /api/v1/oracle/attest` → EVMSubmitter → `RewardDistributor.submitDormancyProof()` on Arbitrum Sepolia — fully verified ✅
- **`evm_wallet` column + `--evm-wallet` CLI flag** added to `watch add`; required for EVMSubmitter routing
- **28 BTC + 5 DOGE addresses** live on watch list with `evm_wallet` set to deployer `0x42060A5F...`
- **CORS enabled** on ChronoNode HTTP API for browser-facing frontend calls
- **`baals_tls_skip_verify`** config option + CoreConfig serde defaults fixed (was silently dropping attestation config)
- **300ms inter-submission pacing** added to stay under BaaLS rate limiter
- L1. Property-based tests (`proptest`) for Merkle/proof/signing invariants
- L2. Criterion benchmark harness for Merkle root/proof generation/proof verification
- L3. API rate limiter hardened with atomic token-bucket state + deterministic unit tests
- L4. Pagination for list endpoints (`page`/`per_page`) with API coverage
- L5. Adapter config hot reload with test coverage
- L6. Optional MongoDB/Scylla index backend feature paths now safely resolve via SQLite compatibility fallback
- L7. Grafana dashboard JSON in `deploy/grafana/chrononode-dashboard.json`
