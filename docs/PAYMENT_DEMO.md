# Payment Demo Walkthrough

This document walks through the end-to-end payment flow on Soroban testnet,
step by step, with the exact commands and expected outcomes. It is the
runbook for `scripts/demo-payment-flow.sh`.

> **Who this is for:** Contributors reviewing PR #63 (capstone), future client
> developers (`TheGist-web`/`TheGist-mobile`), and anyone validating the
> payment track (#58–#61) against a live network.

---

## Prerequisites

| Requirement | How to get it |
|-------------|---------------|
| Rust + `wasm32v1-none` target | `rustup target add wasm32v1-none` |
| Stellar CLI | `cargo install --locked stellar-cli --features opt` |
| Testnet access | Automatic (soroban-testnet.stellar.org) |
| No funded accounts needed | Script creates and funds accounts via `stellar keys fund` |

## Quick start

```bash
./scripts/demo-payment-flow.sh
```

The script is fully unattended — no prompts, no manual steps. It takes
~60–90 seconds on a fast connection (testnet consensus + ledger time).

---

## What the demo does (7 steps)

### Step 0: Build

Compiles all three contracts to WASM and the `reconcile-vault` tool.

```bash
cargo build --workspace --target wasm32v1-none --release
cargo build -p reconcile-vault
```

### Step 1: Deploy

Deploys `GistRegistry`, `GistVault`, and `LocationVerifier` to testnet.
Initializes all three contracts and configures:
- `LocationVerifier` with the registry address and a geohash prefix
- `GistVault` with a 5% protocol fee and treasury

If `.env.demo` already exists, the deployment is **reused** (no duplicate
contracts). Delete `.env.demo` to redeploy from scratch.

### Step 2: Create accounts

Uses `stellar keys generate` + `stellar keys fund` to create two funded
testnet accounts: a **tipper** and an **author**. No manual friendbot
visits required.

### Step 3: Post a gist

The author calls `LocationVerifier.verify_and_post` (which internally
calls `GistRegistry.post_gist`). The gist gets:
- A generated IPFS CID
- A geohash (`u4pruyd`) matching the allowed prefix
- A 24-hour TTL

Returns a numeric `gist_id` used in subsequent steps.

### Step 4: Single tip

The tipper calls `GistVault.tip_author` with:
- `tipper`: tipper address
- `recipient`: author address
- `gist_id`: from step 3
- `amount`: 10,000,000 stroops (1 XLM)
- `idempotency_key`: deterministic bytes

**Fee split (5%):**
- Gross: 10,000,000 stroops
- Fee to treasury: 500,000 stroops (0.05 XLM)
- Net to author's pending balance: 9,500,000 stroops (0.95 XLM)

### Step 5: Simulated retry (idempotency guard)

The script re-submits the **exact same call** (same idempotency key,
same parameters). Expected behavior:
- The contract recognizes the duplicate and returns immediately
- **No second transfer** — the author's pending balance stays at
  9,500,000 (not 19,000,000)
- The script verifies this by querying `get_pending_balance`

This proves the idempotency mechanism from #58 works correctly and
prevents double-charging on retries.

### Step 6: Batch tip

The script posts a second gist, then calls `GistVault.tip_authors`
with a batch of two items:
1. 5,000,000 stroops to gist #1
2. 3,000,000 stroops to gist #2

**One `require_auth()`**, one summed transfer (8,000,000 stroops), per-item
fee split:
- Gist #1 fee: 250,000 | net: 4,750,000
- Gist #2 fee: 150,000 | net: 2,850,000
- Total fee: 400,000 | Total net: 7,600,000

Author's pending balance after batch: 9,500,000 + 7,600,000 = 17,100,000

### Step 7: Claim tips

The author calls `GistVault.claim_tips` to withdraw all pending tips
(17,100,000 stroops). The pending balance drops to 0.

### Step 8: Reconciliation

Runs `reconcile-vault` against the ledger range from step 4 to now.
The tool:
1. Fetches all `GistTipped` and `TipsClaimed` events from the RPC
2. Computes per-gist totals and per-author pending balances from events
3. Verifies the invariant: `sum(net) - sum(claimed) == pending`
4. Reports **zero drift** if everything is consistent

---

## Understanding the output

When the demo runs successfully, you'll see:

```
━━━ Step 4: Sending a single tip via GistVault.tip_author ━━━
✓ Tip sent: 10,000,000 stroops → net 9,500,000 (fee 500,000)

━━━ Step 5: Simulating a retry with the same idempotency key ━━━
✓ Retry succeeded (no-op) — idempotency guard prevented double-charge
✓ Pending balance confirmed: 9500000 stroops (no double-charge)

━━━ Step 6: Sending a batch tip via GistVault.tip_authors ━━━
✓ Batch tip sent: 5,000,000 + 3,000,000 = 8,000,000 stroops
✓ Pending balance after batch: 17100000 stroops

━━━ Step 7: Claiming tips via GistVault.claim_tips ━━━
✓ Claimed: 17100000 stroops
✓ Pending after claim: 0 (expected 0)

━━━ Step 8: Running reconciliation tool ━━━
=== GistVault Reconciliation Tool (reference) ===
  Found 4 GistTipped events
  Found 1 TipsClaimed events
  All event-derived invariants hold. No drift detected.
✓ Reconciliation passed — zero drift
```

## Looking up transactions on Stellar Explorer

Every `stellar contract invoke` call produces a transaction hash. You
can look these up at:

```
https://stellar.expert/testnet/explorer/<TX_HASH>
```

The script prints contract IDs and a link template at the end. To find
specific transaction hashes, run the script with verbose output or
check the Stellar Explorer for the contract address (all transactions
for that contract are listed).

---

## Failure modes and how to handle them

| Scenario | What happens | Recovery |
|----------|-------------|----------|
| Testnet congestion | Script may take longer but will succeed (no timeout) | Wait, or re-run |
| Duplicate deployment | `.env.demo` exists → reuses existing contracts | Delete `.env.demo` to deploy fresh |
| Insufficient balance | `stellar keys fund` auto-funds; testnet rarely runs dry | Wait a minute and re-run |
| `tip_authors` Vec arg error | CLI version mismatch | Update stellar CLI: `cargo install --locked stellar-cli --features opt` |

## Re-running the demo

The demo is **idempotent by design**:
- Same idempotency keys → no double-charge (proven in step 5)
- Same gist IDs → no duplicate gists (different IPFS CIDs each run)
- Contract state accumulates correctly across runs

Delete `.env.demo` to start completely fresh with new contracts.

---

## Architecture for client developers

This demo is a CLI simulation of what `TheGist-web` and `TheGist-mobile`
would do. The key differences for a real client:

| Concern | Demo (CLI) | Real client |
|---------|-----------|-------------|
| Account management | `stellar keys generate/fund` | Wallet integration (Freighter, etc.) |
| Signing | `--source` flag | Wallet signs transactions |
| Fee-bump sponsorship | Not used | Sponsor wraps tipper's tx (see DEPLOYMENT.md) |
| Polling | Not needed (CLI waits) | Implement PAYMENT_LIFECYCLE.md state machine |
| Idempotency key | Hardcoded hex | Generate per-tip UUID |
| Error handling | `set -e` aborts | Map panics to user-facing messages (see PAYMENT_LIFECYCLE.md) |
| Reconciliation | `reconcile-vault` CLI tool | TheGist-API indexer |

The `stellar contract invoke` commands in this script map 1:1 to the
contract calls a client would make. The function names, arguments, and
return values are identical.
