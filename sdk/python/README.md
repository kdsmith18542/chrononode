# ChronoNode Python SDK

A lightweight Python client library for querying the ChronoNode API and performing client-side Merkle proof and Ed25519 signature verification (stateless client/SPV verification).

## Features

- Fully async-compatible API querying (built on `requests`)
- Client-side Merkle proof verification for block data
- Client-side Ed25519 signature verification of checkpoint roots
- Direct integration with **Pandas** for easy data analytics

## Installation

```bash
pip install .
```

To install with Pandas support for data analytics:

```bash
pip install ".[pandas]"
```

## Basic Usage

```python
from chrononode import ChronoNodeClient

# Initialize client
client = ChronoNodeClient("http://localhost:8080")

# Check node health
health = client.health()
print(f"Status: {health['status']}, Uptime: {health['uptime_seconds']}s")

# List active chains
chains = client.list_chains()
for chain in chains:
    print(f"Chain: {chain['display_name']} ({chain['chain_id']})")
```

## Verifying Proofs Locally

```python
# Fetch block proof at height 500
proof = client.get_block_proof("baals", 500)

# Verify the proof locally (Merkle inclusion proof + optional signature check)
is_valid = client.verify_proof_locally(proof)
print(f"Proof is cryptographically valid: {is_valid}")
```

## Pandas Data Analytics

```python
from chrononode.pandas_helper import to_dataframe

# Fetch range of blocks
blocks = client.get_block_range("baals", 1, 100)

# Convert to Pandas DataFrame
df = to_dataframe(blocks, category="blocks")
print(df.head())
```
