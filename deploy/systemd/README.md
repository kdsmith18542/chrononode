# Systemd Deployment

These units run ChronoNode as services per chain:

- `chrononode-ingest@<chain>.service`
- `chrononode-api@<chain>.service`
- `chrononode-anchor@<chain>.timer` — weekly Arweave checkpoint anchoring

Example chain IDs:
- `mock`
- `baals`
- `bitcoin-light`
- `dogecoin`

## Prerequisites

1. Binary installed at `/usr/local/bin/chrononode-cli`
2. Config at `/etc/chrononode/config.toml` (copy from `config/chrononode.toml.example`)
3. Operator key at `/etc/chrononode/operator_key` (generated via `chrononode init`)
4. BaaLS signing key at `/etc/chrononode/baals.key` (32 bytes raw ed25519 seed)
5. Watch lists imported:
   ```bash
   chrononode-cli watch import --chain bitcoin-light --file config/watchlist-bitcoin.txt
   chrononode-cli watch import --chain dogecoin --file config/watchlist-dogecoin.txt
   ```
6. Optional DOGE API token for higher BlockCypher limits:
   ```bash
   echo 'CHRONONODE_DOGE_API_TOKEN=replace-with-token' | sudo tee -a /etc/chrononode/evm.env
   sudo chmod 600 /etc/chrononode/evm.env
   ```
7. Alternative BTC provider (recommended when Esplora endpoints are rate-limited):
   set `[adapters.bitcoin-light] mode = "rpc"` and `rpc_url` in config, then restart ingest.
8. Alternative DOGE provider (recommended when BlockCypher is rate-limited):
   set `[adapters.dogecoin] mode = "rpc"` and `rpc_url` in config, then restart ingest.

## Install

```bash
# Binary
sudo install -m 0755 target/release/chrononode-cli /usr/local/bin/chrononode-cli

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
sudo cp deploy/systemd/chrononode-anchor@.service /etc/systemd/system/
sudo cp deploy/systemd/chrononode-anchor@.timer /etc/systemd/system/
sudo cp deploy/systemd/chrononode.target /etc/systemd/system/
sudo systemctl daemon-reload

# Enable
sudo systemctl enable --now chrononode-ingest@baals.service
sudo systemctl enable --now chrononode-ingest@bitcoin-light.service
sudo systemctl enable --now chrononode-ingest@dogecoin.service
sudo systemctl enable --now chrononode.service
sudo systemctl enable --now chrononode-anchor@bitcoin-light.timer
sudo systemctl enable --now chrononode-anchor@dogecoin.timer
```

## Operations

```bash
sudo systemctl status chrononode-ingest@baals.service
sudo systemctl status chrononode-ingest@bitcoin-light.service
sudo systemctl status chrononode-ingest@dogecoin.service
sudo systemctl status chrononode.service
sudo systemctl status chrononode-anchor@bitcoin-light.timer
sudo journalctl -u chrononode-ingest@baals.service -f
sudo journalctl -u chrononode.service -f
sudo journalctl -u chrononode-anchor@bitcoin-light.service -f
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
