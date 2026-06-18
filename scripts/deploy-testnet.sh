#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NETWORK_NAME="${NETWORK_NAME:-testnet}"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
DEPLOYER="${DEPLOYER:-alice}"
DEFAULT_ALLOWED_PREFIX="${DEFAULT_ALLOWED_PREFIX:-u4pruy}"
ENV_FILE="${ENV_FILE:-$ROOT_DIR/.env.contracts}"
WASM_PATH="${WASM_PATH:-$ROOT_DIR/target/wasm32-unknown-unknown/release/the_gist_contracts.wasm}"
GIST_REGISTRY_WASM="${GIST_REGISTRY_WASM:-$WASM_PATH}"
GIST_VAULT_WASM="${GIST_VAULT_WASM:-$WASM_PATH}"
LOCATION_VERIFIER_WASM="${LOCATION_VERIFIER_WASM:-$WASM_PATH}"

command -v stellar >/dev/null 2>&1 || {
  echo "stellar CLI is required" >&2
  exit 1
}

cd "$ROOT_DIR"

stellar network add "$NETWORK_NAME" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" >/dev/null 2>&1 || true

cargo build --target wasm32-unknown-unknown --release

stellar keys generate --global "$DEPLOYER" >/dev/null 2>&1 || true
stellar keys fund "$DEPLOYER" --network "$NETWORK_NAME"

DEPLOYER_ADDRESS="$(stellar keys address "$DEPLOYER")"

deploy_contract() {
  local wasm_path="$1"
  stellar contract deploy \
    --wasm "$wasm_path" \
    --source "$DEPLOYER" \
    --network "$NETWORK_NAME"
}

GIST_REGISTRY_CONTRACT_ID="$(deploy_contract "$GIST_REGISTRY_WASM" | tail -n 1 | tr -d '\r')"
GIST_VAULT_CONTRACT_ID="$(deploy_contract "$GIST_VAULT_WASM" | tail -n 1 | tr -d '\r')"
LOCATION_VERIFIER_CONTRACT_ID="$(deploy_contract "$LOCATION_VERIFIER_WASM" | tail -n 1 | tr -d '\r')"

stellar contract invoke \
  --id "$LOCATION_VERIFIER_CONTRACT_ID" \
  --source "$DEPLOYER" \
  --network "$NETWORK_NAME" \
  -- set_registry_address \
  --registry_address "$GIST_REGISTRY_CONTRACT_ID"

stellar contract invoke \
  --id "$GIST_REGISTRY_CONTRACT_ID" \
  --source "$DEPLOYER" \
  --network "$NETWORK_NAME" \
  -- initialize \
  --admin "$DEPLOYER_ADDRESS"

stellar contract invoke \
  --id "$LOCATION_VERIFIER_CONTRACT_ID" \
  --source "$DEPLOYER" \
  --network "$NETWORK_NAME" \
  -- add_allowed_prefix \
  --prefix "$DEFAULT_ALLOWED_PREFIX"

cat > "$ENV_FILE" <<EOF
NETWORK_NAME="$NETWORK_NAME"
RPC_URL="$RPC_URL"
NETWORK_PASSPHRASE="$NETWORK_PASSPHRASE"
DEPLOYER="$DEPLOYER"
DEPLOYER_ADDRESS="$DEPLOYER_ADDRESS"
DEFAULT_ALLOWED_PREFIX="$DEFAULT_ALLOWED_PREFIX"
GIST_REGISTRY_CONTRACT_ID="$GIST_REGISTRY_CONTRACT_ID"
GIST_VAULT_CONTRACT_ID="$GIST_VAULT_CONTRACT_ID"
LOCATION_VERIFIER_CONTRACT_ID="$LOCATION_VERIFIER_CONTRACT_ID"
EOF

echo "Saved deployment details to $ENV_FILE"
echo "GistRegistry: $GIST_REGISTRY_CONTRACT_ID"
echo "GistVault: $GIST_VAULT_CONTRACT_ID"
echo "LocationVerifier: $LOCATION_VERIFIER_CONTRACT_ID"
