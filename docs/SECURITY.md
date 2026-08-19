# Security Model — TheGist Contracts

This document covers the security model for all three Soroban contracts:
**GistRegistry**, **GistVault**, and **LocationVerifier**.

Smart contracts on Soroban are immutable once deployed (absent an explicit upgrade mechanism).
Any exploitable bug may result in permanent loss of user funds or data integrity violations.

---

## Authorization Model

### Privilege Levels

| Role | Who | Scope |
|------|-----|-------|
| Admin | Single address set at `initialize` | Can expire any gist, transfer admin role, manage location prefixes |
| Author | Any authenticated Stellar address | Can post, expire, and extend their own gists |
| Public | Anyone | Read-only queries |

### Functions Requiring `require_auth()`

| Contract | Function | Auth Required | Notes |
|----------|----------|---------------|-------|
| GistRegistry | `post_gist` | `author` | Author must sign the transaction |
| GistRegistry | `expire_gist` | `caller` | Only gist author can self-expire |
| GistRegistry | `admin_expire_gist` | `admin` | Verified against stored admin address |
| GistRegistry | `batch_expire` | `admin` | Verified against stored admin address |
| GistRegistry | `extend_gist_ttl` | `gist.author` | Loaded from storage, not caller-supplied |
| GistRegistry | `set_admin` | `current_admin` | Transfers admin role atomically |
| GistVault | `deposit` | `depositor` | User authorizing vault deposit |
| GistVault | `withdraw` | `user` | User authorizing partial vault withdrawal |
| GistVault | `withdraw_all` | `user` | User authorizing full vault withdrawal |
| GistVault | `tip_author` | `tipper` | Tipper authorizing tip payment |
| LocationVerifier | `set_admin` | `current_admin` | Transfers admin role atomically |
| LocationVerifier | `add_allowed_prefix` | `admin` | Verified against stored admin address |
| LocationVerifier | `remove_allowed_prefix` | `admin` | Verified against stored admin address |
| LocationVerifier | `update_boundaries` | `admin` | Verified against stored admin address |
| LocationVerifier | `set_registry_address` | `admin` | Verified against stored admin address |
| LocationVerifier | `set_required_precision` | `admin` | Verified against stored admin address |
| LocationVerifier | `register_verifier` | `admin` | Verified against stored admin address |

### Admin Privilege Separation

- **Admin** can expire any gist but cannot forge gist content or impersonate authors.
- **Admin** cannot withdraw vault funds; GistVault is independent.
- Admin role transfer (`set_admin`) requires the current admin's signature and verifies the
  stored admin address before writing the new one — no one-step takeover by a third party.
- `initialize` can only be called once; subsequent calls panic with "already initialized".

---

## GistRegistry

### State-Changing Functions

| Function | Access | Event Emitted |
|----------|--------|---------------|
| `initialize` | One-time, no auth | None |
| `post_gist` | Author | `GistPosted` |
| `expire_gist` | Author (own gist) | `GistExpired` |
| `admin_expire_gist` | Admin | `GistExpired` |
| `batch_expire` | Admin (≤20 ids) | `GistExpired` × n |
| `extend_gist_ttl` | Author (own gist) | None |
| `set_admin` | Current admin | None |

### Input Validation

- `ipfs_cid` must be non-empty.
- `geohash` must be exactly 7 characters.
- `expiry` must be in the future and ≤ 168 hours (7 days) from the current ledger timestamp.
- `batch_expire` is capped at 20 gist IDs to bound computational cost.
- `get_gists_by_author` limit is capped at 50.
- All timestamp and TTL arithmetic uses `checked_add` / `checked_sub` / `checked_mul`;
  overflow panics rather than wrapping.

### Gist Counter

`GistCount` is incremented with `checked_add(1)` — overflow would panic at `u64::MAX`
(~1.8 × 10¹⁹ gists), which is not a realistic concern.

### Storage

Gist data is stored in **temporary** storage with a TTL tied to the gist's expiry. When a
gist's TTL lapses, Soroban automatically evicts it from state — no stale data accumulates.
Author lists are retained for 30 days after the last write.

---

## GistVault

Implemented. Tips are transferred via the token contract passed to `initialize` and held in
this contract's own token balance until claimed.

### Custody Model

**GistVault is custodial between tip and claim.** From the moment `tip_author` succeeds
until the recipient calls `claim_tips`, the tipped funds sit in the contract's own token
balance — not in the recipient's wallet, and not returnable to the tipper. This is a
deliberate design choice, not an oversight:

- **No recovery path for unclaimed funds.** If a recipient never calls `claim_tips` — a lost
  key, an abandoned account, or simply never getting around to it — those funds have no
  expiry, no admin override, and no path back to the tipper. They sit in the contract's
  balance indefinitely. This version does not implement any admin-recovery or timeout
  mechanism.
- **Why this tradeoff:** an admin-recovery path would mean an admin key can move user funds
  under some condition, which is a strictly weaker trust model than "only the recipient can
  ever claim their own tips." For a protocol whose stated principles include being
  permissionless and not requiring trust in any operator, we chose trustless-but-unrecoverable
  over recoverable-but-admin-trusted. A future version could revisit this (e.g. a long-dated
  timeout that returns unclaimed funds to the tipper), but that is out of scope for the
  current implementation and would need its own design and auth review before shipping.
- **What is bounded:** the *processing* of a tip (the `tip_author` call itself) is
  idempotent and auth-checked — see the Idempotency note in
  [`GIST_VAULT.md`](GIST_VAULT.md#design-notes). What's explicitly *not* bounded is how long
  a recipient can wait before claiming; that's an accepted, permanent characteristic of this
  design, not a bug.

### Invariants

1. **No overflow on tip amounts.** Tip and balance values use `i128`. All arithmetic uses
   `checked_add`.
2. **No double-spend on claim.** `claim_tips` zeroes the pending balance in storage before
   the token transfer executes.
3. **Balance invariant.** At all times:
   `contract_token_balance == Σ pending_tip_balances[author]`
   Any deviation indicates a bug or unexpected direct transfer.

### Required Auth

- `tip_author` requires `tipper.require_auth()`.
- `claim_tips(recipient)` requires `recipient.require_auth()` before any transfer.

---

## LocationVerifier

### State-Changing Functions

| Function | Access | Notes |
|----------|--------|-------|
| `add_allowed_prefix` | Admin | Verified against stored admin address; capped at 50 prefixes |
| `remove_allowed_prefix` | Admin | Verified against stored admin address |
| `update_boundaries` | Admin | Verified against stored admin address |
| `set_registry_address` | Admin | Verified against stored admin address |
| `set_admin` | Current admin | Transfers admin role atomically |

### Validation

- `verify_geohash` performs a pure prefix comparison with no side effects.
- Prefix matching uses fixed-size stack buffers (`[0u8; 64]`); input lengths above 64 bytes are rejected safely without buffer overflow or panic.
- Property-based testing via `proptest` covers geohash parsing, prefix matching, and TTL/expiry calculations to guarantee crash-freedom across arbitrary inputs.
- `add_allowed_prefix` caps the total number of stored prefixes at 50 to prevent unbounded growth and linear-scan degradation in `is_valid_geohash`.

---

## PR Security Review Checklist

See [`.github/PULL_REQUEST_TEMPLATE.md`](../.github/PULL_REQUEST_TEMPLATE.md).

For the full threat model, see [`docs/THREAT_MODEL.md`](THREAT_MODEL.md).
