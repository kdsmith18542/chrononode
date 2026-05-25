#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CHRONONODE_BIN="${CHRONONODE_BIN:-chrononode}"

BTC_FILE="${BTC_WATCHLIST_FILE:-${ROOT_DIR}/config/watchlist-bitcoin.txt}"
DOGE_FILE="${DOGE_WATCHLIST_FILE:-${ROOT_DIR}/config/watchlist-dogecoin.txt}"

if ! command -v "${CHRONONODE_BIN}" >/dev/null 2>&1; then
  echo "error: '${CHRONONODE_BIN}' not found in PATH (set CHRONONODE_BIN if needed)" >&2
  exit 1
fi

if [[ ! -f "${BTC_FILE}" ]]; then
  echo "error: bitcoin watch list file not found: ${BTC_FILE}" >&2
  exit 1
fi

if [[ ! -f "${DOGE_FILE}" ]]; then
  echo "error: dogecoin watch list file not found: ${DOGE_FILE}" >&2
  exit 1
fi

echo "Importing Bitcoin watch list from ${BTC_FILE}"
"${CHRONONODE_BIN}" watch import --chain bitcoin --file "${BTC_FILE}"

echo "Importing Dogecoin watch list from ${DOGE_FILE}"
"${CHRONONODE_BIN}" watch import --chain dogecoin --file "${DOGE_FILE}"

echo "Listing imported Bitcoin addresses"
"${CHRONONODE_BIN}" watch list --chain bitcoin

echo "Listing imported Dogecoin addresses"
"${CHRONONODE_BIN}" watch list --chain dogecoin

echo "success: watch lists imported"
