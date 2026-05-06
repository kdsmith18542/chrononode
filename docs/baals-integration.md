# BaaLS Integration Guide

## Overview

BaaLS is ChronoNode's first reference adapter. ChronoNode ingests BaaLS blocks via the `ChainAdapter` trait — no BaaLS internal crates are imported in core.

## MVP Integration

```
BaaLS JSON-RPC → BaalsAdapter → ChronoBlock → archive pipeline
```

## Configuration

```toml
[baals]
rpc_url = "http://localhost:8545"
```

## Adapter Source

See `crates/chrononode-cli/src/adapters/baals.rs`
