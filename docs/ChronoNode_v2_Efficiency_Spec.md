# ChronoNode v2.0 — Bootstrapped Efficiency Specification

**Version**: 2.0
**Date**: May 2026
**Status**: Design Phase — replaces ZK-SNARK ambitions with Merkle proofs and IPFS indexing for solo-dev viability.
**Supersedes**: Sections 2.3, 4.2, 5, 6, 7 of PB.md where ZK-SNARKs, $CHN token, and Consensus Layer are referenced. All TB.md Rust tech stack, microservices, API design, observability, and testing sections remain intact and are incorporated by reference.

---

## 1. Core Pivot: Stateless Archival with Verifiable Pointers

### 1.1 The Shift

The v1.x specification assumed a decentralized network of incentivized operators running ZK-provers, a consensus blockchain, and a native token economy. That architecture requires a team, funding, and a launched network before it delivers value.

**v2.0 changes the model from "hosting terabytes" to "verifying pointers":**

| Concept | v1.x | v2.0 |
|---------|------|------|
| Storage model | Host full chain history on operator SSDs | Store raw blocks on IPFS/Arweave; keep only CID index locally |
| Verification | ZK-SNARKs for state transitions | Merkle inclusion proofs (implementable today) |
| Economic model | $CHN token, DAO, staking, slashing | Pay-as-you-go IPFS pinning + VPS; token deferred to post-traction |
| Network model | Decentralized operator network | Single-node operation; federated later |
| Multi-chain | Bitcoin + Ethereum + Solana from day 1 | BaaLS first; add chains as adapter plugins |
| Budget | $200-500/mo (operator-grade hardware) | **$25-50/mo** (VPS + pinning service) |

### 1.2 What Stays From v1.x

The entire Rust microservice architecture, API design, event system, observability stack, and multi-chain adapter pattern from `ChronoNodeTB.md` remain the implementation backbone. This document specifies what changes — everything else carries forward.

---

## 2. The Four-Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ LAYER 4: Query API Gateway (query-api-svc)                  │
│ GraphQL + JSON-RPC + gRPC. Merkle proofs in responses.      │
│ Built with axum + async-graphql + jsonrpsee (from TB.md)    │
├─────────────────────────────────────────────────────────────┤
│ LAYER 3: Metadata Index (indexing-svc)                      │
│ SQLite local index: {chain_id, height, block_hash → CID}.   │
│ 10M blocks = ~640MB on disk. (~$0 cost with VPS SSD)        │
│ PostgreSQL for structured queries on recent data.            │
├─────────────────────────────────────────────────────────────┤
│ LAYER 2: Cold Storage Gateway (dsn-gateway-svc)             │
│ IPFS for active retrieval. Arweave for permanent pins.      │
│ Rust IPFS client (rust-libp2p / ipfs-api).                  │
│ Pay Pinata ($20/mo) or run self-hosted IPFS node.           │
├─────────────────────────────────────────────────────────────┤
│ LAYER 1: Chain Ingestion (CACs — ChronoNode Archival Clients)│
│ Syncs with BaaLS (primary), later Vigil, external L1s.       │
│ Serializes blocks as Protobuf → pushes to IPFS → records CID.│
│ Rust + tokio + protobuf + ipfs-api.                          │
└─────────────────────────────────────────────────────────────┘
```

### 2.1 Layer 1: Chain Ingestion (CAC)

The CAC connects to a running BaaLS node (or any supported chain RPC) and processes new blocks:

1. **Sync**: Connect to BaaLS node via its JSON-RPC API (already built in BaaLS)
2. **Serialize**: Convert each block to a compact Protobuf `ChronoBlock` message (see Section 8)
3. **Push to IPFS**: Upload serialized block, receive a Content Identifier (CID)
4. **Index CID**: Store `{chain_id, block_height, block_hash → CID, merkle_root}` in the local SQLite index
5. **Discard raw data**: Delete the block from local disk — the CID is the sole reference

**Key insight**: The CAC never stores full chain history. It only stores 64-byte pointers per block.

### 2.2 Layer 2: Cold Storage Gateway (DSN Gateway)

Handles all communication with decentralized storage:

- **IPFS (primary)**: For active, frequently-accessed recent blocks. Uses either a self-hosted IPFS node or Pinata's pinning API.
- **Arweave (permanent)**: For genesis blocks, checkpoint blocks (every 1,000th block), and critical state snapshots. One-time payment per byte stored.
- **Fallback**: If IPFS retrieval fails, the CAC can re-fetch from the source BaaLS node and re-pin.

**BaaLS community pinning (future)**: Once BaaLS has active users, wallets can optionally pin shards of chain history, creating a distributed storage layer at $0 marginal cost. Not required for v2.0 launch.

### 2.3 Layer 3: Metadata Index (Indexing Service)

A local, lightweight database that maps blockchain identifiers to storage locations:

- **SQLite** (primary): `{chain_id, block_height, block_hash, cid, merkle_root, timestamp}` — covers 100% of indexing
- **PostgreSQL** (optional, for structured queries): Transaction-level indexing (sender, recipient, amount, method) for recent blocks
- **Storage math**: 64 bytes per row × 10M blocks = 640MB. Fits on any VPS.

### 2.4 Layer 4: Query API Gateway

Unchanged from `ChronoNodeTB.md` Section 2.6. Provides:

- **GraphQL**: Flexible historical queries (block by height, transaction by hash, account history)
- **JSON-RPC**: Compatibility with existing blockchain tooling
- **gRPC**: High-performance internal and external access
- **Merkle proofs**: Every response optionally includes a Merkle inclusion proof the client can verify against a known state root (Section 5)
- **Rate limiting, auth, caching**: As specified in TB.md Section 2.6

---

## 3. Data Flow: End-to-End

```
BaaLS Node produces Block #500
         │
         ▼
[CAC] Serialize block → ChronoBlock protobuf (400-800 bytes for typical block)
         │
         ▼
[CAC] Push to IPFS → receives CID: QmXyZ...
         │
         ▼
[CAC] Write to SQLite: (baals, 500, 0xabc..., QmXyZ..., 0xdef...)
         │
         ▼
[CAC] Delete local block copy
         │
    ─ ─ ─ later ─ ─ ─
         │
[User] Query: "Give me BaaLS Block #500"
         │
         ▼
[query-api-svc] Lookup SQLite → CID: QmXyZ...
         │
         ▼
[dsn-gateway-svc] Fetch from IPFS → ChronoBlock bytes
         │
         ▼
[query-api-svc] Deserialize → return block + Merkle proof → User verifies
```

---

## 4. Verification Strategy: Merkle Proofs (Not ZK-SNARKs)

### 4.1 How It Works

Every N blocks (default: N=1,000), the CAC computes a **checkpoint Merkle root** of all block hashes in that range and stores it. When a user requests any block, the API response includes:

1. The requested block data
2. A Merkle inclusion proof that the block hash is part of the checkpoint root
3. The checkpoint root itself (signed by the CAC or verifiable against chain state)

The user verifies: `MerkleProof.verify(block_hash, checkpoint_root) → true/false`

### 4.2 Why Merkle Proofs Instead of ZK-SNARKs

| Factor | ZK-SNARKs | Merkle Proofs |
|--------|-----------|---------------|
| Implementation complexity | 6-12 months of specialized crypto work | Standard library call |
| Proof generation cost | Heavy CPU/GPU, minutes per proof | Essentially free (SHA-256) |
| Proof size | ~200 bytes (Groth16) | ~1KB for 1,000-leaf tree |
| Verification cost | ~3ms (depends on curve) | ~0.01ms |
| Requires trusted setup | Yes (for Groth16) | No |
| Library maturity in Rust | Active but complex (arkworks, halo2) | Production-grade (rs-merkle, sha2) |
| Sufficient security for archival | Yes | Yes — same security model as Bitcoin SPV |

### 4.3 Checkpoint Verification

To prevent the CAC from fabricating checkpoints, each checkpoint root is **signed by the CAC's key** and the signature is verified against a known CAC public key. In a federated future, multiple CACs cross-sign each other's checkpoints.

### 4.4 ZK-SNARK Path (Future Research — Not v2.0)

ZK-SNARKs are pushed to the "Future Research" appendix. They would enable:

- Compressing an entire 1,000-block checkpoint proof into ~200 bytes
- Allowing light clients to verify chain history without downloading any blocks
- Enabling recursive proof composition for multi-chain state verification

But none of this is required for a functional archival layer. Merkle proofs provide equivalent security with a fraction of the implementation cost.

---

## 5. Multi-Chain Support

### 5.1 Adapter Plugin Pattern

The CAC architecture from TB.md Section 2.1 is preserved: each chain gets its own adapter implementing the `ChainAdapter` trait (Rust):

```rust
trait ChainAdapter {
    async fn sync_blocks(&self, from_height: u64) -> Vec<ChronoBlock>;
    fn chain_id(&self) -> &str;
    fn block_model(&self) -> BlockModel; // UTXO, Account, or Custom
}
```

### 5.2 Supported Chains (Phased)

| Phase | Chain | Adapter Complexity | Rationale |
|-------|-------|--------------------|-----------|
| Phase 1 (now) | **BaaLS** | Low | Same developer, JSON-RPC API already built, UTXO model |
| Phase 2 | Vigil (Decred fork) | Medium | Go codebase, but wire protocol is well-documented |
| Phase 3 | Bitcoin | High | Existing Rust bitcoin libraries, high value |
| Phase 3 | Ethereum | High | ethers-rs available, but EVM state is complex |

### 5.3 BaaLS as the Reference Implementation

BaaLS is the ideal first chain because:
- It exposes a JSON-RPC API (already built: `rusty-jsonrpc` crate)
- Its block structure is simple (index, timestamp, transactions, metadata)
- The developer controls both the source chain and the archival layer, enabling tight integration
- Proving ChronoNode works with BaaLS creates a demonstration for external chains

---

## 6. Realistic Budget

### 6.1 Monthly Operating Costs

| Component | Provider | Cost | Notes |
|-----------|----------|------|-------|
| VPS (compute + SSD) | Hetzner CX22 / DigitalOcean Basic | **$5-7/mo** | 2 vCPU, 4GB RAM, 40GB SSD |
| IPFS pinning | Pinata (dedicated gateway) | **$20/mo** | 50GB pinned, 500GB bandwidth. Self-hosted IPFS drops this to $0 but costs VPS disk. |
| Arweave (permanent pins) | Arweave | **~$2-5/mo** | Genesis blocks + checkpoints only (~10MB/mo for BaaLS) |
| Domain | Namecheap | **~$1/mo** | Annual purchase |
| Monitoring | Grafana Cloud free tier | **$0** | For single-node operation |
| **Total** | | **$28-33/mo** | |

### 6.2 Scaling Costs

| Growth Stage | Additional Cost | Trigger |
|-------------|----------------|---------|
| 2nd chain added | +$0-5/mo | Each additional chain adds ~10MB of checkpoint data to Arweave |
| Self-hosted IPFS | +$10-20/mo for disk | When pinned data exceeds Pinata free tier |
| PostgreSQL for rich queries | +$0 | Included in VPS for small-to-medium datasets |
| Second VPS for HA | +$5-7/mo | When uptime SLA becomes critical |

### 6.3 What This Budget Doesn't Cover

- A launched `$CHN` token (requires legal, smart contract audit, exchange listing)
- Hardware for ZK-proof generation (requires GPU instances at $200+/mo)
- A team to maintain multiple chain adapters simultaneously
- 24/7 on-call operations support

These are post-traction concerns. The v2.0 budget covers a fully functional archival layer for BaaLS that runs 24/7.

---

## 7. What Is Removed From v1.x (And Why)

| v1.x Component | v2.0 Disposition | Reason |
|---------------|-----------------|--------|
| ZK-SNARK/STARK proof generation (zkp-prover-svc) | **Removed** → Future Research | Requires specialized crypto expertise, GPU hardware, trusted setup |
| ChronoNode Consensus Layer (CL) | **Removed** → Future Work | Requires launched token economy; overkill for single-node archival |
| $CHN Token & Tokenomics | **Removed** → Future Work | Requires legal, audit, exchange listing, community; premature |
| DAO Governance | **Removed** → Future Work | No token, no DAO |
| Operator staking/slashing | **Removed** → Future Work | Requires CL + token |
| Rust-based ZK-prover microservice | **Removed** → Future Research | Replaced by Merkle proof library call |
| Hardware acceleration (GPU/FPGA/ASIC for ZK) | **Removed** → Future Research | Solves a problem v2.0 doesn't have |
| Team & Advisor placeholder sections | **Removed** | Solo dev project |
| Legal & Compliance placeholder sections | **Removed** | No token, no regulatory concern at this stage |
| Filecoin/Storj DSN integration | **Deferred** | IPFS + Arweave sufficient for v2.0 |
| Cross-chain data orchestration (CCIP, LayerZero) | **Deferred** | Requires multiple supported chains first |

### What Is Preserved From v1.x

- Rust microservice architecture (CAC, CQS, indexing-svc, query-api-svc)
- GraphQL + JSON-RPC + gRPC API design (TB.md Section 2.6)
- Event-driven inter-service communication (TB.md Section 2.8)
- Prometheus + Grafana + OpenTelemetry observability (TB.md Section 7)
- Docker + Kubernetes deployment model (TB.md Section 5.3)
- Property-based testing, fuzzing, benchmarking (TB.md Section 5.2)
- Cross-chain adapter plugin pattern (TB.md Section 2.1)
- PostgreSQL indexing for structured queries (TB.md Section 2.5)
- DSN Gateway abstraction for storage backends (TB.md Section 2.4)

---

## 8. Protobuf Schema: ChronoBlock

Compact binary serialization of chain data for IPFS storage. Replaces JSON to minimize storage and bandwidth.

```protobuf
syntax = "proto3";

message ChronoBlock {
    string chain_id = 1;          // "baals", "vigil", "bitcoin"
    uint64 height = 2;            // Block index
    bytes block_hash = 3;         // 32 bytes
    bytes prev_hash = 4;          // 32 bytes
    uint64 timestamp = 5;         // Unix seconds
    uint64 nonce = 6;             // Mining nonce (PoW) or block index (PoA)
    repeated ChronoTx transactions = 7;
    bytes extra_data = 8;         // Chain-specific metadata
}

message ChronoTx {
    bytes tx_hash = 1;            // 32 bytes
    bytes sender = 2;             // Variable length (public key or address)
    bytes recipient = 3;          // Variable length
    uint64 amount = 4;            // Native token value
    uint64 nonce = 5;             // Sender nonce
    bytes payload = 6;            // Contract call data, memo, etc.
    uint64 gas_limit = 7;
    uint64 gas_used = 8;          // If available
}
```

---

## 9. Implementation Roadmap

### Phase 1: BaaLS Archival MVP (4-6 weeks)

| Week | Deliverable | Files |
|------|-------------|-------|
| 1-2 | Protobuf schema + Rust codegen. SQLite index schema. | `proto/chrononode.proto`, `src/storage/index.rs` |
| 2-3 | BaaLS CAC adapter: sync blocks via JSON-RPC, serialize to protobuf, push to IPFS. | `src/adapters/baals_adapter.rs` |
| 3-4 | DSN Gateway: IPFS upload/retrieval via `ipfs-api` or `rust-libp2p`. Pinata integration. | `src/dsn/ipfs_gateway.rs` |
| 4-5 | Query API: GraphQL endpoint for `getBlock(height)`, `getTx(hash)`, `getAccountHistory(address)`. | `src/api/graphql.rs` |
| 5-6 | Merkle checkpoint generation + proof delivery in API responses. Integration test: BaaLS block → archived → queried → verified. | `src/verification/merkle.rs` |

### Phase 2: Production Hardening (2-3 weeks)

- Dockerfile + docker-compose for single-command deployment
- Prometheus metrics: blocks indexed, IPFS latency, query QPS
- Grafana dashboard (import from existing `dashboards/chrononode-dashboard.json`)
- Rate limiting, API key authentication
- Arweave integration for permanent checkpoint pins

### Phase 3: Multi-Chain (Ongoing)

- Vigil CAC adapter (Decred wire protocol)
- External L1 adapters (Bitcoin, Ethereum) as community interest grows
- Federated verification: multiple CACs cross-signing checkpoints

---

## 10. Integration With Other Projects

### 10.1 BaaLS ↔ ChronoNode

BaaLS provides the chain data. ChronoNode archives it. The integration is a one-way data flow:

```
BaaLS Node (JSON-RPC) → ChronoNode CAC → IPFS + SQLite → Query API
```

No modifications needed to BaaLS. ChronoNode consumes BaaLS's existing JSON-RPC API.

### 10.2 CanvasContracts ↔ ChronoNode

CanvasContracts compiles visual graphs to WASM smart contracts that run on BaaLS. ChronoNode archives the deployment and execution history, providing:

- Historical contract state for the Canvas debugger
- "Time travel" debugging: rewind a contract to any past block
- Verifiable proof that a contract was deployed at a specific block

### 10.3 Trellis ↔ ChronoNode (Future)

When Trellis is production-ready, its .NET nodes can serve as additional CAC sources. The Protobuf schema is language-agnostic — a .NET Protobuf implementation can push to the same IPFS storage layer.

---

## Appendix A: Future Research (Not Roadmap)

- **ZK-SNARKs for state transition proofs**: Revisit when Rust ZK libraries mature and GPU proving is accessible. Current estimate: 2027+.
- **$CHN token economy**: Revisit when ChronoNode has 3+ active chain adapters, external users, and a demonstrated willingness to pay for archival access.
- **Decentralized operator network**: Revisit when multiple independent operators want to run ChronoNode nodes.
- **Hardware-accelerated ZK proving**: Revisit when ZK-SNARKs are re-evaluated.
- **Cross-chain data orchestration**: Revisit when ChronoNode supports 3+ chains and cross-chain DApps request the feature.

---

## Appendix B: Related Documents

- `ChronoNodePB.md` — Original v1.1 project blueprint (348 lines). Still valid for vision, problem statement, audience.
- `ChronoNodeTB.md` — Original v1.5 technical specification (944 lines). Still valid for Rust crate list, microservice design, observability, deployment, all API specs. **ZK sections (2.3, 2.8 ZKP parts) are superseded by this document.**
- `ChronoNodeMonetization.md` — Original monetization strategy (67 lines). Superseded by this document's Section 6. Token model deferred to post-traction.
- `todo.md` — Development roadmap (727 lines). Phase 1 tasks in this document replace the original Phase 1-3 tasks. Remaining todo.md items for advanced features (ZKP, CL, cross-chain) are deferred.
