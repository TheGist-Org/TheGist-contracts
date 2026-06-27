# GistRegistry

`GistRegistry` is the core contract of TheGist. It stores "gists" — short, location-tagged posts that point to content stored on IPFS — and owns their full lifecycle: posting, querying, expiry, and TTL extension. It also manages a single admin address with moderation privileges.

## Storage layout

Storage keys are defined by the internal `DataKey` enum:

| Key | Storage type | Type stored | Purpose |
|---|---|---|---|
| `DataKey::Admin` | instance | `Address` | The current admin address |
| `DataKey::GistCount` | instance | `u64` | The total number of gists ever posted (used to generate new IDs) |
| `DataKey::ContractVersion` | instance | `u32` | Set on `initialize`, intended to track contract version |
| `DataKey::Gist(u64)` | temporary | `Gist` | One entry per gist, keyed by its `gist_id` |
| `DataKey::AuthorGists(Address)` | temporary | `Vec<u64>` | List of gist IDs posted by a given author |

**Important distinction:** `Admin`, `GistCount`, and `ContractVersion` are stored in **instance** storage, which does not expire. `Gist` records and `AuthorGists` lists are stored in **temporary** storage, which has a TTL (time-to-live) measured in ledgers and must be periodically extended or the data will be evicted by the network — see the TTL section below.

### The `Gist` struct

```rust
pub struct Gist {
    pub gist_id: u64,
    pub ipfs_cid: Bytes,
    pub geohash: String,
    pub author: Address,
    pub timestamp: u64,
    pub expiry: u64,
    pub is_active: bool,
}
```

- `ipfs_cid` — the IPFS content identifier pointing to the actual gist content (not stored on-chain itself)
- `geohash` — must be exactly 7 characters (enforced in `post_gist`)
- `timestamp` — ledger timestamp at the time of posting
- `expiry` — ledger timestamp after which the gist is considered expired
- `is_active` — manually settable to `false` by the author or admin to expire early, independent of the `expiry` timestamp

## TTL and ledger mechanics

Soroban temporary storage entries expire after a number of **ledgers**, not seconds directly. This contract converts between hours, seconds, and ledgers using these constants:

- `LEDGERS_PER_HOUR = 720`
- `SECONDS_PER_HOUR = 3600`
- `DEFAULT_GIST_TTL_HOURS = 24`
- `MAX_GIST_TTL_HOURS = 168` (24 × 7, i.e. 7 days)
- `AUTHOR_LIST_TTL_HOURS = 720` (24 × 30, i.e. 30 days)

When a gist is posted, its temporary storage TTL is set to match its expiry time (converted to ledgers), so the on-chain record will be evicted around the same time it logically expires. The author's gist-ID list is kept alive much longer (30 days) so that historical queries via `get_gists_by_author` keep working even after individual gists have expired and been evicted.

## Full function reference

### `initialize(env: Env, admin: Address)`
Sets the admin address. Intended to be called exactly once, immediately after deployment.
- **Inputs:** `admin: Address`
- **Output:** none
- **Errors:** panics with `"already initialized"` if called a second time

### `get_admin(env: Env) -> Option<Address>`
- **Inputs:** none beyond the environment
- **Output:** `Some(Address)` if initialized, `None` otherwise
- **Errors:** none

### `get_version(env: Env) -> u32`
- **Inputs:** none beyond the environment
- **Output:** always returns the hardcoded value `1`
- **Errors:** none
- **⚠️ Known issue:** Due to a missing closing brace in the source, `get_version`'s body and `get_contract_version`'s body are merged in the compiled contract — `get_contract_version` is effectively unreachable dead code nested inside `get_version`. As written, calling what is intended to be `get_contract_version` will not work as a separate exported function. Treat `get_version` as the only working version-check function for now, and note this as a bug to flag for a future fix rather than relying on `get_contract_version` in integrations.

### `set_admin(env: Env, current_admin: Address, new_admin: Address)`
Transfers admin rights to a new address.
- **Inputs:** `current_admin: Address` (must match the stored admin and provide auth), `new_admin: Address`
- **Output:** none
- **Errors:** panics with `"admin not initialized"` if `initialize` was never called; panics with `"caller is not the current admin"` if `current_admin` doesn't match the stored admin
- **Auth:** requires `current_admin.require_auth()`

### `post_gist(env: Env, ipfs_cid: Bytes, geohash: String, author: Address, ttl_or_expiry: Option<u64>) -> u64`
Creates a new gist record.
- **Inputs:**
  - `ipfs_cid: Bytes` — must not be empty
  - `geohash: String` — must be exactly 7 characters
  - `author: Address` — must provide auth
  - `ttl_or_expiry: Option<u64>` — if `None`, defaults to a 24-hour TTL from now; if provided as a small value it's interpreted contextually by the internal time-resolution logic, and the final resolved expiry must be strictly in the future and no more than 168 hours (7 days) from now
- **Output:** the new `gist_id: u64`
- **Errors:** panics with `"ipfs_cid cannot be empty"`, `"geohash must be exactly 7 characters"`, `"expiry must be in the future"`, or `"expiry cannot exceed 168 hours from now"` depending on which validation fails
- **Auth:** requires `author.require_auth()`
- **Events:** publishes a `GistPostedEvent`

### `get_gist(env: Env, gist_id: u64) -> Option<Gist>`
- **Inputs:** `gist_id: u64`
- **Output:** `Some(Gist)` if it exists and hasn't been evicted from temporary storage, `None` otherwise
- **Errors:** none — never panics, returns `None` for missing/expired-and-evicted gists

### `get_gist_count(env: Env) -> u64`
Returns the total number of gists ever created (including expired ones).
- **Inputs:** none beyond the environment
- **Output:** `u64`
- **Errors:** none

### `get_active_gist_count(env: Env) -> u64`
Iterates over every gist ID from `1` to the current count and tallies how many are both `is_active == true` and not yet past their `expiry`.
- **Inputs:** none beyond the environment
- **Output:** `u64`
- **Errors:** none
- **Note:** this is an O(n) scan over all gists ever created — could become expensive as gist count grows. Be cautious calling this frequently in a production indexer; prefer event-based tracking where possible (see [`INTEGRATION.md`](./INTEGRATION.md)).

### `get_gists_by_author(env: Env, author: Address, limit: u32, offset: u32) -> Vec<u64>`
Returns a paginated list of gist IDs posted by a given author, skipping any that have already been evicted from storage.
- **Inputs:** `author: Address`, `limit: u32` (max 50), `offset: u32`
- **Output:** `Vec<u64>` of gist IDs
- **Errors:** panics with `"limit exceeds maximum of 50"` if `limit > 50`

### `get_gists_by_geohash(env: Env, geohash_prefix: String) -> Vec<u64>`
Returns all gist IDs whose geohash starts with the given prefix. Uses a fixed 12-byte buffer internally for comparison.
- **Inputs:** `geohash_prefix: String`
- **Output:** `Vec<u64>` of matching gist IDs
- **Errors:** none directly, but be aware this is also an O(n) scan over every gist ever created

### `is_gist_active(env: Env, gist_id: u64) -> bool`
- **Inputs:** `gist_id: u64`
- **Output:** `true` only if the gist exists, `is_active` is `true`, and the current ledger timestamp is before `expiry`. Returns `false` for a missing gist, a manually-expired gist, or a gist past its TTL.
- **Errors:** none

### `expire_gist(env: Env, caller: Address, gist_id: u64)`
Lets an author manually expire their own gist early.
- **Inputs:** `caller: Address` (must be the gist's author and provide auth), `gist_id: u64`
- **Output:** none
- **Errors:** panics with `"gist not found"` if the gist doesn't exist; panics with `"only the author can expire this gist"` if `caller` is not the gist's author
- **Auth:** requires `caller.require_auth()`
- **Events:** publishes a `GistExpiredEvent` with `expired_by` set to the caller

### `admin_expire_gist(env: Env, admin: Address, gist_id: u64)`
Lets the admin forcibly expire any gist.
- **Inputs:** `admin: Address` (must match stored admin and provide auth), `gist_id: u64`
- **Output:** none
- **Errors:** panics with `"admin not initialized"` or `"caller is not the admin"` (via `ensure_admin`); panics with `"gist not found"` if the gist doesn't exist
- **Auth:** requires `admin.require_auth()`
- **Events:** publishes a `GistExpiredEvent` with `expired_by` set to the admin

### `batch_expire(env: Env, admin: Address, gist_ids: Vec<u64>) -> u32`
Lets the admin expire up to 20 gists in a single call.
- **Inputs:** `admin: Address`, `gist_ids: Vec<u64>` (max length 20)
- **Output:** `u32` — the count of gists actually expired (gists that don't exist are silently skipped rather than causing a panic)
- **Errors:** panics with `"batch size exceeds maximum of 20"` if `gist_ids.len() > 20`; admin-related panics same as above
- **Auth:** requires `admin.require_auth()`
- **Events:** publishes one `GistExpiredEvent` per gist actually expired

### `extend_gist_ttl(env: Env, gist_id: u64)`
Extends a gist's expiry by another 24 hours, capped so the total lifetime never exceeds 7 days from the original post time.
- **Inputs:** `gist_id: u64`
- **Output:** none
- **Errors:** panics with `"gist not found"`; panics with `"cannot extend an inactive gist"` if `is_active` is `false`; panics with `"gist ttl exceeds maximum of 7 days"` if the extension would push the expiry past the 7-day cap from the original timestamp
- **Auth:** requires the gist's own author to provide auth (read from the stored gist record, not passed as a parameter)

## Error messages reference

| Message | Thrown by | When |
|---|---|---|
| `"already initialized"` | `initialize` | Called more than once |
| `"admin not initialized"` | `set_admin`, `ensure_admin` (used by `admin_expire_gist`, `batch_expire`) | `initialize` was never called |
| `"caller is not the current admin"` | `set_admin` | `current_admin` doesn't match stored admin |
| `"caller is not the admin"` | `ensure_admin` | Caller doesn't match stored admin |
| `"gist not found"` | `expire_gist`, `admin_expire_gist`, `extend_gist_ttl` | `gist_id` doesn't exist in storage |
| `"only the author can expire this gist"` | `expire_gist` | Caller is not the gist's author |
| `"batch size exceeds maximum of 20"` | `batch_expire` | More than 20 gist IDs passed |
| `"cannot extend an inactive gist"` | `extend_gist_ttl` | Gist's `is_active` is already `false` |
| `"gist ttl exceeds maximum of 7 days"` | `extend_gist_ttl` | Extension would exceed the 7-day cap |
| `"ipfs_cid cannot be empty"` | `post_gist` | Empty `ipfs_cid` passed |
| `"geohash must be exactly 7 characters"` | `post_gist` | Geohash isn't exactly 7 chars |
| `"expiry must be in the future"` | `post_gist` | Resolved expiry is in the past or now |
| `"expiry cannot exceed 168 hours from now"` | `post_gist` | Resolved expiry is more than 7 days out |
| `"limit exceeds maximum of 50"` | `get_gists_by_author` | `limit > 50` |

## Event reference

`GistRegistry` publishes two event types via `env.events().publish()`. Both events use the topic tuple `(symbol_short!("gist"), symbol_short!(<action>))`.

### `GistPostedEvent`
- **Topic:** `("gist", "posted")`
- **Payload:**
```rust
  pub struct GistPostedEvent {
      pub gist_id: u64,
      pub author: Address,
      pub timestamp: u64,
  }
```
- **Emitted by:** `post_gist`, once per successful post

### `GistExpiredEvent`
- **Topic:** `("gist", "expired")`
- **Payload:**
```rust
  pub struct GistExpiredEvent {
      pub gist_id: u64,
      pub expired_by: Address,
  }
```
- **Emitted by:** `expire_gist`, `admin_expire_gist`, and `batch_expire` (once per gist actually expired in the batch). `expired_by` is the author in the self-expire case, or the admin in the admin/batch case.

**Note on `ContractUpgradedEvent`:** `lib.rs` re-exports a type called `ContractUpgradedEvent` from this module, but no such struct or emitting function exists in `gist_registry.rs` as currently written. This appears to be aspirational/future-facing — there is no upgrade mechanism implemented yet. Do not document this as a working event until it actually exists in the source.