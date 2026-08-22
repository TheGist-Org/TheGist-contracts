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

# shellcheck source=deploy-lib.sh
source "$ROOT_DIR/scripts/deploy-lib.sh"

require_cmd stellar

: "${NETWORK_NAME:?missing NETWORK_NAME}"
: "${DEPLOYER:?missing DEPLOYER}"
: "${GIST_REGISTRY_CONTRACT_ID:?missing GIST_REGISTRY_CONTRACT_ID}"
: "${GIST_VAULT_CONTRACT_ID:?missing GIST_VAULT_CONTRACT_ID}"
: "${LOCATION_VERIFIER_CONTRACT_ID:?missing LOCATION_VERIFIER_CONTRACT_ID}"
: "${DEFAULT_ALLOWED_PREFIX:?missing DEFAULT_ALLOWED_PREFIX}"
: "${DEPLOYER_ADDRESS:?missing DEPLOYER_ADDRESS}"

registry_version="$(contract_invoke "$DEPLOYER" "$NETWORK_NAME" "$GIST_REGISTRY_CONTRACT_ID" get_version | extract_uint)"
require_uint "$registry_version" "GistRegistry.get_version"

configured_registry="$(contract_invoke "$DEPLOYER" "$NETWORK_NAME" "$LOCATION_VERIFIER_CONTRACT_ID" get_registry_address | extract_contract_id)"
require_contract_id "$configured_registry" "LocationVerifier.get_registry_address"

vault_balance="$(contract_invoke "$DEPLOYER" "$NETWORK_NAME" "$GIST_VAULT_CONTRACT_ID" get_pending_balance --author "$DEPLOYER_ADDRESS" | extract_int)"
require_int "$vault_balance" "GistVault.get_pending_balance"

prefix_ok="$(contract_invoke "$DEPLOYER" "$NETWORK_NAME" "$LOCATION_VERIFIER_CONTRACT_ID" verify_geohash --geohash "${DEFAULT_ALLOWED_PREFIX}x" | extract_bool)"

if [[ "$configured_registry" != "$GIST_REGISTRY_CONTRACT_ID" ]]; then
  die "LocationVerifier registry mismatch: expected $GIST_REGISTRY_CONTRACT_ID, got '$configured_registry'"
fi

require_true "$prefix_ok" "LocationVerifier.verify_geohash for allowed prefix '$DEFAULT_ALLOWED_PREFIX'"

cat <<EOF
Deployment verified
GistRegistry version: $registry_version
LocationVerifier registry: $configured_registry
GistVault balance for deployer: $vault_balance
Allowed prefix: $DEFAULT_ALLOWED_PREFIX
EOF
