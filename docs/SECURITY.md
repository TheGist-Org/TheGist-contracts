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
| Admin | Single address set at `initialize` | Can expire any gist, transfer admin role |
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

> **Status:** Placeholder implementation. The functions below contain no logic; tip
> accounting is not yet live. This section documents the intended invariants that must be
> enforced when the implementation is completed.

### Intended Invariants

1. **No overflow on tip amounts.** Tip and balance values use `i128` (Stellar native token
   standard). All arithmetic must use checked operations.
2. **No double-spend on withdrawal.** Balances must be zeroed atomically with the transfer in
   a single storage write before the token transfer executes.
3. **Balance invariant.** At all times:
   `contract_token_balance == Σ pending_tip_balances[author]`
   Any deviation indicates a bug or unexpected direct transfer.

### Required Auth (when implemented)

- `withdraw_tips(author)` must call `author.require_auth()` before any transfer.
- `deposit_tip` does not require author auth but must verify the transferred amount matches
  `amount`.

---

## LocationVerifier

### State-Changing Functions

| Function | Access | Notes |
|----------|--------|-------|
| `add_allowed_prefix` | **No auth — open write** | See finding below |
| `update_boundaries` | **No auth — open write** | See finding below |
| `set_registry_address` | **No auth — open write** | See finding below |

> **Finding:** `add_allowed_prefix`, `update_boundaries`, and `set_registry_address` do not
> call `require_auth()`. Any account can currently modify boundary definitions or overwrite
> the registry address. These functions must be gated behind an admin address before
> production deployment.

### Validation

- `verify_geohash` performs a pure prefix comparison with no side effects.
- Prefix matching uses fixed-size stack buffers (`[0u8; 64]`); geohashes or prefixes longer
  than 64 bytes would cause a panic — acceptable given geohash max length is 12.

---

## PR Security Review Checklist

See [`.github/PULL_REQUEST_TEMPLATE.md`](../.github/PULL_REQUEST_TEMPLATE.md).

For the full threat model, see [`docs/THREAT_MODEL.md`](THREAT_MODEL.md).
