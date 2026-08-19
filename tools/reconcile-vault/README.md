# reconcile-vault

**Reference implementation — not a production indexer.**

This tool cross-checks `GistTipped` and `TipsClaimed` events against derived on-chain
state for the `GistVault` contract. It is intended to prove the payment lifecycle state
machine is correct and give `TheGist-API` a concrete spec to implement against.

## What it does

1. Queries all `GistTipped` and `TipsClaimed` events within a ledger range via the Soroban RPC.
2. Aggregates event data to compute:
   - Per-gist gross tip totals (`GistTotalTips`)
   - Per-author pending balances (`PendingBalance`)
   - Per-author total claims
3. Verifies internal invariants (`amount = fee + net`, pending = net - claimed).
4. Reports any drift and provides the `stellar contract invoke` commands to cross-check
   against live on-chain state.

## What it does NOT do

- It does **not** query contract storage directly (that requires auth, which a third-party
  tool cannot provide).
- It does **not** replace `TheGist-API`'s future indexer.
- It does **not** handle real-time streaming or webhooks.

## Usage

```bash
cargo run -p reconcile-vault -- \
  <RPC_URL> \
  <CONTRACT_ID> \
  <START_LEDGER> \
  <END_LEDGER>
```

Example (testnet):

```bash
cargo run -p reconcile-vault -- \
  https://soroban-testnet.stellar.org \
  CDJHW2LHABCDEF...XYZ \
  100000 \
  100500
```

## Reconciliation invariants

The tool verifies these invariants from event data:

1. **Amount consistency:** For each `GistTipped` event, `amount = fee + net`.
2. **Pending balance:** For each author, `pending = sum(net from tips) - sum(amount from claims)`.

Any mismatch is reported as a drift warning and causes a non-zero exit code.

## On-chain state verification

After running the tool, use the `stellar` CLI to verify against live state:

```bash
# Check a gist's total tips
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  -- get_total_tips_for_gist --gist_id <GIST_ID>

# Check an author's pending balance
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <AUTHOR_ACCOUNT> \
  -- get_pending_balance --recipient <AUTHOR_ADDRESS>
```

Compare the CLI output against the tool's derived state. Any mismatch indicates contract
drift or an indexer bug.

## Scope

This is a **verification harness** for development and auditing. The production indexer
(`TheGist-API`) should implement its own event processing pipeline with proper database
storage, real-time updates, and error handling. This tool validates that the contract's
event emissions are internally consistent and that the reconciliation logic is correct.
