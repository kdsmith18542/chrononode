# ChronoNode `plan.md` — Independent Verifiable Archival Layer

**Project:** ChronoNode  
**Version:** v2.1 Planning Draft  
**Status:** Greenfield / Early Build Plan  
**Primary Goal:** Build a solo-dev viable archival, indexing, and proof layer that can support BaaLS first, then other networks through adapters.  
**Core Positioning:** ChronoNode is an independent verifiable history layer for app-specific ledgers, local-first chains, and eventually public networks.

---

## 1. Executive Summary

ChronoNode should be built as a **separate infrastructure project**, not as a BaaLS submodule. BaaLS should be the first reference integration because it is under your control, easier to adapt, and ideal for proving the architecture. But ChronoNode should remain network-agnostic from day one.

The project should start with the realistic v2 pivot:

- No native token.
- No ChronoNode consensus layer.
- No DAO.
- No staking/slashing.
- No ZK-SNARKs in the MVP.
- No multi-chain complexity on day one.
- No requirement to host full chain history locally.

Instead, ChronoNode v2 should archive chain data to content-addressed storage, maintain a lightweight local index, and serve historical queries with verifiable Merkle proofs.

The first successful demo should be:

```text
BaaLS block → ChronoBlock protobuf → IPFS CID → SQLite index → query by height → Merkle proof → client verification
```

The long-term product should be:

```text
Any supported network → ChainAdapter → ChronoNode archive/index/proof API → apps, explorers, debuggers, or auditors
```

---

## 2. Product Identity

### 2.1 Recommended Positioning

ChronoNode should be positioned as:

> **An independent verifiable archival layer for blockchain and app-ledger history.**

Alternative shorter pitch:

> **Archive once. Query forever. Verify every response.**

Do not position it only as a BaaLS add-on. BaaLS is the first adapter, not the whole identity.

### 2.2 What ChronoNode Is

ChronoNode is:

- A chain archival client framework.
- A content-addressed historical block/event archive.
- A compact CID metadata index.
- A Merkle proof generator.
- A historical query API.
- A future multi-network adapter platform.
- A useful backend for explorers, debuggers, compliance tooling, and local-first apps.

### 2.3 What ChronoNode Is Not Yet

ChronoNode v2 should **not** be:

- A new blockchain.
- A token network.
- A staking protocol.
- A DAO.
- A consensus layer.
- A ZK-proof network.
- A decentralized storage network.
- A replacement for source chain nodes.
- A full Bitcoin/Ethereum/Solana indexer at launch.

### 2.4 Existing Codebase Note

The `chrononode_archival_client/` directory contains the v1 implementation (full Bitcoin/Ethereum/Solana sync clients, RocksDB, GraphQL, AMQP event bus, ZK-proof stubs). This code represents the old vision and is **not** the starting point for v2.1. It should be preserved on a legacy branch and referenced only for reusable patterns (`errors.rs` error types, protobuf build setup). The new `src/` tree as described in Section 4 should be built from scratch.

---

## 3. Strategic Relationship With BaaLS

ChronoNode should remain separate, but BaaLS is the best first integration.

### 3.1 Why BaaLS First

BaaLS is ideal as the reference adapter because:

- You control both projects.
- The block model is simpler than public L1s.
- The JSON-RPC interface can be shaped to support archival needs.
- Contract execution history from BaaLS gives ChronoNode useful data.
- CanvasContracts can use ChronoNode for contract history, proofs, and time-travel debugging.

### 3.2 Relationship Model

```text
BaaLS
  produces blocks, events, contract execution data

ChronoNode
  archives, indexes, proves, and serves that data

CanvasContracts
  consumes ChronoNode history for debugging, proof display, and contract timeline views
```

### 3.3 Independence Rule

ChronoNode must not import BaaLS internals directly in core crates.

Use adapters:

```text
chrononode-core          # network-agnostic models, proofs, archive logic
chrononode-adapter-baals # BaaLS JSON-RPC ingestion
chrononode-adapter-vigil # future adapter
chrononode-adapter-bitcoin # future adapter
chrononode-adapter-ethereum # future adapter
```

This keeps ChronoNode reusable with other networks.

---

## 4. Core Architecture

Start as a **modular monolith**. Do not split into microservices until the core loop works.

### 4.1 Recommended Repository Layout

Use a **Cargo workspace from day one** to enforce adapter independence. The `chrononode-core` crate must have zero adapter dependencies, which the compiler enforces when they live in separate crates.

```text
chrononode/
├── Cargo.toml            # workspace root
├── crates/
│   ├── chrononode-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── block.rs
│   │       ├── tx.rs
│   │       ├── chain.rs
│   │       ├── proof.rs
│   │       ├── error.rs
│   │       └── config.rs
│   └── chrononode-cli/
│       ├── Cargo.toml    # depends on core + adapters
│       └── src/
│           ├── main.rs
│           ├── cli/
│           │   └── mod.rs
│           ├── adapters/
│           │   ├── mod.rs
│           │   ├── baals.rs
│           │   └── mock.rs
│           ├── archive/
│           │   ├── mod.rs
│           │   ├── serializer.rs
│           │   └── pipeline.rs
│           ├── storage/
│           │   ├── mod.rs
│           │   ├── local_fs.rs
│           │   ├── ipfs.rs
│           │   └── pinata.rs
│           ├── index/
│           │   ├── mod.rs
│           │   └── sqlite.rs
│           ├── verification/
│           │   ├── mod.rs
│           │   ├── merkle.rs
│           │   └── checkpoint.rs
│           └── api/
│               ├── mod.rs
│               └── http.rs
├── proto/
│   └── chrononode.proto
├── tests/
│   ├── archival_flow.rs
│   ├── proof_verification.rs
│   └── adapter_contract.rs
├── docker-compose.yml
├── Dockerfile
└── docs/
    ├── architecture.md
    ├── adapter-authoring.md
    ├── proof-model.md
    ├── baals-integration.md
    └── operations.md
```

Key decisions:
- `chrononode-core` is a library crate with traits, models, proof logic, and error types — no adapter code, no I/O beyond trait definitions.
- `chrononode-cli` is the binary crate that pulls in adapters, storage backends, the index, and the API server.
- The `dsn/` module is renamed to `storage/` — the MVP uses local filesystem and IPFS, neither of which is meaningfully decentralized storage at this stage.
- A `Dockerfile` and `docker-compose.yml` are added to the root from Phase 1 onward so new contributors can run `docker compose up` and get BaaLS + ChronoNode + IPFS.

### 4.2 Future Crate Splitting

When the project outgrows the two-crate workspace, split further:

```text
crates/
├── chrononode-core
├── chrononode-adapter-sdk
├── chrononode-adapter-baals
├── chrononode-storage
├── chrononode-index
├── chrononode-api
└── chrononode-cli
```

Do this only when load or compile times justify it.

---

## 5. Adapter-First Design

ChronoNode’s long-term success depends on a clean adapter interface.

### 5.1 ChainAdapter Trait

```rust
#[async_trait::async_trait]
pub trait ChainAdapter: Send + Sync {
    fn chain_id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn block_model(&self) -> BlockModel;

    /// Fetch a canonical ChronoBlock by height (adapter handles conversion internally).
    async fn fetch_block(&self, height: u64) -> Result<ChronoBlock>;

    /// Fetch by chain-native block hash.
    async fn fetch_block_by_hash(&self, hash: &[u8]) -> Result<ChronoBlock>;

    /// Batch-fetch a contiguous range. Adapters should optimize this (e.g. single
    /// batch RPC call) rather than issuing N individual requests.
    async fn fetch_range(&self, from: u64, to: u64) -> Result<Vec<ChronoBlock>> {
        let mut blocks = Vec::with_capacity((to - from + 1) as usize);
        for h in from..=to {
            blocks.push(self.fetch_block(h).await?);
        }
        Ok(blocks)
    }

    /// Latest confirmed block height known to the source chain.
    async fn latest_height(&self) -> Result<u64>;
}
```

Design notes:
- A single `fetch_block` replaces the old `fetch_block_by_height` + `normalize_block` two-step. The adapter owns conversion from its native type to `ChronoBlock`.
- `fetch_range` has a default iterative implementation but adapters can override it for efficiency.
- No `SourceBlock` is exposed on the trait surface — it would create a leaky abstraction since every chain's raw block format is different.

### 5.2 Block Models

```rust
pub enum BlockModel {
    Utxo,
    Account,
    EventLedger,
    Custom(String),
}
```

### 5.3 First Adapters

| Phase | Adapter | Why |
|---|---|---|
| v0.1 | Mock adapter | Makes tests deterministic |
| v0.1 | Local file/import adapter | Simplest real adapter; no external service required. Import exported JSON/CSV block files |
| v0.1 | BaaLS adapter | First real chain integration |
| v0.3 | Vigil adapter | Good second network if it remains aligned with your ecosystem |
| v0.4+ | Bitcoin adapter | High-value but more complex |
| v0.4+ | Ethereum adapter | High demand but heavy state/query complexity |

Local file adapter moved before BaaLS: it requires no external RPC endpoint and makes the archival loop testable end-to-end without BaaLS running. If BaaLS exposes a Rust crate (not just JSON-RPC), a native adapter is preferable for latency, but JSON-RPC is fine for the first integration.

### 5.4 Adapter Authoring Guide

`docs/adapter-authoring.md` should cover:

1. Implementing `ChainAdapter` trait — required methods and contracts.
2. Block normalization — converting chain-native blocks to `ChronoBlock`.
3. Handle chain-specific quirks (reorgs, empty blocks, no-event chains).
4. Writing deterministic tests with the mock adapter.
5. Passing the adapter contract test suite (`tests/adapter_contract.rs`).
6. Example: walk through building the BaaLS adapter.

---

## 6. Canonical Data Model

ChronoNode should use a network-neutral model. Each adapter converts source-chain blocks into ChronoBlock.

### 6.1 Protobuf Schema

```protobuf
syntax = "proto3";

package chrononode.v1;

message ChronoBlock {
  uint32 schema_version = 1;    // for protobuf evolution (currently 1)
  string chain_id = 2;
  uint64 height = 3;
  bytes block_hash = 4;
  bytes prev_hash = 5;
  uint64 timestamp = 6;
  string block_model = 7;
  string hash_algorithm = 8;    // "sha256d", "keccak256", etc.
  repeated ChronoTx transactions = 9;
  repeated ChronoEvent events = 10;
  bytes extra_data = 11;
}

message ChronoTx {
  bytes tx_hash = 1;
  bytes sender = 2;
  bytes recipient = 3;
  uint64 amount = 4;
  uint64 nonce = 5;
  bytes payload = 6;
  uint64 gas_limit = 7;
  uint64 gas_used = 8;
  bytes extra_data = 9;
}

message ChronoEvent {
  string event_type = 1;
  bytes emitter = 2;
  uint64 tx_index = 3;
  uint64 event_index = 4;
  bytes payload = 5;
}
```

### 6.2 Why Include Events

BaaLS and CanvasContracts will likely generate useful contract/workflow events. Events are also helpful for app-ledger use cases, audit logs, and future explorer/debugger features.

For UTXO chains that do not have native events, adapters can leave `events` empty.

---

## 7. Storage Strategy

### 7.1 Storage Philosophy

ChronoNode should not store full chain history locally by default.

It should store:

```text
local SQLite index
checkpoint roots
proof metadata
small cache
CIDs / storage pointers
```

Raw blocks should live in:

```text
IPFS
Pinata/IPFS pinning service
local filesystem backend for development
Arweave for important permanent checkpoints later
```

### 7.2 StorageBackend Trait

```rust
#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    /// Store bytes, return a content-addressed pointer (CID).
    async fn put(&self, bytes: &[u8]) -> Result<StoragePointer>;

    /// Retrieve full object by pointer. Must verify content integrity internally:
    /// recalculate hash/key after retrieval and compare with the pointer.
    async fn get(&self, pointer: &StoragePointer) -> Result<Vec<u8>>;

    /// Pin an object to prevent garbage collection (IPFS-backend specific).
    async fn pin(&self, pointer: &StoragePointer) -> Result<()>;

    /// Check backend health.
    async fn health_check(&self) -> Result<StorageHealth>;
}
```

Content verification inside `get` is mandatory: an IPFS gateway could return wrong bytes for a valid CID. The backend must compute `hash(retrieved_bytes)` and fail if it doesn't match the pointer. See Section 21 for the full verification model.

### 7.3 v0.1 Backends

Implement these first:

```text
local_fs
ipfs
pinata
```

Start with `local_fs` so the project can be tested without external services.

### 7.4 Defer

Defer these until the MVP works:

```text
Arweave
Filecoin
Storj
S3
custom DSN federation
```

---

## 8. SQLite Index Schema

Use SQLite first. It is simple, portable, cheap, and good enough for the early product.

```sql
CREATE TABLE chains (
    chain_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    adapter_type TEXT NOT NULL,
    block_model TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE archived_blocks (
    chain_id TEXT NOT NULL,
    height INTEGER NOT NULL,
    block_hash BLOB NOT NULL,
    block_hash_hex TEXT NOT NULL,
    prev_hash BLOB,
    storage_backend TEXT NOT NULL,
    storage_pointer TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    byte_size INTEGER NOT NULL,
    checkpoint_id TEXT,
    archived_at INTEGER NOT NULL,
    pin_status TEXT NOT NULL DEFAULT 'pending',
    degraded INTEGER NOT NULL DEFAULT 0,
    reorg_detected INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (chain_id, height)
);

CREATE UNIQUE INDEX idx_archived_blocks_hash
ON archived_blocks(chain_id, block_hash_hex);

CREATE TABLE ingest_state (
    chain_id TEXT PRIMARY KEY,
    latest_archived_height INTEGER NOT NULL DEFAULT -1,
    latest_checked_height INTEGER NOT NULL DEFAULT -1,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (chain_id) REFERENCES chains(chain_id)
);

CREATE TABLE merkle_checkpoints (
    checkpoint_id TEXT PRIMARY KEY,
    chain_id TEXT NOT NULL,
    start_height INTEGER NOT NULL,
    end_height INTEGER NOT NULL,
    root_hash BLOB NOT NULL,
    signer_pubkey BLOB,
    signature BLOB,
    anchored_chain_id TEXT,
    anchored_tx_hash BLOB,
    created_at INTEGER NOT NULL
);

CREATE TABLE storage_objects (
    storage_pointer TEXT PRIMARY KEY,
    storage_backend TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    pinned INTEGER NOT NULL DEFAULT 0,
    last_verified_at INTEGER,
    degraded INTEGER NOT NULL DEFAULT 0
);
```

Design notes:
- `block_hash_hex` is a denormalized hex column used for indexed lookups. SQLite BLOB indices degrade at scale; hex text comparison is faster for the unique index.
- `ingest_state` tracks resume position across `--follow` restarts and enables idempotent ingestion.
- `reorg_detected` (default 0) is a minimal reorg flag. Full reorg handling is deferred, but the column exists so the pipeline can record that a block at a given height was replaced. The ingest loop should: fetch block N, compare hash with stored `block_hash`, and if different, mark old row as `degraded=1` and insert the new row (via INSERT OR REPLACE or UPSERT).
- Table renamed from `checkpoints` to `merkle_checkpoints` to avoid conflict with SQL reserved words.

Use PostgreSQL later only for richer hosted deployments or high-volume structured queries.

---

## 9. Merkle Proof Model

### 9.1 Leaf Format

Use a deterministic, domain-separated leaf hash with length-prefixed fields to prevent ambiguity:

```text
leaf = sha256(
    len(tag)         as u16 BE || tag           ||  // "chrononode:v1:block" (length-prefixed)
    len(chain_id)    as u16 BE || chain_id      ||  // variable-length, length-prefixed
    height           as u64 BE                  ||  // fixed 8 bytes
    block_hash       as [u8; 32]                ||  // fixed 32 bytes
    len(storage_backend) as u16 BE || storage_backend ||  // variable-length
    len(storage_pointer)  as u16 BE || storage_pointer     // variable-length
)
```

Length-prefixed variable fields prevent the ambiguity where e.g. `"chain1" || "height123"` could collide with `"chain" || "1height123"`. Fixed-width fields (height as u64, block_hash as exactly 32 bytes) are always predictable.

Alternative: serialize a leaf protobuf struct and hash its canonical bytes — more self-documenting but introduces a proto dependency into the verifier.

### 9.2 Checkpoint Size

Default:

```text
checkpoint_size = 1,000 blocks
```

Make it configurable:

```toml
[verification]
checkpoint_size = 1000
hash_algorithm = "sha256"
```

### 9.3 Proof Response Format

```json
{
  "version": "chrononode-proof-v1",
  "chain_id": "baals",
  "height": 500,
  "block_hash": "0x...",
  "storage_backend": "ipfs",
  "storage_pointer": "bafy...",
  "checkpoint": {
    "checkpoint_id": "baals-0-999",
    "start_height": 0,
    "end_height": 999,
    "root": "0x...",
    "signer_pubkey": "0x...",
    "signature": "0x...",
    "anchored_chain_id": null,
    "anchored_tx_hash": null
  },
  "proof": [
    { "position": "left", "hash": "0x..." },
    { "position": "right", "hash": "0x..." }
  ]
}
```

### 9.4 Trust Model

v0.1 trust model:

```text
Client trusts a configured ChronoNode public key.
ChronoNode signs checkpoint roots.
Client verifies:
  1. Block hash matches returned block bytes.
  2. Leaf hash includes block hash and storage pointer.
  3. Leaf is included in checkpoint root.
  4. Checkpoint root is signed by a trusted ChronoNode key.
```

### 9.5 Optional Anchoring

Later, ChronoNode can anchor checkpoint roots into BaaLS or another supported chain.

```text
ChronoNode checkpoint root → anchor transaction/event → external verification
```

Do not require anchoring for v0.1.

---

## 10. Repair and Availability Strategy

Content-addressed storage still needs operational repair.

Required repair flow:

```text
If storage retrieval fails:
1. Try primary backend.
2. Try alternate configured gateway.
3. Try local cache.
4. Determine repair policy for this chain/height:
   - RECOVERABLE: Source chain can re-serve the block → re-fetch through adapter, re-upload, re-pin.
   - MIRROR-ONLY: Source chain has pruned the block → try alternate IPFS gateways, Pinata alternatives.
   - PERMANENTLY LOST: No source and no retrievable mirror → mark block unavailable, surface in /health.
5. Re-upload and re-pin object if recovered.
6. Mark old pointer degraded if repair succeeds.
7. Mark block unavailable if all repair attempts fail.
```

The `verify-archive` command should proactively identify blocks in the MIRROR-ONLY category *before* degradation occurs, so operators have time to create redundant copies.

Add CLI:

```bash
chrononode repair --chain baals --height 500
chrononode verify-archive --chain baals --from 0 --to 1000
```

The `repair` command follows the policy levels:
1. `RECOVERABLE` blocks: re-fetch from source via adapter, re-upload, re-pin.
2. `MIRROR-ONLY` blocks: try alternate IPFS gateways, Pinata, or local cache.
3. `PERMANENTLY LOST` blocks: mark as unavailable; surface in `/health` and `verify-archive` output.

This is important for credibility. A CID index is only valuable if degraded objects can be detected and repaired.

---

## 11. API Plan

### 11.1 MVP HTTP API

Start with REST/JSON over Axum.

```text
GET /health
GET /v1/chains
GET /v1/chains/{chain_id}/blocks/{height}
GET /v1/chains/{chain_id}/blocks?from={from}&to={to}
GET /v1/chains/{chain_id}/blocks/hash/{hash}
GET /v1/chains/{chain_id}/proofs/block/{height}
POST /v1/proofs/verify
GET /v1/checkpoints/{checkpoint_id}
```

The range endpoint (`?from=&to=`) returns ndjson or paginated JSON. This avoids N+1 request patterns when consumers need a historical window (checkpoint rebuilding, canvas timeline views, etc.).

### 11.2 Delay GraphQL and gRPC

GraphQL and gRPC are good later, but REST is enough for v0.1.

Add GraphQL when:

```text
block archival works
proofs work
transaction/event indexing exists
CanvasContracts needs flexible historical queries
```

Add gRPC when:

```text
multiple services exist
high-throughput internal calls are needed
SDKs are being built
```

---

## 12. CLI Plan

The CLI is critical for solo-dev usability.

Required commands:

```bash
chrononode init
chrononode config show
chrononode ingest --chain baals --from 0
chrononode ingest --chain baals --follow
chrononode query block --chain baals --height 500
chrononode prove block --chain baals --height 500 --out proof.json
chrononode verify proof.json
chrononode repair --chain baals --height 500
chrononode verify-archive --chain baals --from 0 --to 1000
chrononode serve --port 8080
```

All ingest commands are **idempotent by default**: they check `ingest_state` and `archived_blocks` before fetching. Use `--force` to re-archive already-indexed blocks. Running `ingest` twice is a no-op.

Nice-to-have later:

```bash
chrononode adapter list
chrononode checkpoint create --chain baals --from 0 --to 999
chrononode archive stats
chrononode export checkpoint --id baals-0-999
```

---

## 13. Suggested Rust Stack

### 13.1 Core

```toml
tokio = { version = "1", features = ["full"] }
anyhow = "1"
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

### 13.2 Storage and Indexing

```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "macros", "migrate"] }
prost = "0.13"
prost-build = "0.13"
sha2 = "0.10"
hex = "0.4"
```

### 13.3 API

```toml
axum = "0.8"
tower = "0.5"
tower-http = "0.6"
utoipa = "5"
```

### 13.4 Proofs and Crypto

```toml
ed25519-dalek = "2"
rand = "0.8"
```

Use `rs_merkle` if it fits, but writing a small deterministic Merkle module is also reasonable for v0.1.

### 13.5 IPFS / HTTP

```toml
reqwest = { version = "0.12", features = ["json", "multipart"] }
```

Prefer Pinata/IPFS HTTP APIs first. Native libp2p/IPFS node integration can come later.

### 13.6 Important Note

Start with a **fresh `Cargo.toml`** for the new workspace. The existing `chrononode_archival_client/Cargo.toml` has incompatible dependencies (`rusqlite` instead of `sqlx`, `prometheus` instead of `tower`-based metrics, `lapin`/AMQP, `rocksdb`, `log`+`env_logger` instead of `tracing`). Do not merge old and new dependencies — extract only what's reusable and rebuild.

Consider also:
- `object_store` crate for future S3-compatible backends (cleaner than writing S3 HTTP from scratch).
- `cargo-make` or `just` for task automation (build/publish all crates, run integration tests).

---

## 14. Development Roadmap

## Phase 0 — Repository Cleanup and Direction Lock (2–3 days)

Deliverables:

- Quarantine old `chrononode_archival_client/` code: move to a `legacy-v1/` directory or a separate branch. Keep only reusable patterns (`errors.rs` error type hierarchy, protobuf build configuration).
- Create clean Cargo workspace with `crates/chrononode-core` and `crates/chrononode-cli`.
- Replace placeholder README with the v2.1 identity (see Section 17).
- Add this `plan.md`.
- Add `docs/architecture.md`.
- Add `docs/proof-model.md`.
- Add `docs/adapter-authoring.md`.
- Add GitHub Actions for:
  - `cargo fmt --check`
  - `cargo clippy -- -D warnings`
  - `cargo test --workspace`
  - `cargo doc --no-deps`
- Remove `chrononode_archival_client/` from workspace default members (keep it inert but not deleted).

Success criteria:

```text
New contributor can understand what ChronoNode is in 5 minutes.
Workspace builds cleanly (cargo build --workspace succeeds).
Scope is clearly independent but adapter-based.
Old v1 code does not appear in workspace compilation.
```

---

## Phase 1 — Local Archival Loop (1–2 weeks)

Goal:

```text
Mock blocks → protobuf → local_fs storage → SQLite index → query → proof
```

Build:

- `ChronoBlock` protobuf (with schema_version and hash_algorithm fields).
- `chrononode-core` crate with traits: `ChainAdapter`, `StorageBackend`, proof types.
- Mock adapter (generates deterministic blocks).
- Local filesystem storage backend.
- SQLite schema and migrations (including `ingest_state` and `merkle_checkpoints` tables).
- Archive pipeline (adapter → serialize → store → index).
- Merkle checkpoint builder (length-prefixed leaf hashes, configurable checkpoint size).
- Idempotent ingest (checks `ingest_state` + `archived_blocks` before fetching).
- `Dockerfile` + `docker-compose.yml` (ChronoNode + local IPFS daemon, so the full loop runs with `docker compose up`).
- CLI commands:
  - `init`
  - `ingest --chain mock`
  - `query block`
  - `prove block`
  - `verify`
- `criterion` benchmarks for Merkle tree build time and proof generation latency.

Success criteria:

```text
cargo test --workspace passes (including integration tests in tests/).
A mock chain block can be archived and proven without IPFS or BaaLS.
cargo bench shows determinable perf baselines.
docker compose up runs the full mock archival loop.
```

---

## Phase 2 — BaaLS Adapter MVP (1–2 weeks)

Goal:

```text
BaaLS JSON-RPC → ChronoNode archive pipeline
```

Build:

- BaaLS adapter implementing `ChainAdapter` trait (JSON-RPC client, block normalization into `ChronoBlock`).
- If BaaLS exposes a Rust crate, use a native adapter instead of JSON-RPC for lower latency and better reliability.
- Config for BaaLS endpoint.
- Follow mode (`ingest --chain baals --follow`) that polls for new blocks.
- Integration test using mocked BaaLS RPC responses.
- `buster` tx model in tests (BaaLS native transaction format).

Success criteria:

```text
ChronoNode can ingest BaaLS blocks by height and by range.
ChronoNode core crate compiles with zero BaaLS imports (enforced by workspace split).
Integration tests pass with mocked BaaLS RPC.
```

---

## Phase 3 — IPFS / Pinata Storage (1 week)

Goal:

```text
ChronoBlock bytes → IPFS/Pinata → CID → SQLite pointer
```

Build:

- IPFS HTTP backend.
- Pinata backend.
- Local cache.
- Retrieval fallback.
- Repair command.

Success criteria:

```text
Archived block can be fetched from IPFS by pointer.
Retrieval failure can be detected.
Repair can re-fetch from source and re-pin.
```

---

## Phase 4 — Minimal HTTP API (1 week)

Goal:

```text
External apps can query archived blocks and proofs.
```

Build:

- Axum server.
- `/health`
- `/v1/chains`
- `/v1/chains/{chain_id}/blocks/{height}`
- `/v1/chains/{chain_id}/blocks?from={from}&to={to}`
- `/v1/chains/{chain_id}/proofs/block/{height}`
- `/v1/proofs/verify`
- API key middleware optional but recommended.

Success criteria:

```text
CanvasContracts or a web client can request block history and proof JSON.
```

---

## Phase 5 — Production Hardening (1–2 weeks)

Build:

- Structured logs with `tracing` (JSON format).
- Prometheus metrics endpoint.
- Config file examples (TOML).
- Backup and restore for SQLite.
- Archive verification command (`chrononode verify-archive`).
- Rate limiting (tower middleware).
- API key auth (optional but recommended).
- Basic OpenAPI docs via `utoipa`.

Success criteria:

```text
ChronoNode can run continuously on a $5–10 VPS.
Operator can monitor indexed blocks, failed retrievals, and query traffic.
```

---

## Phase 6 — Event and Transaction Indexing (2–3 weeks)

Only start this after block archival works.

Build:

- Optional transaction table.
- Optional event table.
- Query by transaction hash.
- Query by contract/emitter.
- Query by address.
- CanvasContracts contract deployment/execution timeline.

Success criteria:

```text
ChronoNode can support explorer/debugger features without scanning entire block payloads.
```

---

## Phase 7 — Additional Network Adapters (Ongoing)

Candidate order:

1. BaaLS.
2. Local file/import adapter.
3. Vigil or another controlled/custom network.
4. Bitcoin.
5. Ethereum.

Do not add public L1s until:

```text
adapter trait is stable
proof model is stable
index schema is stable
BaaLS integration works end-to-end
```

---

## 15. Success Metrics

### 15.1 Technical Metrics

Track:

```text
blocks archived
archive throughput
average IPFS upload latency
average retrieval latency
proof generation latency
proof verification latency
index DB size
repair success rate
CID retrieval failure rate
```

### 15.2 Project Metrics

Track:

```text
time from fresh clone to working demo
number of supported adapters
number of successful proof verifications
docs completeness
number of external apps using API
```

---

## 16. Key Design Principles

1. **Independent core, adapter integrations.** (enforced by workspace separation).
2. **BaaLS first, not BaaLS only.**
3. **Merkle proofs now, ZK later.**
4. **SQLite first, Postgres later.**
5. **REST first, GraphQL/gRPC later.**
6. **Local filesystem backend first, IPFS second.**
7. **No token until real external usage exists.**
8. **No consensus layer until multiple independent operators exist.**
9. **No microservices until one binary becomes painful.**
10. **Every response should be verifiable or clearly marked unverified.**
11. **Verify on retrieval, not just on storage.** Re-hash and compare after every `get`.
12. **Ingest must survive crashes.** SQLite transaction scope ensures atomicity; idempotent resume on restart.
13. **Operator key stays local.** Generate at `init`, never log, never serialize.

---

## 17. README Rewrite Recommendation

Replace the placeholder README with:

```markdown
# ChronoNode

ChronoNode is an independent verifiable archival layer for blockchain and app-ledger history.

It archives blocks/events from supported networks into content-addressed storage, stores compact metadata locally, and serves historical queries with Merkle proofs.

## Quick Start

```bash
docker compose up
```

See the demo flow in `docs/architecture.md`.

## First Supported Network

BaaLS is the first reference adapter. ChronoNode is not limited to BaaLS; additional networks can be supported through the ChainAdapter interface.

## MVP Flow

BaaLS block → ChronoBlock protobuf → IPFS/local storage → SQLite index → query API → Merkle proof.

## Status

Early design/build phase. Not production-ready.
```

---

## 18. Test Strategy

### 18.1 Test Pyramid

```text
┌──────────────────┐
│   E2E tests      │  tests/archival_flow.rs — full mock adapter → proof loop
├──────────────────┤
│ Integration tests│  tests/adapter_contract.rs — adapter trait contract tests
├──────────────────┤
│ Property tests   │  Merkle proof verification against randomly generated trees
├──────────────────┤
│ Unit tests       │  #[cfg(test)] inline in each source file
└──────────────────┘
```

### 18.2 Key Test Categories

| Category | Tool | What It Covers |
|---|---|---|
| Unit tests | Rust `#[test]` | Models, proof math, serialization round-trips |
| Integration tests | `#[cfg(test)]` in `tests/` | Archive pipeline, index CRUD, adapter + storage interaction |
| Adapter contract tests | Shared test harness | Every adapter passes the same test suite (deterministic block generation, reorg simulation) |
| Property-based testing | `proptest` | Merkle leaf hashing commutativity, proof inclusion/absence |
| Merkle fuzzing | `cargo-fuzz` or manual | Invalid proof vectors must never pass verification |
| Benchmarks | `criterion` | Merkle tree build time, checkpoint generation, proof size |

### 18.3 Deterministic Mocking

The mock adapter must:

- Produce the same blocks for the same seed/hash function.
- Support configurable block counts (100, 1000, 10000) for scale testing.
- Generate blocks fast enough that `cargo test` completes in under 5 seconds.

### 18.4 CI Test Matrix

```yaml
# .github/workflows/test.yml
strategy:
  matrix:
    rust: [stable, beta]
features:
  - cargo test --workspace
  - cargo test --workspace --all-features
  - cargo clippy --workspace -- -D warnings
  - cargo doc --workspace --no-deps
```

Add `cargo bench` as a non-blocking nightly job.

---

## 19. Operator Key Management

The checkpoint signing model requires a key pair. This has security and operational implications that should be designed early.

### 19.1 Key Generation

```bash
chrononode init
```

Generates an Ed25519 keypair and stores it at `~/.chrononode/operator_key`. The private key is stored with file permissions `0600`. Alternatively, accept `CHRONONODE_OPERATOR_KEY` as an env var (hex-encoded 64-byte seed) for containerized deployments.

### 19.2 Key Storage

| Environment | Key Location | Rationale |
|---|---|---|
| Local dev | `~/.chrononode/operator_key` | File-based, `chmod 600` |
| Docker | `CHRONONODE_OPERATOR_KEY` env var | Passed via secrets manager or `.env` |
| Production | `/secrets/operator_key` (mounted volume) | Kubernetes Secrets / Vault |

### 19.3 Key Rotation

Not required for v0.1, but the checkpoint schema includes a `signer_pubkey` column. When rotation is needed:
1. Generate new key.
2. Sign new checkpoints with the new key.
3. Clients verify against a list of trusted keys (not just one).
4. Old key is kept for historical proof verification but no longer used for signing.

### 19.4 Security

- The private key never leaves the ChronoNode process.
- The signing module should be a small, isolated function with no I/O dependencies (unit-testable).
- Never log or serialize the private key.

---

## 20. Content Verification on Retrieval

A content-addressed pointer (CID, SHA-256 hash, etc.) is not enough on its own. The storage retrieval flow must verify content integrity after fetching.

### 20.1 Required Verification Flow

```text
1. retrieve bytes from storage backend
2. compute expected_hash from StoragePointer (e.g., sha256(bytes) for local_fs, CID hash for IPFS)
3. compare expected_hash with the block_hash stored in the archived_blocks table
4. if mismatch → retrieval has failed → trigger repair flow
5. if match → return bytes
```

This verification lives inside `StorageBackend::get` or as a wrapper in the archive pipeline. It must not be optional.

### 20.2 Why This Matters

IPFS gateways can return incorrect data for a valid CID (malicious or misconfigured gateway, bit rot on the storage node, hash collision in non-cryptographic contexts). Without re-hashing after retrieval, ChronoNode could return wrong data while claiming it's verified by the CID pointer.

### 20.3 Local FS Verification

For `local_fs`, the "pointer" is a path like `blocks/baals/000000500.block`. The storage backend should:
1. Read the file.
2. Hash the contents.
3. Compare with the hash stored in `archived_blocks.block_hash`.
4. Fail if they don't match.

---

## 21. Graceful Shutdown & Crash Safety

The archive pipeline must be safe to interrupt at any point (Ctrl+C, OOM kill, VPS reboot).

### 21.1 Design

```text
For each block:
1. fetch from adapter   → volatile memory
2. serialize to protobuf → volatile memory
3. store to storage     → durable (storage backend write)
4. insert into SQLite   → durable (within a transaction)
5. update ingest_state  → durable (within the same transaction as step 4)
```

Steps 3, 4, and 5 MUST be atomic or ordered such that restart is always safe:
- If crash after step 3 but before step 4: on restart, the index won't have the block. `ingest_state` will show it's not archived. The next run re-fetches and re-archives. The orphaned storage object is harmless (garbage collection later).
- If crash after step 4 but before step 5: on restart, `archived_blocks` has the row but `ingest_state` might be stale. The ingest loop should check `archived_blocks` for the highest height and update `ingest_state` accordingly.

### 21.2 Implementation

Wrap steps 3–5 in a SQLite transaction:

```rust
let mut tx = db.begin().await?;
// insert into archived_blocks
// update ingest_state
tx.commit().await?;
```

If `tx.commit()` hasn't returned yet and the process crashes, SQLite's WAL journal will roll back the transaction on next open.

### 21.3 Graceful Shutdown Signal

```rust
tokio::select! {
    result = ingest_pipeline.run() => result,
    _ = tokio::signal::ctrl_c() => {
        tracing::info!("shutting down ingest pipeline, waiting for in-flight blocks...");
        ingest_pipeline.drain().await;
        Ok(())
    }
}
```

The `drain()` method completes the current block and flushes the index before returning.

---

## 22. Biggest Risks and Mitigations

| Risk | Mitigation |
|---|---|---|
| Too much scope too early | Build mock adapter + local filesystem first |
| IPFS unreliability | Add repair flow with policy levels and local cache |
| BaaLS coupling | Workspace-enforced adapter independence; no BaaLS imports in core |
| Query API complexity | Start with REST and block queries only; add range endpoint for efficiency |
| Proof ambiguity | Length-prefixed domain-separated leaf hashing |
| Multi-chain complexity | Stabilize adapter trait and proof model before adding public chains |
| Placeholder-heavy legacy code | Quarantine `chrononode_archival_client/` immediately; start fresh workspace |
| Overbuilding microservices | Two-crate workspace (core + cli); split further only when justified |
| Content-integrity on retrieval | Hash-and-compare after every storage `get`; see Section 21 |
| Crash during ingest | SQLite transactions + idempotent resume; see Section 22 |
| Operator key compromise | Isolated signing module; never log or serialize the private key; see Section 20 |
| Source chain prunes archived blocks | Repair policy levels (RECOVERABLE vs MIRROR-ONLY); proactive `verify-archive` |

---

## 23. Recommended Near-Term Task List

### Day 1–2

- Quarantine old `chrononode_archival_client/` code (move to `legacy-v1/` directory).
- Create Cargo workspace with `crates/chrononode-core` and `crates/chrononode-cli`.
- Replace README with v2.1 identity.
- Add `plan.md`.
- Add CI (fmt, clippy, test, doc).

### Day 3–5

- Add Protobuf schema (with `schema_version`, `hash_algorithm`).
- Add `chrononode-core` traits: `ChainAdapter`, `StorageBackend`.
- Add `ChronoBlock` Rust types with serialization.
- Add SQLite migrations (`ingest_state`, `merkle_checkpoints`, `archived_blocks`).
- Add local filesystem storage backend (with content verification).

### Week 2

- Add mock adapter.
- Add ingest pipeline (idempotent, crash-safe).
- Add query by height.
- Add Merkle checkpoint/proof module (length-prefixed leaf hashing).
- Add proof verification CLI.
- Add `Dockerfile` + `docker-compose.yml`.
- Add `criterion` benchmarks for proof operations.

### Week 3

- Add BaaLS adapter.
- Add BaaLS config.
- Add mocked BaaLS RPC integration tests.

### Week 4

- Add IPFS/Pinata backend.
- Add repair command.
- Add minimal HTTP API.

### Week 5–6

- Add metrics.
- Add auth/rate limiting.
- Add operations docs.
- Add backup/restore tooling.
- Demo with BaaLS and CanvasContracts.

---

## 24. Final Direction

ChronoNode should be built as an independent project with BaaLS as the first proof point.

The winning architecture is:

```text
ChronoNode Core
  network-agnostic archive, index, proof, and query system

Adapters
  BaaLS first, other networks later

Storage Backends
  local_fs first, IPFS/Pinata next, Arweave later

APIs
  CLI and REST first, GraphQL/gRPC later
```

The winning demo is:

```text
Run BaaLS locally.
Run ChronoNode.
Archive BaaLS blocks.
Query a historical block.
Return a Merkle proof.
Verify the proof.
Show the same history inside CanvasContracts.
```

That gives ChronoNode a real identity, a practical build path, and a credible future beyond BaaLS.

---

## 25. Implementation Status

| Phase | Status | Notes |
|---|---|---|
| Phase 0 — Repository Cleanup | ✅ Complete | Old code quarantined to `legacy-v1/`, Cargo workspace created, CI configured, docs written |
| Phase 1 — Local Archival Loop | ✅ Complete | Core types, protobuf, ChainAdapter/StorageBackend traits, SQLite index, mock adapter, local FS storage, Merkle proofs, CLI, Docker, tests |
| Phase 2 — BaaLS Adapter | ✅ Complete | JSON-RPC implementation: `baals_getBlockByHeight`, `baals_getBlockByHash`, `baals_blockNumber` |
| Phase 3 — IPFS/Pinata Storage | 🟡 Skeleton | Backend traits wired; production implementation deferred |
| Phase 4 — Minimal HTTP API | ✅ Complete | Axum server with `/health`, `/v1/chains`, `/v1/chains/{id}/blocks/{height}`, range query, proof verification, metrics |
| Phase 5 — Production Hardening | ✅ Complete | Tracing, metrics endpoint, API key auth, config example, backup/restore CLI, verify-archive command, rate limiting |
| Phase 6 — Event/Tx Indexing | ✅ Complete | `indexed_txns` + `indexed_events` tables, query by sender/recipient/event-type, CLI commands |
| Phase 7 — Additional Adapters | 🟡 BaaLS done | BaaLS adapter implemented; Vigil/Bitcoin/Ethereum deferred |

### Test Results

```
13 tests passing:
  - chrononode-core: 6 tests (Merkle proofs, domain separation, odd-size trees)
  - chrononode-cli integration: 7 tests (serialization, local FS, archival flow, adapter contract)
```

### Key Files

| File | Purpose |
|---|---|
| `crates/chrononode-core/src/block.rs` | Canonical ChronoBlock model |
| `crates/chrononode-core/src/chain.rs` | ChainAdapter & StorageBackend traits |
| `crates/chrononode-core/src/proof.rs` | Length-prefixed Merkle proof system |
| `crates/chrononode-cli/src/adapters/mock.rs` | Deterministic mock chain |
| `crates/chrononode-cli/src/adapters/baals.rs` | BaaLS JSON-RPC adapter |
| `crates/chrononode-cli/src/index/sqlite.rs` | SQLite index with all tables |
| `crates/chrononode-cli/src/api/http.rs` | Axum HTTP API with auth + metrics |
| `crates/chrononode-cli/src/main.rs` | CLI entry point with all commands |
| `docker-compose.yml` | Single-command deployment |
