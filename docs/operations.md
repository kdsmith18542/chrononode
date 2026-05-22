# Operations Guide

## Deployment Modes

### Single VPS (recommended starting point)

Run ChronoNode as two long-running services on the same VPS:
- `ingest` service: continuously archives new blocks.
- `api` service: serves queries and proofs from the local index/storage.

This matches the current scalable topology and lets you move either service to another VPS later.

### Easy Scale-Out Path

When traffic grows:
- Keep `chrononode-api` on VPS-A.
- Move `chrononode-ingest` to VPS-B.
- Keep storage backend configuration stable (`local_fs`, `ipfs`, or `pinata`).

## Docker (single VPS)

```bash
docker compose up -d
```

This starts:
- `chrononode-ingest` (follow mode)
- `chrononode-api` on port `8080`

Optional: run local IPFS on the same VPS:

```bash
docker compose --profile ipfs up -d
```

## Systemd (single VPS)

ChronoNode includes production-ready unit templates under:

```text
deploy/systemd/
```

Quick install flow:

```bash
# 1) Build and install binary
cargo build --release --workspace
sudo install -m 0755 target/release/chrononode-cli /usr/local/bin/chrononode

# 2) Create service user/dirs
sudo useradd --system --home /var/lib/chrononode --create-home --shell /usr/sbin/nologin chrononode || true
sudo install -d -m 0750 -o chrononode -g chrononode /etc/chrononode /var/lib/chrononode

# 3) Configure environment
sudo cp config/chrononode.env.example /etc/chrononode/chrononode.env
sudo chown root:chrononode /etc/chrononode/chrononode.env
sudo chmod 0640 /etc/chrononode/chrononode.env

# 4) Install units
sudo cp deploy/systemd/chrononode-ingest@.service /etc/systemd/system/
sudo cp deploy/systemd/chrononode-api@.service /etc/systemd/system/
sudo systemctl daemon-reload

# 5) Start chain instance (mock example)
sudo systemctl enable --now chrononode-ingest@mock.service
sudo systemctl enable --now chrononode-api@mock.service
```

For a real chain, replace `mock` with `baals` (or another adapter ID).

## Manual Runtime

```bash
# Initialize
chrononode init

# Ingest loop
chrononode ingest --chain mock --from 0 --follow

# API server (serves real pipeline data for selected chain)
chrononode serve --chain mock --port 8080 --rate-limit 100
```

## Runtime Configuration

Environment variables:

```bash
# Storage backend
CHRONONODE_STORAGE_BACKEND=local_fs    # or ipfs or pinata

# IPFS
CHRONONODE_IPFS_API_URL=http://127.0.0.1:5001

# Pinata
CHRONONODE_PINATA_JWT=...
CHRONONODE_PINATA_API_BASE=https://api.pinata.cloud
CHRONONODE_PINATA_GATEWAY_BASE=https://gateway.pinata.cloud

# API auth (optional)
CHRONONODE_API_KEY=...
```

## Monitoring

Prometheus-style metrics are exposed at:

```text
/metrics
```

Health endpoint:

```text
/health
```

Systemd logs:

```bash
sudo journalctl -u chrononode-ingest@mock.service -f
sudo journalctl -u chrononode-api@mock.service -f
```

## Backup

Use CLI backup/restore for the SQLite index:

```bash
chrononode backup --chain mock --out /backup/index-mock.db
chrononode restore --chain mock --from /backup/index-mock.db
```

Also back up the storage directory for `local_fs` backend:

```bash
# default interactive path (when XDG_DATA_HOME is not set):
~/.local/share/chrononode/data/<chain>/

# systemd template path (XDG_DATA_HOME=/var/lib):
/var/lib/chrononode/data/<chain>/
```
