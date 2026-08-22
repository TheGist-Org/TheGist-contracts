#!/usr/bin/env bash
# Deploy GistRegistry, GistVault, and LocationVerifier to Soroban Testnet.
#
# Hardened against silent partial failures:
#   - a failed friendbot funding is surfaced loudly and confirmed on-chain
#     (balance check) before any deploy is attempted;
#   - every deployed contract ID is validated against the StrKey contract
#     shape before it is used;
#   - each freshly deployed contract must answer a read-only call correctly
#     *before* anything is written to .env.contracts.
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
HORIZON_URL="${HORIZON_URL:-https://horizon-testnet.stellar.org}"
MIN_DEPLOYER_XLM="${MIN_DEPLOYER_XLM:-10}"

command -v stellar >/dev/null 2>&1 || {
  echo "stellar CLI is required" >&2
  exit 1
}
command -v curl >/dev/null 2>&1 || {
  echo "curl is required to confirm the deployer funding" >&2
  exit 1
}

# Progress goes to stderr so that stdout stays clean wherever this script's
# functions are captured via command substitution.
log() { printf '%s\n' "$*" >&2; }
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

cd "$ROOT_DIR"

stellar network add "$NETWORK_NAME" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" >/dev/null 2>&1 || true

for wasm_path in "$GIST_REGISTRY_WASM" "$GIST_VAULT_WASM" "$LOCATION_VERIFIER_WASM"; do
  [[ -f "$wasm_path" ]] || die "missing WASM artifact: $wasm_path"
done

log "building contract WASM artifacts..."
# Exclude the host-only reconcile-vault tool: it links tokio/reqwest and
# cannot be compiled for the wasm32v1-none target.
cargo build --workspace --exclude reconcile-vault --target wasm32v1-none --release >/dev/null

# ---------------------------------------------------------------------------
# Fund the deployer — loudly.
#
# `stellar keys fund` hits friendbot, which returns an error when the target
# account already exists. That specific case is tolerable; any other failure
# is reported. Either way we then *prove* the account exists on-chain and
# holds at least MIN_DEPLOYER_XLM before deploying anything.
# ---------------------------------------------------------------------------
stellar keys generate "$DEPLOYER" >/dev/null 2>&1 || true
DEPLOYER_ADDRESS="$(stellar keys address "$DEPLOYER")"

if ! fund_out="$(stellar keys fund "$DEPLOYER" --network "$NETWORK_NAME" 2>&1)"; then
  log "friendbot funding returned an error (the account may already be funded):"
  printf '%s\n' "$fund_out"
fi

account_exists_on_horizon() {
  curl -sf -m 15 "$HORIZON_URL/accounts/$DEPLOYER_ADDRESS" >/dev/null 2>&1
}

native_balance_xlm() {
  # Horizon returns pretty-printed JSON, so flatten it before matching the
  # native-balance object.
  curl -sf -m 15 "$HORIZON_URL/accounts/$DEPLOYER_ADDRESS" \
    | tr '\n' ' ' \
    | grep -oE '\{[^{}]*"asset_type"[[:space:]]*:[[:space:]]*"native"[^{}]*\}' \
    | grep -oE '"balance"[[:space:]]*:[[:space:]]*"[0-9.]+"' \
    | grep -oE '[0-9.]+' | tail -n 1 || true
}

log "waiting for deployer $DEPLOYER_ADDRESS ($DEPLOYER) to appear on-chain..."
account_ready=false
for _ in $(seq 1 15); do
  if account_exists_on_horizon; then
    account_ready=true
    break
  fi
  sleep 2
done

if [[ "$account_ready" != true ]]; then
  die "deployer account $DEPLOYER_ADDRESS did not appear on $NETWORK_NAME within 30s of the funding attempt. Refusing to deploy with an unfunded account — fund it manually and re-run."
fi

balance="$(native_balance_xlm)"
if [[ -z "$balance" ]] \
  || ! awk -v b="$balance" -v m="$MIN_DEPLOYER_XLM" 'BEGIN { exit !(b + 0 >= m + 0) }'; then
  die "deployer account holds ${balance:-an unknown} XLM balance, below the required minimum of $MIN_DEPLOYER_XLM XLM."
fi
log "deployer funded: $balance XLM available"

# ---------------------------------------------------------------------------
# Deploy contracts — unambiguous ID extraction.
#
# The deploy exit code is captured explicitly and every extracted ID must
# match the StrKey contract shape exactly. A bare `grep | tail -n 1` can pick
# up any C-prefixed 56-char string from error output without failing, which
# is how nonexistent-but-valid-looking IDs ended up in README.md (#67).
# ---------------------------------------------------------------------------
is_contract_id() {
  [[ "${1:-}" =~ ^C[A-Z0-9]{55}$ ]]
}

extract_contract_id() {
  grep -oE 'C[A-Z0-9]{55}' | tail -n 1 || true
}

deploy_contract() {
  local name="$1" wasm_path="$2"
  local out rc=0 id
  log "deploying $name from $(basename "$wasm_path")..."
  out="$(stellar contract deploy \
    --wasm "$wasm_path" \
    --source "$DEPLOYER" \
    --network "$NETWORK_NAME" 2>&1)" || rc=$?
  if [[ $rc -ne 0 ]]; then
    printf '%s\n' "$out" >&2
    die "deploy of $name failed (exit code $rc)"
  fi
  id="$(printf '%s\n' "$out" | extract_contract_id)"
  if ! is_contract_id "$id"; then
    printf 'raw deploy output:\n%s\n' "$out" >&2
    die "deploy of $name produced no valid contract ID (captured: '${id:-<empty>}')"
  fi
  log "$name deployed at $id"
  printf '%s' "$id"
}

GIST_REGISTRY_CONTRACT_ID="$(deploy_contract "GistRegistry" "$GIST_REGISTRY_WASM")"
GIST_VAULT_CONTRACT_ID="$(deploy_contract "GistVault" "$GIST_VAULT_WASM")"
LOCATION_VERIFIER_CONTRACT_ID="$(deploy_contract "LocationVerifier" "$LOCATION_VERIFIER_WASM")"

# Resolve the native token (SAC) address. Newer CLIs expose `contract id asset`
# while older ones only have the deprecated `contract asset id` — support both.
NATIVE_TOKEN_ID="$(stellar contract id asset --asset native --network "$NETWORK_NAME" 2>/dev/null | extract_contract_id || true)"
if ! is_contract_id "$NATIVE_TOKEN_ID"; then
  NATIVE_TOKEN_ID="$(stellar contract asset id --asset native --network "$NETWORK_NAME" | extract_contract_id)"
fi
is_contract_id "$NATIVE_TOKEN_ID" \
  || die "could not resolve the native token (SAC) contract ID (captured: '${NATIVE_TOKEN_ID:-<empty>}')"

# ---------------------------------------------------------------------------
# Initialize
# ---------------------------------------------------------------------------
invoke_contract() {
  local id="$1"
  shift
  stellar contract invoke \
    --id "$id" \
    --source "$DEPLOYER" \
    --network "$NETWORK_NAME" \
    -- "$@"
}

log "initializing contracts..."
invoke_contract "$GIST_REGISTRY_CONTRACT_ID" initialize --admin "$DEPLOYER_ADDRESS"

invoke_contract "$LOCATION_VERIFIER_CONTRACT_ID" initialize --admin "$DEPLOYER_ADDRESS"

invoke_contract "$GIST_VAULT_CONTRACT_ID" initialize \
  --token "$NATIVE_TOKEN_ID" \
  --admin "$DEPLOYER_ADDRESS" \
  --registry "$GIST_REGISTRY_CONTRACT_ID"

invoke_contract "$LOCATION_VERIFIER_CONTRACT_ID" set_registry_address \
  --caller "$DEPLOYER_ADDRESS" \
  --registry_address "$GIST_REGISTRY_CONTRACT_ID"

invoke_contract "$LOCATION_VERIFIER_CONTRACT_ID" add_allowed_prefix \
  --caller "$DEPLOYER_ADDRESS" \
  --prefix "$DEFAULT_ALLOWED_PREFIX"

# ---------------------------------------------------------------------------
# Post-deploy verification — before anything is persisted.
#
# A bad address is caught here, at deploy time, not later by someone reading
# the README (#67).
# ---------------------------------------------------------------------------
log "verifying freshly deployed contracts on-chain..."

registry_version="$(invoke_contract "$GIST_REGISTRY_CONTRACT_ID" get_contract_version | grep -oE '[0-9]+' | tail -n 1 || true)"
[[ "$registry_version" =~ ^[0-9]+$ ]] \
  || die "post-deploy verification failed: GistRegistry.get_contract_version returned '${registry_version:-<empty>}'"

vault_balance="$(invoke_contract "$GIST_VAULT_CONTRACT_ID" get_pending_balance --author "$DEPLOYER_ADDRESS" | grep -oE -- '-?[0-9]+' | tail -n 1 || true)"
[[ "$vault_balance" =~ ^-?[0-9]+$ ]] \
  || die "post-deploy verification failed: GistVault.get_pending_balance returned '${vault_balance:-<empty>}'"

verified_registry="$(invoke_contract "$LOCATION_VERIFIER_CONTRACT_ID" get_registry_address | extract_contract_id)"
[[ "$verified_registry" == "$GIST_REGISTRY_CONTRACT_ID" ]] \
  || die "post-deploy verification failed: LocationVerifier registry mismatch (expected $GIST_REGISTRY_CONTRACT_ID, got '${verified_registry:-<empty>}')"

prefix_ok="$(invoke_contract "$LOCATION_VERIFIER_CONTRACT_ID" verify_geohash --geohash "${DEFAULT_ALLOWED_PREFIX}x" | grep -oE 'true|false' | tail -n 1 || true)"
[[ "$prefix_ok" == "true" ]] \
  || die "post-deploy verification failed: allowed prefix '$DEFAULT_ALLOWED_PREFIX' check returned '${prefix_ok:-<empty>}'"

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

log ""
log "All three contracts deployed, initialized, and verified live:"
log "Saved deployment details to $ENV_FILE"
log "GistRegistry:      $GIST_REGISTRY_CONTRACT_ID (version $registry_version)"
log "GistVault:         $GIST_VAULT_CONTRACT_ID (pending balance $vault_balance)"
log "LocationVerifier:  $LOCATION_VERIFIER_CONTRACT_ID (registry wired, prefix '$DEFAULT_ALLOWED_PREFIX' active)"
