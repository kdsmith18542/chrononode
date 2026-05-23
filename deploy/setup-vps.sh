#!/usr/bin/env bash
# One-shot VPS setup for ChronoNode on Arbitrum Sepolia testnet.
# Run as root on the VPS.
set -euo pipefail

BINARY_PATH="${1:-/media/keith/Data/block/chrononode/chrononode/target/release/chrononode-cli}"
EVM_PRIVATE_KEY="${2:-}"   # hex private key, no 0x prefix

if [[ -z "$EVM_PRIVATE_KEY" ]]; then
  echo "Usage: $0 <binary-path> <evm-private-key-hex>"
  exit 1
fi

echo "=== Creating chrononode user ==="
id chrononode &>/dev/null || useradd --system --no-create-home --shell /usr/sbin/nologin chrononode

echo "=== Installing binary ==="
install -m 755 "$BINARY_PATH" /usr/local/bin/chrononode-cli

echo "=== Creating data directory ==="
mkdir -p /var/lib/chrononode/data
chown -R chrononode:chrononode /var/lib/chrononode
chmod 750 /var/lib/chrononode

echo "=== Installing config ==="
mkdir -p /etc/chrononode
install -m 640 -o root -g chrononode "$(dirname "$0")/../config/chrononode.testnet.toml" /var/lib/chrononode/config.toml

echo "=== Writing EVM key environment file ==="
cat > /etc/chrononode/evm.env << EOF
CHRONONODE_EVM_PRIVATE_KEY=${EVM_PRIVATE_KEY}
CHRONONODE_DATA_DIR=/var/lib/chrononode
EOF
chmod 600 /etc/chrononode/evm.env
chown root:chrononode /etc/chrononode/evm.env

echo "=== Installing systemd service ==="
install -m 644 "$(dirname "$0")/chrononode.service" /etc/systemd/system/chrononode.service
systemctl daemon-reload
systemctl enable chrononode
systemctl start chrononode

echo ""
echo "=== ChronoNode installed ==="
echo "Status:  systemctl status chrononode"
echo "Logs:    journalctl -u chrononode -f"
echo "Config:  /var/lib/chrononode/config.toml"
echo "EVM key: /etc/chrononode/evm.env (chmod 600)"
