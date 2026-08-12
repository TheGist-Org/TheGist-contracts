# GistVault

`GistVault` is an optional tipping vault. Users can send anonymous tips (in any Soroban
token, e.g. the native XLM SAC) to gist authors; tips accumulate in escrow (this contract's
token balance) until the author claims them.

This contract is fully implemented (not a stub).

## Storage layout

Storage keys are defined by the internal `DataKey` enum, all in **instance** storage:

| Key | Type stored | Purpose |
|---|---|---|
| `DataKey::TokenAddress` | `Address` | The token contract used for tips, set once via `initialize` |
| `DataKey::PendingBalance(Address)` | `i128` | An author's claimable tip balance |
| `DataKey::GistTotalTips(u64)` | `i128` | Running total of tips a specific gist has received |

## Full function reference

### `initialize(env: Env, token: Address)`
Sets the token contract used for tips. Must be called once before any other function.
- **Errors:** panics with `"already initialized"` if called a second time.

### `tip_author(env: Env, tipper: Address, recipient: Address, gist_id: u64, amount: i128)`
Transfers `amount` of the vault's token from `tipper` into escrow (this contract's own
token balance), crediting it against `recipient`'s pending balance and `gist_id`'s running
tip total.
- **Auth:** requires `tipper.require_auth()`.
- **Errors:** panics with `"tip amount must be positive"` if `amount <= 0`.
- **Events:** emits `GistTipped` (see [EVENTS.md](./EVENTS.md)).

### `claim_tips(env: Env, recipient: Address) -> i128`
Claims `recipient`'s full pending tip balance. The balance is zeroed in storage before the
token transfer executes, so a reentrant or repeated call cannot double-spend it.
- **Auth:** requires `recipient.require_auth()`.
- **Output:** the amount claimed.
- **Errors:** panics with `"no pending tips to claim"` if the pending balance is `0`.
- **Events:** emits `TipsClaimed` (see [EVENTS.md](./EVENTS.md)).

### `get_pending_balance(env: Env, author: Address) -> i128`
Returns the author's withdrawable (unclaimed) tip balance. Returns `0` if the author has
never been tipped.

### `get_total_tips_for_gist(env: Env, gist_id: u64) -> i128`
Returns the running total of tips a specific gist has received (this does not reset when
the recipient claims — it is a lifetime counter per `gist_id`, not per author).

## Design notes

- **Anonymity:** the tipper's Stellar address is visible in the tip transaction and the
  `GistTipped` event's `recipient`/`amount` fields, but nothing links the tip to the
  recipient's identity beyond the transaction itself — the same anonymity model as the rest
  of the protocol.
- **Token choice:** the vault is token-agnostic; whichever token address is passed to
  `initialize` is used for all transfers. In production this is expected to be the native
  XLM Stellar Asset Contract (SAC).
- **No overflow:** balance and total updates use `checked_add`, matching the invariant in
  [SECURITY.md](./SECURITY.md).
