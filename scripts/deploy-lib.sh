#!/usr/bin/env bash
# Shared helpers for the deployment scripts.
#
# This file is sourced, never executed: scripts/deploy-testnet.sh and
# scripts/verify-deployment.sh both pull the same invoke / parse / validate
# primitives from here so their verification logic cannot drift apart.

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: required command '$1' not found" >&2
    exit 1
  }
}

# Progress goes to stderr so that stdout stays clean wherever these functions
# are captured via command substitution.
log() { printf '%s\n' "$*" >&2; }

die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

is_contract_id() {
  [[ "${1:-}" =~ ^C[A-Z0-9]{55}$ ]]
}

extract_contract_id() {
  grep -oE 'C[A-Z0-9]{55}' | tail -n 1 || true
}

require_contract_id() {
  local value="$1" what="$2"
  is_contract_id "$value" \
    || die "$what produced no valid contract ID (got '${value:-<empty>}')"
}

extract_uint() {
  grep -oE '[0-9]+' | tail -n 1 || true
}

require_uint() {
  local value="$1" what="$2"
  [[ "$value" =~ ^[0-9]+$ ]] \
    || die "$what returned '${value:-<empty>}' — is the contract actually deployed?"
}

extract_int() {
  grep -oE -- '-?[0-9]+' | tail -n 1 || true
}

require_int() {
  local value="$1" what="$2"
  [[ "$value" =~ ^-?[0-9]+$ ]] \
    || die "$what returned '${value:-<empty>}' — is the contract actually deployed?"
}

extract_bool() {
  grep -oE 'true|false' | tail -n 1 || true
}

require_true() {
  local value="$1" what="$2"
  [[ "$value" == "true" ]] || die "$what returned '${value:-<empty>}'"
}

contract_invoke() {
  # contract_invoke SOURCE_NETWORK_ACCOUNT NETWORK CONTRACT_ID [args...]
  local src="$1" network="$2" id="$3"
  shift 3
  stellar contract invoke \
    --id "$id" \
    --source "$src" \
    --network "$network" \
    -- "$@"
}
