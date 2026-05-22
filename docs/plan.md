# ChronoNode v2 Implementation Plan

**Status:** Current — updated May 21, 2026  
**Tests:** 51 passing (10 core + 3 adapter + 4 archival + 3 storage + 5 BaaLS + 16 API + 4 checkpoint + 6 storage backends + 3 mock adapter)  
**Build:** Clean with and without `--features postgres`  
**Clippy:** Zero warnings across workspace  
**Rust:** 1.95.0 (current stable)

---

## 1. Executive Summary

ChronoNode is a verifiable archival layer for blockchain history. The v2 pivot replaced ZK-SNARKs/token/consensus with Merkle proofs + decentralized storage. The codebase is a working Rust workspace (`chrononode-core`, `chrononode-cli`, `chrononode-adapter-*`) with a monolithic CLI that handles ingestion, archival, indexing, proof generation, and an HTTP API server.

This plan catalogues every remaining gap — ordered from highest to lowest priority — based on a full spec-vs-code comparison against `ChronoNode_v2_Efficiency_Spec.md`, `ChronoNodeTB.md`, `ChronoNodePB.md`, `proof-model.md`, `operations.md`, and the original `chrononode-plan.md`.

---

## 2. What Is Already Implemented (Code-Verified)

### 2.1 Core

| Component | Location | Detail |
|---|---|---|
| Ed25519 keypair | `core/src/signing.rs` | Generate, sign, verify, file roundtrip, Unix perms |
| Merkle proofs | `core/src/proof.rs` | Domain-separated leaf hash, generate/verify, odd-size trees |
| Protobuf schema | `proto/chrononode.proto` + `build.rs` | Serialize/deserialize via prost 0.14 |
| `StoragePointer` | `core/src/chain.rs` | `backend:key` format, `from_string` parser |
| `ChainAdapter` trait | `core/src/chain.rs` | `latest_height`, `fetch_block`, `fetch_block_by_hash` |
| `StorageBackend` trait | `core/src/chain.rs` | `put`, `get`, `pin`, `health_check` |
| `MerkleLeaf` | `core/src/proof.rs` | Public, re-exported for benchmarks |

### 2.2 Storage Backends

| Backend | Env Var | Key Format | `put` | `get` (verifies) | `health_check` |
|---|---|---|---|---|---|
| `local_fs` | `CHRONONODE_STORAGE_BACKEND=local_fs` | SHA-256 hex as filename | Yes | Yes | Yes |
| `ipfs` | `CHRONONODE_IPFS_API_URL` | `{sha256}:{cid}` | Yes | Yes | Yes |
| `pinata` | `CHRONONODE_PINATA_BASE`, `_GATEWAY_BASE`, `_JWT` | `{sha256}:{cid}` | Yes | Yes | Yes |
| `arweave` | `CHRONONODE_ARWEAVE_GATEWAY`, `_BUNDLER` | `{sha256}:{txid}` | Yes (via bundler) | Yes | Yes |
| `s3` | `CHRONONODE_S3_BUCKET`, `_REGION`, `_ENDPOINT` | SHA-256 hex under `chrononode/` | Yes | Yes | Yes |

### 2.3 Index Backends

| Backend | Selection | Detail |
|---|---|---|
| `SqliteIndex` | Default / `CHRONONODE_INDEX_BACKEND=sqlite` | All tables, indexes, atomic transactions, reorg detection |
| `PostgresIndex` | `--features postgres` / `CHRONONODE_INDEX_BACKEND=postgres` | Full schema parity, ON CONFLICT upserts, `CHRONONODE_POSTGRES_URL` |

### 2.4 Chain Adapters

| Adapter | Crate | Detail |
|---|---|---|
| `mock` | `chrononode-adapter-mock` | Deterministic mock chain for testing |
| `baals` | `chrononode-adapter-baals` | BaaLS REST API (`/api/v1/chain/head`, `/api/v1/blocks/{index}`, `/api/v1/blocks/by_hash/{hash}`) |
| `localfile` | `chrononode-adapter-localfile` | Import JSON block dumps from filesystem |

### 2.5 Signing & Trust

- Ed25519 keypair generated at `chrononode init` (`main.rs:96-100`)
- Operator key loaded from `CHRONONODE_OPERATOR_KEY` env var or `operator_key` file
- Checkpoint roots signed via `CheckpointBuilder::with_keypair()`
- `signer_pubkey` and `signature` stored in `merkle_checkpoints` table
- Proof JSON includes signature fields; `verify_proof_json` validates signatures

### 2.6 HTTP API Endpoints (`crates/chrononode-cli/src/api/http.rs`)

| Endpoint | Method | Status |
|---|---|---|
| `/health` | GET | Implemented |
| `/v1/chains` | GET | Implemented |
| `/v1/chains/{id}/blocks/{height}` | GET | Implemented |
| `/v1/chains/{id}/blocks/hash/{hash}` | GET | Implemented |
| `/v1/chains/{id}/blocks?from=&to=&format=ndjson` | GET | Implemented |
| `/v1/chains/{id}/proofs/block/{height}` | GET | Implemented |
| `/v1/checkpoints/{id}` | GET | Implemented |
| `/v1/chains/{id}/checkpoints` | POST | Implemented |
| `/v1/proofs/verify` | POST | Implemented |
| `/v1/chains/{id}/txs/sender/{sender}?limit=` | GET | Implemented |
| `/v1/chains/{id}/txs/recipient/{recipient}?limit=` | GET | Implemented |
| `/v1/chains/{id}/events/{event_type}?limit=` | GET | Implemented |
| `/metrics` | GET | Implemented (real prometheus counters via `install_recorder()`) |
| `/api-docs` | GET | Swagger UI (utoipa) |
| `/api-docs/openapi.json` | GET | OpenAPI 3.0 spec |

### 2.7 CLI Commands

| Command | Status |
|---|---|
| `init` | Implemented |
| `config show` | Implemented |
| `ingest --chain --from [--follow] [--force]` | Implemented |
| `query block --chain --height` | Implemented |
| `query txs-by-sender --chain --sender [--limit]` | Implemented |
| `query txs-by-recipient --chain --recipient [--limit]` | Implemented |
| `query events-by-type --chain --event-type [--limit]` | Implemented |
| `prove --chain --height [--out]` | Implemented |
| `verify <proof.json>` | Implemented |
| `repair --chain --height` | Implemented |
| `verify-archive --chain --from --to` | Implemented |
| `backup --chain --out` | Implemented |
| `restore --chain --from` | Implemented |
| `stats --chain` | Implemented |
| `adapters` | Implemented |
| `checkpoint create --chain --from --to` | Implemented |
| `checkpoint anchor --chain --id --tx-hash` | Implemented |
| `export-checkpoint --id [--out]` | Implemented |
| `serve --port --chain [--api-key] [--rate-limit]` | Implemented |

### 2.8 Tests (51 total)

| Suite | File | Count |
|---|---|---|
| Core (proof + signing) | `chrononode-core/src/proof.rs`, `signing.rs` | 10 |
| Mock adapter contract | `tests/adapter_contract.rs` | 3 |
| Archival flow | `tests/archival_flow.rs` | 4 |
| Storage backends | `tests/storage_backends.rs` | 6 |
| BaaLS adapter | `baals_adapter_contract.rs` | 5 |
| API endpoints | `tests/api_endpoints.rs` | 16 |
| Checkpoint commands | `tests/checkpoint_commands.rs` | 4 |
| Benchmarks | `benches/benchmarks.rs` | Manual run |

### 2.9 Infrastructure

| Component | Status |
|---|---|
| Dockerfile (multi-stage, cached) | Implemented |
| docker-compose.yml (ingest + api + postgres + ipfs) | Implemented |
| `.dockerignore` | Implemented |
| `.env.example` | Implemented |
| systemd templates (`deploy/systemd/`) | Implemented |
| Kubernetes manifests (`deploy/k8s/`) | Implemented |
| GitHub Actions CI (fmt + clippy + tests) | Implemented |
| Performance benchmarks | Implemented |

### 2.10 Dependency Audit

| Crate | Version | Notes |
|---|---|---|
| `thiserror` | 2 | Upgraded from 1 |
| `reqwest` | 0.13 | Upgraded from 0.12 |
| `prost` | 0.14 | Upgraded from 0.13 |
| `object_store` | 0.13 | Upgraded from 0.11 |
| `metrics` | 0.24 | Upgraded from 0.21 |
| `metrics-exporter-prometheus` | 0.17 | Upgraded from 0.12 |
| `sqlx` | 0.8 | Held at 0.8 (0.9 is alpha) |
| `ed25519-dalek` | 2 | Held at 2 (3 is pre-release) |

---

## 3. Remaining Work — By Priority

All HIGH and MEDIUM priority items have been completed. Remaining LOW priority items:

### LOW PRIORITY — Future / Deferred

These items are explicitly deferred or are nice-to-haves.

#### L1. Property-based tests (proptest)

Add `proptest` to `chrononode-core` dev-dependencies. Write property tests for Merkle tree roundtrip, serialization/deserialization idempotency, and signing verification with random keypairs.

**Effort:** ~3 hours

---

#### L2. Criterion benchmarks

Add `criterion` to `chrononode-core` dev-dependencies. Benchmark Merkle tree construction (1k, 10k, 100k leaves), proof generation, and proof verification.

**Effort:** ~2 hours

---

#### L3. Rate limiter improvements

Replace `Arc<Mutex<Instant>>` with atomic operations (use `tokio::time::Instant` + `AtomicU64` for leaky-bucket or token-bucket algorithm). Add tests.

**Files:** `crates/chrononode-cli/src/api/http.rs:30-65`

**Effort:** ~1 hour

---

#### L4. Pagination for list endpoints

Add `?page=&per_page=` to `/v1/chains`, `/v1/chains/{id}/txs/sender/{sender}`, and `/v1/chains/{id}/events/{event_type}`.

**Effort:** ~1 hour

---

#### L5. Adapter config hot reload

Watch `config.toml` for changes with `notify` crate. Reload adapter configs without restart.

**Effort:** ~3 hours

---

#### L6. MongoDB / Cassandra index backends

Add optional `mongodb` and `scylla` features to Cargo.toml. Implement `IndexBackend` trait for each.

**Effort:** ~8 hours

---

#### L7. Grafana dashboard

Create `deploy/grafana/chrononode-dashboard.json` with panels for ingest rate, archive depth, storage latency, checkpoint frequency, and error rate.

**Effort:** ~2 hours

---

### EXPLICITLY DEFERRED — Not in v2 Scope

These items from v1.x specs (TB.md, PB.md) are deliberately removed or deferred to post-v2:

| Item | Reason |
|---|---|
| ZK-SNARK proofs (`zkp-prover-svc`) | Requires R&D into proof systems; Merkle proofs suffice for v2 |
| $CHN token, staking, slashing, DAO | Economic model removed from v2 |
| Multi-CAC cross-signing / federated verification | Requires network of operators (Phase 3) |
| GraphQL, JSON-RPC, gRPC APIs | REST + ndjson streaming meet current needs |
| Message queues (Kafka/RabbitMQ/NATS) | Not needed for single-node operation |
| Distributed tracing (OpenTelemetry, Jaeger) | Overhead exceeds budget; Prometheus metrics suffice |
| Cross-chain messaging (LayerZero, Wormhole, IBC) | Requires 3+ adapters first |
| Hardware acceleration (GPU/FPGA/ASIC) | Not relevant for SHA-256 Merkle proofs |
| Bitcoin / Ethereum / Vigil adapters | Phase F — requires extensive RPC integration work |
| WASM verification module | Nice-to-have for client-side trust |
| Full-text search on events/txs | Requires FTS5 extension (SQLite) or tsvector (Postgres) |
| Time-series aggregations | Requires analytics use case definition |
| ChronoNode Explorer DApp | Requires frontend development |
| Operator staking/slashing | Economic layer removed from v2 |
| RocksDB / LMDB embedded databases | SQLite + PostgreSQL cover the use cases |
| Self-hosted IPFS node (libp2p) | HTTP client to existing IPFS node is simpler and cheaper |
| Garbage collection for orphaned storage objects | Low risk until significant reorgs occur |

---

## 4. Suggested Execution Order

```
Completed: H1 → H2 → H3 → H4 → M7 → M1 → M2 → M3 → M5 → M6 → M4
Future:    L1 → L2 → L3 → L4 → L5 → L6 → L7
```

---

## 5. Key Design Decisions Documented

- **Ed25519** for checkpoint signing per v2 spec (not ECDSA/secp256k1 for Bitcoin compatibility)
- **SHA-256** Merkle tree (not Keccak/Blake3) — universal compatibility
- **Domain-separated, length-prefixed leaf hashes** — prevents second-preimage attacks
- **SQLite** as default index — zero-config, embedded, sufficient for solo-dev
- **PostgreSQL** as feature-gated alternative — for rich queries, not forced on all users
- **Content-addressed storage keys** (`sha256_hex:provider_id`) — self-verifying pointers
- **Feature-gated PostgreSQL** (`--features postgres`) — keeps default build lean
- **Monolithic CLI** — replaces v1.x microservices architecture for solo-dev viability
- **Protobuf serialization** — `prost` for cross-language schemas
- **Environment variables** for all configuration — 12-factor app style

---

## 6. Budget Alignment

All decisions align with the $25–50/mo budget target:
- SQLite requires no separate database server
- `local_fs` storage costs $0 beyond VPS disk
- Pinata free tier covers up to 1 GB pinned
- Arweave bundler (Irys) pay-per-upload with perma-storage
- S3 falls within AWS free tier for first 5 GB
- Monolithic binary needs only one VPS ($5–10/mo on Hetzner/DO)
- No message broker, no distributed tracing infrastructure needed

---

## 7. Key Files Reference

| File | Purpose |
|---|---|
| `crates/chrononode-core/src/proof.rs` | Merkle leaf/tree/proof logic |
| `crates/chrononode-core/src/signing.rs` | Ed25519 keypair and signature verification |
| `crates/chrononode-core/src/chain.rs` | `ChainAdapter`, `StorageBackend` traits, `StoragePointer` |
| `crates/chrononode-core/src/error.rs` | `CoreError` enum with thiserror 2 |
| `crates/chrononode-cli/src/main.rs` | CLI entry point, command handlers |
| `crates/chrononode-cli/src/api/http.rs` | Axum HTTP API server, all endpoints, Swagger UI |
| `crates/chrononode-cli/src/archive/pipeline.rs` | Archive pipeline (fetch → serialize → store → index) |
| `crates/chrononode-cli/src/archive/serializer.rs` | Protobuf serialize/deserialize |
| `crates/chrononode-cli/src/index/sqlite.rs` | SQLite index implementation |
| `crates/chrononode-cli/src/index/postgres.rs` | PostgreSQL index implementation |
| `crates/chrononode-cli/src/index/mod.rs` | `IndexBackend` trait, `IndexKind`, `open_index` factory |
| `crates/chrononode-cli/src/storage/mod.rs` | `BackendKind`, `BackendConfig`, `create_backend` factory |
| `crates/chrononode-cli/src/storage/arweave.rs` | Arweave backend (gateway + bundler) |
| `crates/chrononode-cli/src/storage/s3.rs` | S3 backend (object_store crate) |
| `crates/chrononode-cli/src/storage/ipfs.rs` | IPFS backend (reqwest 0.13) |
| `crates/chrononode-cli/src/storage/pinata.rs` | Pinata backend |
| `crates/chrononode-cli/src/storage/local_fs.rs` | Local filesystem backend |
| `crates/chrononode-cli/src/storage/fallback.rs` | Primary/secondary storage fallback |
| `crates/chrononode-cli/src/archive/cache.rs` | Moka-based content-addressable cache |
| `crates/chrononode-cli/src/cli/mod.rs` | Clap CLI definitions |
| `crates/chrononode-cli/src/metrics.rs` | Prometheus counter definitions |
| `crates/chrononode-adapter-baals/src/lib.rs` | BaaLS REST adapter |
| `crates/chrononode-adapter-mock/src/lib.rs` | Mock chain adapter |
| `crates/chrononode-adapter-localfile/src/lib.rs` | Local file import adapter |
| `crates/chrononode-adapter-sdk/src/registry.rs` | Adapter registry |
| `crates/chrononode-adapter-sdk/src/retry.rs` | Retry with backoff utilities |
| `benches/benchmarks.rs` | Performance benchmarks |
| `deploy/systemd/` | systemd service templates |
| `deploy/k8s/` | Kubernetes manifests (configmap, deployments, service, PVC) |
| `.github/workflows/ci.yml` | GitHub Actions CI |
| `docs/*.md` | Specification documents |
