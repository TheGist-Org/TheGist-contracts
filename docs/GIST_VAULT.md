# GistVault

`GistVault` is an optional tipping vault. Users can send anonymous tips (in any Soroban
token, e.g. the native XLM SAC) to gist authors; tips accumulate in escrow (this contract's
token balance) until the author claims them.

This contract is fully implemented (not a stub).

## Storage layout

Storage keys are defined by the internal `DataKey` enum, all in **instance** storage:

| Key | Type stored | Purpose |
|---|---|---|
| `DataKey::Admin` | `Address` | The admin address, set once via `initialize` |
| `DataKey::TokenAddress` | `Address` | The token contract used for tips, set once via `initialize` |
| `DataKey::RegistryAddress` | `Address` | The `GistRegistry` contract address, set once via `initialize` |
| `DataKey::PendingBalance(Address)` | `i128` | An author's claimable tip balance |
| `DataKey::GistTotalTips(u64)` | `i128` | Running total of tips a specific gist has received |

## Full function reference

### `initialize(env: Env, token: Address, admin: Address, registry: Address)`
Sets the token contract, admin address, and GistRegistry address. Must be called once before
any other function.
- **Errors:** panics with `"already initialized"` if called a second time.

### `set_registry_address(env: Env, admin: Address, new_registry: Address)`
Updates the `GistRegistry` address. Only callable by the admin.
- **Auth:** requires `admin.require_auth()`.
- **Errors:** panics with `"caller is not the admin"` if caller is not the stored admin.

### `tip_author(env: Env, tipper: Address, recipient: Address, gist_id: u64, amount: i128)`
Transfers `amount` of the vault's token from `tipper` into escrow (this contract's own
token balance), crediting it against `recipient`'s pending balance and `gist_id`'s running
tip total. **Verifies** that the gist exists in `GistRegistry`, is active, and that
`recipient` matches the gist's author.
- **Auth:** requires `tipper.require_auth()`.
- **Errors:**
  - `"tip amount must be positive"` if `amount <= 0`
  - `"registry address not set"` if `initialize` was not called
  - `"gist not found in GistRegistry"` if the gist_id doesn't exist
  - `"gist is not active"` if the gist has been expired
  - `"recipient does not match gist author"` if the recipient address doesn't match
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

- **Gist verification:** `tip_author` cross-contract calls `GistRegistry.get_gist(gist_id)`
  to verify the gist exists, is active, and that `recipient` matches the gist's author. This
  prevents tips from being attributed to nonexistent or wrong gists, keeping
  `get_total_tips_for_gist` reliable as a public signal.
- **Anonymity:** the tipper's Stellar address is visible in the tip transaction and the
  `GistTipped` event's `recipient`/`amount` fields, but nothing links the tip to the
  recipient's identity beyond the transaction itself — the same anonymity model as the rest
  of the protocol.
- **Token choice:** the vault is token-agnostic; whichever token address is passed to
  `initialize` is used for all transfers. In production this is expected to be the native
  XLM Stellar Asset Contract (SAC).
- **No overflow:** balance and total updates use `checked_add`, matching the invariant in
  [SECURITY.md](./SECURITY.md).
- **Admin model:** the admin is set at initialization and can update the registry address
  via `set_registry_address`. There is no mechanism to change the admin or token after
  initialization — redeploy if those need to change.
