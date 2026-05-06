# ChronoNode Architecture

## Overview

ChronoNode is a two-crate workspace:

```
chrononode-core   ← traits, models, proof logic (no I/O, no adapter deps)
chrononode-cli    ← CLI binary (adapters, storage, index, API, verification)
```

## Data Flow

```
ChainAdapter.fetch_block(height)
    → ChronoBlock (canonical model)
    → archive::serializer::serialize_block (protobuf)
    → StorageBackend.put (content-addressed storage)
    → SQLite index insert (metadata only)
    → Merkle checkpoint builder
    → proof verification
    → HTTP API query
```

## Key Traits

- `ChainAdapter` — fetch blocks from any network
- `StorageBackend` — store/retrieve content-addressed data
- `SqliteIndex` — compact metadata index, checkpoint tracking

## Design Principles

See `chrononode-plan.md` Section 16.
