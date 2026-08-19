# Payment Lifecycle

This document defines the payment status state machine for TheGist tipping, bridging
Soroban's on-chain finality model to application-level state. It specifies the exact
client-side polling contract required to distinguish confirmed outcomes from ambiguous ones,
and enumerates every failure mode the contracts can produce.

> **Scope:** This is a specification for clients and the future `TheGist-API` indexer.
> Nothing in this document changes contract behavior.

---

## State machine

```
                        ┌──────────────┐
                        │   SUBMITTED  │
                        └──────┬───────┘
                               │
              ┌────────────────┼────────────────┐
              ▼                ▼                 ▼
     CONFIRMED_SUCCESS  CONFIRMED_FAILED   UNKNOWN_TIMEOUT
              │                │                 │
              ▼                ▼                 │
        Terminal          Terminal              │
                                             ┌──┴──────────────────────┐
                                             │                         │
                                    Poll / retry              Resolved by
                                    (same idempotency key)    polling
                                             │                         │
                                             ▼                         ▼
                                   CONFIRMED_SUCCESS          CONFIRMED_FAILED
                                   or CONFIRMED_FAILED        or CONFIRMED_SUCCESS
```

### State definitions

| State | Meaning |
|-------|---------|
| **SUBMITTED** | The client has sent the transaction to the Soroban RPC but has not yet received a definitive result. |
| **CONFIRMED_SUCCESS** | The transaction was included in a ledger and the contract call succeeded (no revert). The on-chain state has been mutated. |
| **CONFIRMED_FAILED** | The transaction was included in a ledger but the contract call reverted. The on-chain state has **not** been mutated (Soroban reverts atomically). |
| **UNKNOWN_TIMEOUT** | The RPC did not return a definitive result within the polling window. The transaction may or may not have landed. |

### Transitions

- **SUBMITTED → CONFIRMED_SUCCESS:** `getTransaction` returns `status: SUCCESS`.
- **SUBMITTED → CONFIRMED_FAILED:** `getTransaction` returns `status: FAILED` (with `result.code` in the XDR).
- **SUBMITTED → UNKNOWN_TIMEOUT:** The client's polling window expires without a definitive result from `getTransaction`.
- **UNKNOWN_TIMEOUT → CONFIRMED_SUCCESS / CONFIRMED_FAILED:** A subsequent poll with the same transaction hash resolves the state.
- **UNKNOWN_TIMEOUT → SUBMITTED (retry):** The client re-submits using the same idempotency key (safe per #58). If the original transaction landed, the retry is a no-op; if it didn't, the retry submits a new transaction.

---

## Client-side polling contract

### Step 1: Submit the transaction

Submit the Soroban transaction via `sendTransaction`. Capture:
- **Transaction hash** (`txHash`) — returned by the RPC on submission.
- **Idempotency key** — the `BytesN<32>` passed to `tip_author` or `claim_tips`.

### Step 2: Poll for confirmation

Use `getTransaction` with the transaction hash. Poll at the following cadence:

| Phase | Poll interval | Max attempts | Total window |
|-------|--------------|--------------|--------------|
| Initial | 4 seconds (≈1 ledger) | 10 | ~40 seconds |
| Extended (if still unknown) | 8 seconds | 15 | ~120 seconds |
| **Total timeout** | — | — | **~160 seconds** |

The initial phase covers ~10 ledgers (Soroban target is 5s ledger time; 4s poll catches
most). The extended phase handles node lag or transient RPC issues.

> **Why 160 seconds?** Stellar's fast-finality model finalizes in ~2 rounds (~10s). A
> transaction that hasn't landed in 160s almost certainly hit a fee-bump issue, sequence
> conflict, or was dropped. The client should treat this as `UNKNOWN_TIMEOUT` and either
> retry with the idempotency key or prompt the user.

### Step 3: Interpret the result

```
getTransaction response:
├── status: "SUCCESS"  → CONFIRMED_SUCCESS. Done.
├── status: "FAILED"   → CONFIRMED_FAILED. Decode result XDR for the revert reason.
├── status: "NOT_FOUND"
│   ├── attempts < max → Continue polling (transient).
│   └── attempts >= max → UNKNOWN_TIMEOUT. Decide: retry or abort.
└── error (5xx)        → Transient RPC error. Retry the query itself (do NOT treat as NOT_FOUND).
```

### Distinguishing transient RPC errors from genuine NOT_FOUND

| RPC response | Client action |
|-------------|---------------|
| HTTP 500, 502, 503, 504 | Retry the same `getTransaction` call. Do **not** increment the NOT_FOUND counter. |
| HTTP 200 with `"status": "NOT_FOUND"` | This is a genuine "not found" — the transaction hash is unknown to this node. Continue polling up to `max` attempts. |
| HTTP 200 with `"status": "SUCCESS"` or `"FAILED"` | Terminal state reached. |

> **Why the distinction matters:** A 5xx means the RPC node is temporarily unable to
> answer, not that the transaction doesn't exist. Treating a 5xx as NOT_FOUND would cause
> premature `UNKNOWN_TIMEOUT` and unnecessary retries.

### Step 4: Retry with idempotency key (UNKNOWN_TIMEOUT only)

If the state resolves to `UNKNOWN_TIMEOUT` and the user wants to retry:

1. Re-submit the transaction with the **same idempotency key** and **identical parameters**.
2. The contract will either:
   - Return the original outcome (if the first transaction landed) — **no double-spend**.
   - Execute the tip as a new transaction (if the first transaction never landed).
3. **Never change the idempotency key on retry** — this would bypass idempotency and risk a double-spend.

### Step 5: Ledger reorg safety

Soroban's consensus model has extremely low reorg probability (~0). However, the polling
logic should still use a **confirmed-ledger threshold** for safety:

- After `getTransaction` returns `SUCCESS`, wait **2 additional ledgers** (~10s) before
  marking the state as `CONFIRMED_SUCCESS` in the application layer.
- This provides a buffer against the theoretical case where a node reports a transaction as
  included in a ledger that is later orphaned.

In practice this threshold can be set to 0 for TheGist (tipping is low-value, fast-finality
is sufficient), but it should be stated as a configurable parameter in the indexer.

---

## On-chain state vs. event state (reconciliation)

The payment lifecycle creates two parallel sources of truth:

| Source | What it records | When it updates |
|--------|----------------|-----------------|
| **Contract storage** (`get_pending_balance`, `get_total_tips_for_gist`) | Current claimable balance and lifetime gross tips per gist | Immediately on successful `tip_author` / `claim_tips` |
| **Events** (`GistTipped`, `TipsClaimed`) | Historical record of every tip and claim | Emitted at the same moment as storage updates |

**Invariant:** For a given gist, the sum of `amount` fields in all `GistTipped` events for
that gist must equal `get_total_tips_for_gist(gist_id)`. For a given author, the sum of
`net` fields in all `GistTipped` events for that author minus the sum of `amount` fields in
all `TipsClaimed` events for that author must equal `get_pending_balance(author)`.

A mismatch indicates a contract bug or an indexer error. The reconciliation tool
(`tools/reconcile-vault`) verifies these invariants.

---

## Complete panic/failure reference (GistVault)

Every message the `GistVault` contract can produce, organized by function. Clients should
map these to user-facing messages.

### `initialize`

| Panic message | Condition | User-facing suggestion |
|--------------|-----------|----------------------|
| `"already initialized"` | Contract was already initialized | "This vault is already set up. No action needed." |

### `set_fee_bps`

| Panic message | Condition | User-facing suggestion |
|--------------|-----------|----------------------|
| `"caller is not the admin"` | Non-admin called the function | "Only the vault admin can change the fee rate." |
| `"fee exceeds maximum of 1000 bps (10%)"` | bps > 1000 | "The fee cannot exceed 10% (1000 basis points)." |

### `set_treasury`

| Panic message | Condition | User-facing suggestion |
|--------------|-----------|----------------------|
| `"caller is not the admin"` | Non-admin called the function | "Only the vault admin can set the treasury address." |

### `tip_author`

| Panic message | Condition | User-facing suggestion |
|--------------|-----------|----------------------|
| `"tip amount must be positive"` | amount ≤ 0 | "Tip amount must be greater than zero." |
| `"idempotency key reused with different parameters"` | Same key, different tipper/recipient/gist_id/amount | "A tip with this reference ID already exists with different details. Use a new reference ID." |
| `"gist not found in GistRegistry"` | gist_id doesn't exist on-chain | "This gist hasn't been posted to TheGist yet." |
| `"gist is not active"` | gist has expired | "This gist is no longer active and cannot receive tips." |
| `"recipient does not match gist author"` | recipient ≠ gist.author | "Tips can only be sent to the gist's author." |
| `"fee_bps > 0 but no treasury configured"` | Fee enabled but no treasury set | "The vault is not fully configured. Please report this issue." |
| `"fee calculation overflow"` | Amount × bps overflows i128 | "Tip amount is too large. Try a smaller amount." |
| `"fee calculation division error"` | Division error in fee calc | "Internal error computing the fee. Please try again." |
| `"fee exceeds tip amount"` | Fee > amount (should never happen with valid bps) | "Internal error: fee exceeds tip. Please try again." |
| `"pending balance overflow"` | Pending balance + net overflows i128 | "Author's pending balance is too large. Claim first." |
| `"gist total overflow"` | Gist total + amount overflows i128 | "This gist has received too many tips. Claim first." |
| `"registry address not set"` | Vault not initialized | "The vault is not configured. Please report this issue." |

### `claim_tips`

| Panic message | Condition | User-facing suggestion |
|--------------|-----------|----------------------|
| `"no pending tips to claim"` | Pending balance is zero | "No tips available to claim." |
| `"vault not initialized"` | Token address not set | "The vault is not configured. Please report this issue." |

---

## Complete panic/failure reference (GistRegistry)

| Panic message | Function | User-facing suggestion |
|--------------|----------|----------------------|
| `"already initialized"` | `initialize` | "This registry is already set up." |
| `"caller is not the admin"` | `initialize`, `set_admin` | "Only the admin can perform this action." |
| `"admin not initialized"` | `set_admin` | "The registry has not been initialized." |
| `"caller is not the current admin"` | `set_admin` | "Only the current admin can transfer admin." |
| `"gist not found"` | `expire_gist`, `admin_expire_gist`, `extend_gist_ttl` | "This gist doesn't exist on-chain." |
| `"only the author can expire this gist"` | `expire_gist` | "Only the gist author can expire it." |
| `"batch size exceeds maximum of 20"` | `batch_expire` | "Batch size cannot exceed 20 gists." |
| `"cannot extend an inactive gist"` | `extend_gist_ttl` | "An expired gist cannot be extended." |
| `"gist ttl exceeds maximum of 7 days"` | `extend_gist_ttl` | "Gist total lifetime cannot exceed 7 days." |
| `"gist already expired"` | `extend_gist_ttl` | "This gist has already expired." |
| `"ipfs_cid cannot be empty"` | `post_gist` | "The IPFS content ID cannot be empty." |
| `"geohash must be exactly 7 characters"` | `post_gist` | "Geohash must be exactly 7 characters." |
| `"expiry must be in the future"` | `post_gist`, `gist_time_to_expiry` | "Expiry must be set in the future." |
| `"expiry cannot exceed 168 hours from now"` | `post_gist` | "Gist expiry cannot exceed 7 days from now." |
| `"limit exceeds maximum of 50"` | `get_gists_by_author`, `get_gists_by_geohash` | "Query limit cannot exceed 50." |
| `"ttl too large"` | internal | "Internal error: TTL value too large." |

---

## Event schema quick reference

See [EVENTS.md](./EVENTS.md) for full event definitions.

| Event | Topics | Key data fields |
|-------|--------|----------------|
| `GistTipped` | `[vault, tipped]` | `gist_id`, `recipient`, `amount` (gross), `fee`, `net` |
| `TipsClaimed` | `[vault, claimed]` | `recipient`, `amount` (claimed) |
| `GistPosted` | `[gist, posted]` | `gist_id`, `author`, `timestamp` |
| `GistExpired` | `[gist, expired]` | `gist_id`, `expired_by` |

---

## Reconciliation invariants

For a given gist `g` and author `a`:

```
sum(tipped.amount WHERE gist_id = g) == get_total_tips_for_gist(g)
sum(tipped.net WHERE recipient = a) - sum(claimed.amount WHERE recipient = a) == get_pending_balance(a)
```

The first invariant checks gross tip tracking. The second checks net claimable balance.
Both must hold at all times. The reconciliation tool verifies these invariants against a
live or testnet deployment.
