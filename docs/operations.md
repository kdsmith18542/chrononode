# Operations Guide

## Running ChronoNode

```bash
# Initialize
chrononode init

# Ingest mock blocks
chrononode ingest --chain mock --from 0

# Query a block
chrononode query block --chain mock --height 0

# Generate and verify proofs
chrononode prove --chain mock --height 0 --out proof.json
chrononode verify proof.json

# Start API server
chrononode serve --port 8080
```

## Docker

```bash
docker compose up
```

## Monitoring

Prometheus metrics are exposed at `/metrics` when running with the API server.

## Backup

```bash
cp ~/.local/share/chrononode/data/*/index.db /backup/
```
