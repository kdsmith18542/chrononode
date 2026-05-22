# ChronoNode Architecture

## Overview

ChronoNode is a two-crate workspace:

```text
chrononode-core   <- traits, models, proof logic (no I/O, no adapter deps)
chrononode-cli    <- CLI binary (adapters, storage, index, API, verification)
```

## Runtime Topology

Current production-friendly shape (single VPS, easy to scale later):

```text
ingest process/container  -> archives new blocks continuously
api process/container     -> serves queries/proofs from index + storage
shared data path          -> ~/.local/share/chrononode/data/<chain>/
```

This same shape scales by moving `ingest` or `api` to separate VPS nodes later.

## Data Flow

```text
ChainAdapter.fetch_block(height)
  -> ChronoBlock (canonical model)
  -> archive::serializer::serialize_block (protobuf)
  -> StorageBackend.put (content-addressed storage)
  -> SQLite index insert (metadata only)
  -> Merkle checkpoint builder
  -> proof verification
  -> HTTP API query
```

## Key Traits

- `ChainAdapter` - fetch blocks from any network
- `StorageBackend` - store/retrieve content-addressed data
- `SqliteIndex` - compact metadata index, checkpoint tracking

## Design Principles

See `chrononode-plan.md` Section 16.
