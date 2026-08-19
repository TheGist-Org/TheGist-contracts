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
| `DataKey::TipRecord(BytesN<32>)` | `TipRecord` | What a given idempotency key already did, kept in **temporary** storage for 48 hours |

## Full function reference

### `initialize(env: Env, token: Address, admin: Address, registry: Address)`
Sets the token contract, admin address, and GistRegistry address. Must be called once before
any other function.
- **Errors:** panics with `"already initialized"` if called a second time.

### `set_registry_address(env: Env, admin: Address, new_registry: Address)`
Updates the `GistRegistry` address. Only callable by the admin.
- **Auth:** requires `admin.require_auth()`.
- **Errors:** panics with `"caller is not the admin"` if caller is not the stored admin.

### `tip_author(env: Env, tipper: Address, recipient: Address, gist_id: u64, amount: i128, idempotency_key: BytesN<32>)`
Transfers `amount` of the vault's token from `tipper` into escrow (this contract's own
token balance), crediting it against `recipient`'s pending balance and `gist_id`'s running
tip total. **Verifies** that the gist exists in `GistRegistry`, is active, and that
`recipient` matches the gist's author.

`idempotency_key` makes retries safe: a caller should generate one key per logical tip
attempt (e.g. a UUID) and reuse the *same* key if it needs to retry that attempt after an
ambiguous confirmation (RPC timeout, dropped connection, etc.). A repeated call with the
same key and identical `tipper`/`recipient`/`gist_id`/`amount` is a no-op: no second
transfer, no second event, the original outcome stands. The record is kept for 48 hours,
long enough for any realistic retry window; a key reused after that window is treated as a
brand new attempt (see Design notes below for the residual risk this implies).
- **Auth:** requires `tipper.require_auth()` (checked on every call, including no-op retries).
- **Errors:**
  - `"tip amount must be positive"` if `amount <= 0`
  - `"idempotency key reused with different parameters"` if the same key was already used
    with a different `tipper`, `recipient`, `gist_id`, or `amount` — this is either a client
    bug or a collision, not a legitimate retry
  - `"registry address not set"` if `initialize` was not called
  - `"gist not found in GistRegistry"` if the gist_id doesn't exist
  - `"gist is not active"` if the gist has been expired
  - `"recipient does not match gist author"` if the recipient address doesn't match
- **Events:** emits `GistTipped` (see [EVENTS.md](./EVENTS.md)) — not re-emitted on a no-op
  retry.

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

- **Custody model:** `GistVault` is custodial escrow between `tip_author` and `claim_tips` —
  tipped funds sit in the contract's own token balance, not the recipient's wallet, until
  claimed. **There is no recovery path for unclaimed funds in this version**: no expiry, no
  admin override, no way to return them to the tipper if the recipient never claims. This is
  a deliberate tradeoff, not an oversight — an admin-recovery mechanism would mean an admin
  key can move user funds under some condition, which is strictly weaker than "only the
  recipient can ever claim their own tips." Given the protocol's stated permissionless,
  trust-nothing principles, this version chooses trustless-but-unrecoverable over
  recoverable-but-admin-trusted. See [SECURITY.md](./SECURITY.md#custody-model) for the full
  rationale and the residual risk this carries.
- **Idempotency:** `tip_author` requires a caller-supplied `idempotency_key`. Stellar's
  account-sequence-number replay protection only stops the *literal same signed envelope*
  from being submitted twice — it does nothing for the far more common case of a client
  retrying after a timeout with a freshly-signed transaction. The idempotency key closes
  that gap: retries with the same key and args are a safe no-op. Records are stored in
  temporary storage with a 48-hour TTL, so a key reused after that window is treated as a
  new attempt rather than protected forever — an explicit, accepted residual risk rather
  than an unbounded storage commitment.
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
