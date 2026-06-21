#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${ENV_FILE:-$ROOT_DIR/.env.contracts}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "Missing $ENV_FILE. Run scripts/deploy-testnet.sh first." >&2
  exit 1
fi

# shellcheck disable=SC1090
source "$ENV_FILE"

command -v stellar >/dev/null 2>&1 || {
  echo "stellar CLI is required" >&2
  exit 1
}

: "${NETWORK_NAME:?missing NETWORK_NAME}"
: "${DEPLOYER:?missing DEPLOYER}"
: "${GIST_REGISTRY_CONTRACT_ID:?missing GIST_REGISTRY_CONTRACT_ID}"
: "${GIST_VAULT_CONTRACT_ID:?missing GIST_VAULT_CONTRACT_ID}"
: "${LOCATION_VERIFIER_CONTRACT_ID:?missing LOCATION_VERIFIER_CONTRACT_ID}"
: "${DEFAULT_ALLOWED_PREFIX:?missing DEFAULT_ALLOWED_PREFIX}"
: "${DEPLOYER_ADDRESS:?missing DEPLOYER_ADDRESS}"

contract_invoke() {
  local contract_id="$1"
  shift
  stellar contract invoke \
    --id "$contract_id" \
    --source "$DEPLOYER" \
    --network "$NETWORK_NAME" \
    -- "$@"
}

registry_version="$(contract_invoke "$GIST_REGISTRY_CONTRACT_ID" get_version | tail -n 1 | tr -d '\r')"
configured_registry="$(contract_invoke "$LOCATION_VERIFIER_CONTRACT_ID" get_registry_address | tail -n 1 | tr -d '\r')"
vault_balance="$(contract_invoke "$GIST_VAULT_CONTRACT_ID" get_tip_balance --author "$DEPLOYER_ADDRESS" | tail -n 1 | tr -d '\r')"
prefix_ok="$(contract_invoke "$LOCATION_VERIFIER_CONTRACT_ID" verify_geohash --geohash "${DEFAULT_ALLOWED_PREFIX}x" | tail -n 1 | tr -d '\r')"

if [[ "$configured_registry" != "$GIST_REGISTRY_CONTRACT_ID" ]]; then
  echo "LocationVerifier registry mismatch: expected $GIST_REGISTRY_CONTRACT_ID, got $configured_registry" >&2
  exit 1
fi

if [[ "$prefix_ok" != "true" && "$prefix_ok" != "1" ]]; then
  echo "LocationVerifier prefix check failed for $DEFAULT_ALLOWED_PREFIX" >&2
  exit 1
fi

cat <<EOF
Deployment verified
GistRegistry version: $registry_version
LocationVerifier registry: $configured_registry
GistVault balance for deployer: $vault_balance
Allowed prefix: $DEFAULT_ALLOWED_PREFIX
EOF
