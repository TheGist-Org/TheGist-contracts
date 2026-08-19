#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# End-to-end payment flow demo on Soroban testnet.
#
# Exercises the full payment track (#58–#61): deploy contracts, post a gist,
# single tip with idempotency, simulated retry (no double-charge), batch tip,
# claim tips, treasury fee verification, and reconciliation.
#
# Prerequisites:
#   - stellar CLI (cargo install --locked stellar-cli --features opt)
#   - Network access to Soroban testnet RPC
#   - reconcile-vault binary built: cargo build -p reconcile-vault
#
# Usage:
#   ./scripts/demo-payment-flow.sh
#
# The script is idempotent: re-running uses the same contract deployments
# (via .env.demo) so it doesn't redeploy unless the env file is deleted.
# ──────────────────────────────────────────────────────────────────────────────

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_ENV="${ROOT_DIR}/.env.demo"
WASM_DIR="${ROOT_DIR}/target/wasm32v1-none/release"
RECONCILE_BIN="${ROOT_DIR}/target/debug/reconcile-vault"
NETWORK_NAME="testnet"
RPC_URL="https://soroban-testnet.stellar.org"
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

step() { echo -e "\n${CYAN}━━━ Step $1: $2 ━━━${NC}"; }
ok()   { echo -e "${GREEN}✓ $1${NC}"; }
warn() { echo -e "${YELLOW}⚠ $1${NC}"; }
fail() { echo -e "${RED}✗ $1${NC}" >&2; exit 1; }

command -v stellar >/dev/null 2>&1 || fail "stellar CLI not found"

stellar network add "$NETWORK_NAME" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$NETWORK_PASSPHRASE" >/dev/null 2>&1 || true

# ── Step 0: Build (always, to pick up latest contract changes) ───────────────
step 0 "Building contracts and tools"
cd "$ROOT_DIR"
cargo build --workspace --target wasm32v1-none --release 2>&1 | tail -1
cargo build -p reconcile-vault 2>&1 | tail -1
ok "Build complete"

# ── Step 1: Deploy (reuse if .env.demo exists) ──────────────────────────────
step 1 "Deploying contracts to testnet"
if [[ -f "$DEMO_ENV" ]]; then
  warn "Reusing existing deployment from $DEMO_ENV"
  # shellcheck disable=SC1090
  source "$DEMO_ENV"
else
  # Fresh deployment
  DEMO_DEPLOYER="demo-deployer-$(date +%s)"
  stellar keys generate "$DEMO_DEPLOYER" >/dev/null 2>&1 || true
  stellar keys fund "$DEMO_DEPLOYER" --network "$NETWORK_NAME" >/dev/null 2>&1 || true
  sleep 3
  DEMO_DEPLOYER_ADDRESS="$(stellar keys address "$DEMO_DEPLOYER")"

  deploy_contract() {
    local wasm_path="$1"
    stellar contract deploy \
      --wasm "$wasm_path" \
      --source "$DEMO_DEPLOYER" \
      --network "$NETWORK_NAME" \
      | grep -oE 'C[A-Z0-9]{55}' | tail -n 1
  }

  REGISTRY_ID="$(deploy_contract "$WASM_DIR/gist_registry.wasm")"
  VAULT_ID="$(deploy_contract "$WASM_DIR/gist_vault.wasm")"
  VERIFIER_ID="$(deploy_contract "$WASM_DIR/location_verifier.wasm")"
  NATIVE_TOKEN_ID="$(stellar contract asset id --asset native --network "$NETWORK_NAME" | grep -oE 'C[A-Z0-9]{55}' | tail -n 1)"

  stellar contract invoke \
    --id "$REGISTRY_ID" --source "$DEMO_DEPLOYER" --network "$NETWORK_NAME" \
    -- initialize --admin "$DEMO_DEPLOYER_ADDRESS"

  stellar contract invoke \
    --id "$VERIFIER_ID" --source "$DEMO_DEPLOYER" --network "$NETWORK_NAME" \
    -- initialize --admin "$DEMO_DEPLOYER_ADDRESS"

  stellar contract invoke \
    --id "$VAULT_ID" --source "$DEMO_DEPLOYER" --network "$NETWORK_NAME" \
    -- initialize --token "$NATIVE_TOKEN_ID" --admin "$DEMO_DEPLOYER_ADDRESS" --registry "$REGISTRY_ID"

  stellar contract invoke \
    --id "$VERIFIER_ID" --source "$DEMO_DEPLOYER" --network "$NETWORK_NAME" \
    -- set_registry_address --caller "$DEMO_DEPLOYER_ADDRESS" --registry_address "$REGISTRY_ID"

  stellar contract invoke \
    --id "$VERIFIER_ID" --source "$DEMO_DEPLOYER" --network "$NETWORK_NAME" \
    -- add_allowed_prefix --caller "$DEMO_DEPLOYER_ADDRESS" --prefix "u4pruy"

  # Configure 5% protocol fee with treasury
  stellar contract invoke \
    --id "$VAULT_ID" --source "$DEMO_DEPLOYER" --network "$NETWORK_NAME" \
    -- set_fee_bps --admin "$DEMO_DEPLOYER_ADDRESS" --bps 500

  TREASURY_ADDRESS="$DEMO_DEPLOYER_ADDRESS"
  stellar contract invoke \
    --id "$VAULT_ID" --source "$DEMO_DEPLOYER" --network "$NETWORK_NAME" \
    -- set_treasury --admin "$DEMO_DEPLOYER_ADDRESS" --treasury "$TREASURY_ADDRESS"

  cat > "$DEMO_ENV" <<ENVEOF
NETWORK_NAME="$NETWORK_NAME"
RPC_URL="$RPC_URL"
NETWORK_PASSPHRASE="$NETWORK_PASSPHRASE"
DEMO_DEPLOYER="$DEMO_DEPLOYER"
DEMO_DEPLOYER_ADDRESS="$DEMO_DEPLOYER_ADDRESS"
REGISTRY_ID="$REGISTRY_ID"
VAULT_ID="$VAULT_ID"
VERIFIER_ID="$VERIFIER_ID"
NATIVE_TOKEN_ID="$NATIVE_TOKEN_ID"
TREASURY_ADDRESS="$TREASURY_ADDRESS"
ENVEOF

  ok "Deployed: Registry=$REGISTRY_ID  Vault=$VAULT_ID  Verifier=$VERIFIER_ID"
  ok "Fee: 500 bps (5%), Treasury=$TREASURY_ADDRESS"
fi

# ── Step 2: Create and fund tipper + author accounts ─────────────────────────
step 2 "Creating tipper and author accounts"
TIPPER="demo-tipper"
AUTHOR="demo-author"
stellar keys generate "$TIPPER" >/dev/null 2>&1 || true
stellar keys generate "$AUTHOR" >/dev/null 2>&1 || true
stellar keys fund "$TIPPER" --network "$NETWORK_NAME" >/dev/null 2>&1 || true
stellar keys fund "$AUTHOR" --network "$NETWORK_NAME" >/dev/null 2>&1 || true
sleep 3
TIPPER_ADDRESS="$(stellar keys address "$TIPPER")"
AUTHOR_ADDRESS="$(stellar keys address "$AUTHOR")"
ok "Tipper: $TIPPER_ADDRESS"
ok "Author: $AUTHOR_ADDRESS"

# ── Step 3: Post a gist via LocationVerifier ────────────────────────────────
step 3 "Posting a gist via LocationVerifier.verify_and_post"
IPFS_CID="QmDemo$(date +%s)"
GEOHASH="u4pruyd"
GIST_ID="$(stellar contract invoke \
  --id "$VERIFIER_ID" --source "$AUTHOR" --network "$NETWORK_NAME" \
  -- verify_and_post \
    --geohash "$GEOHASH" \
    --ipfs_cid "$IPFS_CID" \
    --author "$AUTHOR_ADDRESS" \
    --ttl_or_expiry "$(($(date +%s) + 86400))" \
  | grep -oE '[0-9]+' | tail -n 1)"
ok "Posted gist #$GIST_ID (ipfs_cid=$IPFS_CID, geohash=$GEOHASH)"

# ── Step 4: Single tip with idempotency key ─────────────────────────────────
step 4 "Sending a single tip via GistVault.tip_author"
TIP_AMOUNT=10_000_000  # 1 XLM (in stroops)
IDEMPOTENCY_KEY="$(printf '0101010101010101010101010101010101010101010101010101010101010101')"

# Record the start ledger for reconciliation
START_LEDGER="$(stellar ledger last --network "$NETWORK_NAME" | grep -oE '[0-9]+' | tail -n 1)"
ok "Start ledger for reconciliation: $START_LEDGER"

stellar contract invoke \
  --id "$VAULT_ID" --source "$TIPPER" --network "$NETWORK_NAME" \
  -- tip_author \
    --tipper "$TIPPER_ADDRESS" \
    --recipient "$AUTHOR_ADDRESS" \
    --gist_id "$GIST_ID" \
    --amount "$TIP_AMOUNT" \
    --idempotency_key "$IDEMPOTENCY_KEY"

# Fee: 5% of 10,000,000 = 500,000 stroops (0.05 XLM)
# Net: 9,500,000 stroops (0.95 XLM)
ok "Tip sent: 10,000,000 stroops → net 9,500,000 (fee 500,000)"

# ── Step 5: Simulate retry (idempotency guard) ──────────────────────────────
step 5 "Simulating a retry with the same idempotency key (no double-charge)"
stellar contract invoke \
  --id "$VAULT_ID" --source "$TIPPER" --network "$NETWORK_NAME" \
  -- tip_author \
    --tipper "$TIPPER_ADDRESS" \
    --recipient "$AUTHOR_ADDRESS" \
    --gist_id "$GIST_ID" \
    --amount "$TIP_AMOUNT" \
    --idempotency_key "$IDEMPOTENCY_KEY"
ok "Retry succeeded (no-op) — idempotency guard prevented double-charge"

# Verify: author's pending balance should still be 9,500,000 (not 19,000,000)
PENDING="$(stellar contract invoke \
  --id "$VAULT_ID" --source "$AUTHOR" --network "$NETWORK_NAME" \
  -- get_pending_balance --recipient "$AUTHOR_ADDRESS" \
  | grep -oE '[0-9]+' | tail -n 1)"
if [[ "$PENDING" != "9500000" ]]; then
  fail "Expected pending=9500000, got $PENDING — double-charge detected!"
fi
ok "Pending balance confirmed: $PENDING stroops (no double-charge)"

# ── Step 6: Batch tip via tip_authors ───────────────────────────────────────
step 6 "Sending a batch tip via GistVault.tip_authors"

# Create a second gist to tip in the batch
IPFS_CID2="QmDemo2$(date +%s)"
GIST_ID2="$(stellar contract invoke \
  --id "$VERIFIER_ID" --source "$AUTHOR" --network "$NETWORK_NAME" \
  -- verify_and_post \
    --geohash "$GEOHASH" \
    --ipfs_cid "$IPFS_CID2" \
    --author "$AUTHOR_ADDRESS" \
    --ttl_or_expiry "$(($(date +%s) + 86400))" \
  | grep -oE '[0-9]+' | tail -n 1)"

BATCH_KEY="$(printf '0202020202020202020202020202020202020202020202020202020202020202')"

# Build the Vec<BatchTipItem> argument as a JSON string for the CLI.
# BatchTipItem = { recipient: Address, gist_id: u64, amount: i128 }
BATCH_ARGS="[{\"recipient\":\"$AUTHOR_ADDRESS\",\"gist_id\":$GIST_ID,\"amount\":5000000},{\"recipient\":\"$AUTHOR_ADDRESS\",\"gist_id\":$GIST_ID2,\"amount\":3000000}]"

stellar contract invoke \
  --id "$VAULT_ID" --source "$TIPPER" --network "$NETWORK_NAME" \
  -- tip_authors \
    --tipper "$TIPPER_ADDRESS" \
    --tips "$BATCH_ARGS" \
    --idempotency_key "$BATCH_KEY"

# Batch totals: 5M + 3M = 8M stroops
# Fee: 5% of 8M = 400,000
# Net: 7,600,000
# Author's total pending after single + batch: 9,500,000 + 7,600,000 = 17,100,000
ok "Batch tip sent: 5,000,000 + 3,000,000 = 8,000,000 stroops"

PENDING_AFTER_BATCH="$(stellar contract invoke \
  --id "$VAULT_ID" --source "$AUTHOR" --network "$NETWORK_NAME" \
  -- get_pending_balance --recipient "$AUTHOR_ADDRESS" \
  | grep -oE '[0-9]+' | tail -n 1)"
ok "Pending balance after batch: $PENDING_AFTER_BATCH stroops"

# ── Step 7: Claim tips and verify treasury ──────────────────────────────────
step 7 "Claiming tips via GistVault.claim_tips"
CLAIM_AMOUNT="$(stellar contract invoke \
  --id "$VAULT_ID" --source "$AUTHOR" --network "$NETWORK_NAME" \
  -- claim_tips --recipient "$AUTHOR_ADDRESS" \
  | grep -oE '[0-9]+' | tail -n 1)"
ok "Claimed: $CLAIM_AMOUNT stroops"

PENDING_AFTER_CLAIM="$(stellar contract invoke \
  --id "$VAULT_ID" --source "$AUTHOR" --network "$NETWORK_NAME" \
  -- get_pending_balance --recipient "$AUTHOR_ADDRESS" \
  | grep -oE '[0-9]+' | tail -n 1)"
ok "Pending after claim: $PENDING_AFTER_CLAIM (expected 0)"

# ── Step 8: Reconciliation ──────────────────────────────────────────────────
step 8 "Running reconciliation tool"
END_LEDGER="$(stellar ledger last --network "$NETWORK_NAME" | grep -oE '[0-9]+' | tail -n 1)"
ok "Ledger range: $START_LEDGER – $END_LEDGER"

if [[ -x "$RECONCILE_BIN" ]]; then
  echo ""
  "$RECONCILE_BIN" "$RPC_URL" "$VAULT_ID" "$START_LEDGER" "$END_LEDGER"
  RECONCILE_EXIT=$?
  echo ""
  if [[ $RECONCILE_EXIT -eq 0 ]]; then
    ok "Reconciliation passed — zero drift"
  else
    fail "Reconciliation detected drift (exit code $RECONCILE_EXIT)"
  fi
else
  warn "reconcile-vault binary not found at $RECONCILE_BIN — skipping automated reconciliation"
  warn "Run manually: cargo run -p reconcile-vault -- $RPC_URL $VAULT_ID $START_LEDGER $END_LEDGER"
fi

# ── Summary ──────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  Demo complete!${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "  Contracts deployed (saved to .env.demo):"
echo "    Registry:  $REGISTRY_ID"
echo "    Vault:     $VAULT_ID"
echo "    Verifier:  $VERIFIER_ID"
echo ""
echo "  Testnet links (look up on Stellar Explorer):"
echo "    Registry: https://stellar.expert/testnet/contract/$REGISTRY_ID"
echo "    Vault:    https://stellar.expert/testnet/contract/$VAULT_ID"
echo "    Verifier: https://stellar.expert/testnet/contract/$VERIFIER_ID"
echo ""
echo "  Transaction hashes captured during this run can be looked up at:"
echo "    https://stellar.expert/testnet/explorer/<TX_HASH>"
echo ""
echo "  To re-run: delete .env.demo and re-execute this script."
echo "  To tear down: contracts are testnet-only and will expire."
