# Systemd Deployment

These units run ChronoNode as two services per chain:

- `chrononode-ingest@<chain>.service`
- `chrononode-api@<chain>.service`

Example chain IDs:
- `mock`
- `baals`

## Install

1. Install the `chrononode` binary (default path expected by units):

```bash
sudo install -m 0755 target/release/chrononode-cli /usr/local/bin/chrononode
```

2. Create service account and runtime directories:

```bash
sudo useradd --system --home /var/lib/chrononode --create-home --shell /usr/sbin/nologin chrononode || true
sudo install -d -m 0750 -o chrononode -g chrononode /etc/chrononode
sudo install -d -m 0750 -o chrononode -g chrononode /var/lib/chrononode
```

3. Configure environment file:

```bash
sudo cp config/chrononode.env.example /etc/chrononode/chrononode.env
sudo chown root:chrononode /etc/chrononode/chrononode.env
sudo chmod 0640 /etc/chrononode/chrononode.env
```

4. Install unit files:

```bash
sudo cp deploy/systemd/chrononode-ingest@.service /etc/systemd/system/
sudo cp deploy/systemd/chrononode-api@.service /etc/systemd/system/
sudo systemctl daemon-reload
```

5. Enable and start services (example: `mock` chain):

```bash
sudo systemctl enable --now chrononode-ingest@mock.service
sudo systemctl enable --now chrononode-api@mock.service
```

## Operations

```bash
sudo systemctl status chrononode-ingest@mock.service
sudo systemctl status chrononode-api@mock.service
sudo journalctl -u chrononode-ingest@mock.service -f
sudo journalctl -u chrononode-api@mock.service -f
```

Data path used by these units:

```text
/var/lib/chrononode/data/<chain>/
```

## Scale-out Pattern

Keep the same unit files and env layout:
- move `chrononode-ingest@<chain>` to another VPS when needed
- keep `chrononode-api@<chain>` on the API-facing node
