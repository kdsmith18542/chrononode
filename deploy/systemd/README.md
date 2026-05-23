# Systemd Deployment

These units run ChronoNode as two services per chain:

- `chrononode-ingest@<chain>.service`
- `chrononode-api@<chain>.service`

Example chain IDs:
- `mock`
- `baals`
- `bitcoin`
- `dogecoin`

## Prerequisites

1. Binary installed at `/usr/local/bin/chrononode` (behaves same as `chrononode-cli`)
2. Config at `/etc/chrononode/config.toml` (copy from `config/chrononode.toml.example`)
3. Operator key at `/etc/chrononode/operator_key` (generated via `chrononode init`)
4. BaaLS signing key at `/etc/chrononode/baals.key` (32 bytes raw ed25519 seed)
5. Watch lists imported:
   ```bash
   chrononode watch import --chain bitcoin --file config/watchlist-bitcoin.txt
   chrononode watch import --chain dogecoin --file config/watchlist-dogecoin.txt
   ```

## Install

```bash
# Binary
sudo install -m 0755 target/release/chrononode-cli /usr/local/bin/chrononode

# User + dirs
sudo useradd --system --home /var/lib/chrononode --create-home --shell /usr/sbin/nologin chrononode || true
sudo install -d -m 0750 -o chrononode -g chrononode /etc/chrononode
sudo install -d -m 0750 -o chrononode -g chrononode /var/lib/chrononode

# Config
sudo cp config/chrononode.toml.example /etc/chrononode/config.toml
sudo chown chrononode:chrononode /etc/chrononode/config.toml
sudo chmod 0640 /etc/chrononode/config.toml

# Units
sudo cp deploy/systemd/chrononode-ingest@.service /etc/systemd/system/
sudo cp deploy/systemd/chrononode-api@.service /etc/systemd/system/
sudo cp deploy/systemd/chrononode.target /etc/systemd/system/
sudo systemctl daemon-reload

# Enable
sudo systemctl enable --now chrononode-ingest@baals.service
sudo systemctl enable --now chrononode-api@baals.service
sudo systemctl enable --now chrononode-ingest@bitcoin.service
sudo systemctl enable --now chrononode-api@bitcoin.service
```

## Operations

```bash
sudo systemctl status chrononode-ingest@baals.service
sudo systemctl status chrononode-api@baals.service
sudo journalctl -u chrononode-ingest@baals.service -f
sudo journalctl -u chrononode-api@baals.service -f
```

## Data paths
```
/var/lib/chrononode/data/<chain>/index.db     # SQLite index
/var/lib/chrononode/blocks/                   # Archived block data
/var/lib/chrononode/config.toml               # Alternative config location
```

## Scale-out
- Move `chrononode-ingest@<chain>` to another VPS when needed
- Keep `chrononode-api@<chain>` on the API-facing node
- Same unit files, env layout

## See Also
- `docs/integration.md` — full ChronoNode → Resurgence integration guide
- `config/chrononode.toml.example` — config reference with `[dormancy]` and `[attestation]` sections
