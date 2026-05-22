# BaaLS Integration Guide

## Overview

BaaLS is ChronoNode's first reference adapter. ChronoNode ingests BaaLS blocks via the `ChainAdapter` trait — no BaaLS internal crates are imported in core.

## Architecture

```
BaaLS REST API → BaalsAdapter → ChronoBlock → archive pipeline
```

## API Endpoints

The adapter communicates with BaaLS via these REST endpoints:

| Operation | Endpoint | Method |
|-----------|----------|--------|
| Latest Height | `/api/v1/chain/head` | GET |
| Block by Height | `/api/v1/blocks/{index}` | GET |
| Block by Hash | `/api/v1/blocks/by_hash/{hash}` | GET |

## Schema Mapping

### BaalsBlock → ChronoBlock

| BaaLS Field | ChronoBlock Field | Notes |
|-------------|-------------------|-------|
| `index` | `height` | Block height |
| `hash` | `block_hash` | Hex-encoded |
| `prev_hash` | `prev_hash` | Parent block hash |
| `timestamp` | `timestamp` | Unix timestamp |
| `transactions` | `transactions` | Mapped via payload type |

### BaalsTransaction → ChronoTx

| BAAALS Field | ChronoTx Field | Notes |
|--------------|----------------|-------|
| `hash` | `tx_hash` | Hex-decoded |
| `sender` | `sender` | Hex-decoded |
| `recipient` | `recipient` | Hex-decoded |
| `payload.amount` | `amount` | Only for `Transfer` type |
| `nonce` | `nonce` | Transaction nonce |
| `gas_limit` | `gas_limit` | Gas limit |
| `gas_limit * gas_price` | `gas_used` | Estimated gas used |

### BaalsPayload → ChronoEvent

| Payload Type | Event Type | Emitter |
|--------------|------------|---------|
| `ContractCall` | `contract_call` | `recipient` |
| `ContractDeploy` | `contract_deploy` | `sender` |
| `Transfer` | (none) | - |
| `Data` | (none) | - |
| `ValidatorSetChange` | (none) | - |

## Configuration

```bash
# CLI usage
chrononode start --chain baals --url http://localhost:8080
```

## Adapter Source

See `crates/chrononode-adapter-baals/src/lib.rs`

## Testing

Run BAAALS adapter tests:
```bash
cargo test -p chrononode-adapter-baals
```

Run with clippy checks:
```bash
cargo clippy -p chrononode-adapter-baals -- -D warnings
```
