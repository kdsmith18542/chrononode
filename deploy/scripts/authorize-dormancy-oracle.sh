#!/usr/bin/env bash
set -euo pipefail

if ! command -v cast >/dev/null 2>&1; then
  echo "error: 'cast' (Foundry) is required" >&2
  exit 1
fi

: "${REWARD_DISTRIBUTOR:?set REWARD_DISTRIBUTOR to the RewardDistributor contract address}"
: "${CHRONONODE_ORACLE_ADDRESS:?set CHRONONODE_ORACLE_ADDRESS to the ChronoNode operator EVM address}"
: "${ARBITRUM_SEPOLIA_RPC:?set ARBITRUM_SEPOLIA_RPC to an RPC URL}"

TIMELOCK_ACCOUNT="${TIMELOCK_ACCOUNT:-timelock}"
DORMANCY_ROLE="$(cast keccak "DORMANCY_ORACLE_ROLE")"

echo "Granting DORMANCY_ORACLE_ROLE to ${CHRONONODE_ORACLE_ADDRESS} on ${REWARD_DISTRIBUTOR}"
cast send "${REWARD_DISTRIBUTOR}" \
  "grantRole(bytes32,address)" "${DORMANCY_ROLE}" "${CHRONONODE_ORACLE_ADDRESS}" \
  --rpc-url "${ARBITRUM_SEPOLIA_RPC}" \
  --account "${TIMELOCK_ACCOUNT}"

echo "Verifying role assignment..."
HAS_ROLE="$(cast call "${REWARD_DISTRIBUTOR}" \
  "hasRole(bytes32,address)" "${DORMANCY_ROLE}" "${CHRONONODE_ORACLE_ADDRESS}" \
  --rpc-url "${ARBITRUM_SEPOLIA_RPC}")"

echo "hasRole(bytes32,address) => ${HAS_ROLE}"
if [[ "${HAS_ROLE}" == "0x0000000000000000000000000000000000000000000000000000000000000001" ]]; then
  echo "success: ChronoNode oracle is authorized"
else
  echo "warning: role check did not return true; verify account permissions and tx status" >&2
fi
