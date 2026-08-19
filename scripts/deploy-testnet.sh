#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NETWORK_NAME="${NETWORK_NAME:-testnet}"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
DEPLOYER="${DEPLOYER:-alice}"
DEFAULT_ALLOWED_PREFIX="${DEFAULT_ALLOWED_PREFIX:-u4pruy}"
ENV_FILE="${ENV_FILE:-$ROOT_DIR/.env.contracts}"
WASM_DIR="${WASM_DIR:-$ROOT_DIR/target/wasm32v1-none/release}"
GIST_REGISTRY_WASM="${GIST_REGISTRY_WASM:-$WASM_DIR/gist_registry.wasm}"
GIST_VAULT_WASM="${GIST_VAULT_WASM:-$WASM_DIR/gist_vault.wasm}"
LOCATION_VERIFIER_WASM="${LOCATION_VERIFIER_WASM:-$WASM_DIR/location_verifier.wasm}"

command -v stellar >/dev/null 2>&1 || {
  echo "stellar CLI is required" >&2
  exit 1
}

cd "$ROOT_DIR"

stellar network add "$NETWORK_NAME" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" >/dev/null 2>&1 || true

cargo build --workspace --target wasm32v1-none --release

stellar keys generate "$DEPLOYER" >/dev/null 2>&1 || true
stellar keys fund "$DEPLOYER" --network "$NETWORK_NAME" >/dev/null 2>&1 || true
sleep 3

DEPLOYER_ADDRESS="$(stellar keys address "$DEPLOYER")"

deploy_contract() {
  local wasm_path="$1"
  stellar contract deploy \
    --wasm "$wasm_path" \
    --source "$DEPLOYER" \
    --network "$NETWORK_NAME"
}

GIST_REGISTRY_CONTRACT_ID="$(deploy_contract "$GIST_REGISTRY_WASM" | grep -oE 'C[A-Z0-9]{55}' | tail -n 1)"
GIST_VAULT_CONTRACT_ID="$(deploy_contract "$GIST_VAULT_WASM" | grep -oE 'C[A-Z0-9]{55}' | tail -n 1)"
LOCATION_VERIFIER_CONTRACT_ID="$(deploy_contract "$LOCATION_VERIFIER_WASM" | grep -oE 'C[A-Z0-9]{55}' | tail -n 1)"

NATIVE_TOKEN_ID="$(stellar contract asset id --asset native --network "$NETWORK_NAME" | grep -oE 'C[A-Z0-9]{55}' | tail -n 1)"

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
  -- initialize \
  --admin "$DEPLOYER_ADDRESS"

stellar contract invoke \
  --id "$GIST_VAULT_CONTRACT_ID" \
  --source "$DEPLOYER" \
  --network "$NETWORK_NAME" \
  -- initialize \
  --token "$NATIVE_TOKEN_ID" \
  --admin "$DEPLOYER_ADDRESS" \
  --registry "$GIST_REGISTRY_CONTRACT_ID"

stellar contract invoke \
  --id "$LOCATION_VERIFIER_CONTRACT_ID" \
  --source "$DEPLOYER" \
  --network "$NETWORK_NAME" \
  -- set_registry_address \
  --caller "$DEPLOYER_ADDRESS" \
  --registry_address "$GIST_REGISTRY_CONTRACT_ID"

stellar contract invoke \
  --id "$LOCATION_VERIFIER_CONTRACT_ID" \
  --source "$DEPLOYER" \
  --network "$NETWORK_NAME" \
  -- add_allowed_prefix \
  --caller "$DEPLOYER_ADDRESS" \
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
