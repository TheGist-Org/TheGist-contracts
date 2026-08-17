# Threat Model — TheGist Contracts

## Scope

This threat model covers the three Soroban contracts deployed by the TheGist protocol:
**GistRegistry**, **GistVault**, and **LocationVerifier**.

Contracts are immutable once deployed. Any unmitigated vulnerability is permanent until a
contract upgrade is deployed and adopted by all clients.

---

## Threat 1 — Unauthorized Gist Expiry / Admin Key Compromise

**Vector:** An attacker obtains the admin private key and calls `admin_expire_gist` or
`batch_expire` to censor gists.

**Impact:** Targeted censorship of on-chain gist records. Because gists are stored in
temporary storage, premature expiry is effectively data deletion.

**Mitigations:**
- Admin key should be a hardware-secured or multi-sig Stellar account, never a hot wallet.
- `set_admin` enables transferring admin to a new key without redeploying; use immediately
  if a compromise is suspected.
- All admin actions emit `GistExpired` events, giving the indexer (TheGist-API) an
  auditable trail.
- Authors retain the ability to re-post expired gists independently.

**Residual Risk:** A compromised admin key can expire any gist before its natural TTL. There
is no on-chain multisig for admin actions in the current design.

---

## Threat 2 — Unauthorized GistVault Withdrawal (Double-Spend)

**Vector:** An attacker calls `withdraw_tips` without owning the target author's address, or
exploits a reentrancy pattern to drain the vault.

**Impact:** Theft of accumulated XLM tips from legitimate authors.

**Mitigations (required before production):**
- `withdraw_tips` must call `author.require_auth()` before reading or modifying any balance.
- The pending balance for the author must be set to zero in storage **before** the token
  transfer call (checks-effects-interactions pattern).
- Soroban does not support mid-transaction reentrancy the way EVM does, but the
  checks-effects-interactions ordering must still be followed for correctness.
- Add a balance invariant assertion in tests: after every deposit and withdrawal,
  `contract_balance == Σ pending_balances`.

**Residual Risk:** The current implementation is a placeholder with no logic. These controls
must be implemented and tested before mainnet deployment.

---

## Threat 3 — Integer Overflow in Tip Amounts or TTL Arithmetic

**Vector:** An attacker supplies a very large `amount` to `deposit_tip` or a crafted
`ttl_or_expiry` value to `post_gist` to cause integer overflow, bypassing balance or
expiry checks.

**Impact:** Incorrect balances (vault drain or phantom credits), or gists with arbitrarily
far-future expiry that consume permanent ledger storage.

**Mitigations:**
- All TTL and timestamp arithmetic in GistRegistry uses `checked_add`, `checked_sub`, and
  `checked_mul`; overflow panics rather than wrapping.
- `post_gist` explicitly bounds expiry to ≤ 168 hours from the current ledger timestamp.
- GistVault uses `i128::checked_add` for all pending-balance and gist-total updates, and
  panics on overflow rather than wrapping.

**Residual Risk:** Low in both GistRegistry and GistVault (checked arithmetic throughout).

---

## Threat 4 — Location Boundary Manipulation (Open Write on LocationVerifier)

**Vector:** Any account calls `add_allowed_prefix`, `remove_allowed_prefix`,
`update_boundaries`, or `set_registry_address` without authentication, overwriting geohash
boundary definitions or pointing the verifier at a malicious registry.

**Impact:** Region restrictions bypassed; spam gists accepted from any coordinate; or
GistRegistry association corrupted.

**Mitigations:**
- LocationVerifier has an admin address (identical pattern to GistRegistry), set via
  `initialize`.
- `add_allowed_prefix`, `remove_allowed_prefix`, `update_boundaries`, and
  `set_registry_address` are gated behind `admin.require_auth()` and stored-address
  verification.

**Residual Risk:** Low. These functions are protected.

---

## Threat 5 — Contract Upgrade Attack

**Vector:** An attacker attempts to execute an unauthorized WASM code upgrade on deployed contracts to alter storage layout or administrative logic.

**Impact:** Complete contract compromise post-upgrade.

**Mitigations:**
- Contracts in v1 are strictly **immutable**. There is no `upgrade` function in any contract codebase.
- The unused `ContractUpgradedEvent` has been removed from `GistRegistry` to prevent dead-code confusion.
- Upgrading to a new protocol version requires deploying a new contract instance and announcing the new address off-chain.

**Residual Risk:** Risk is eliminated for v1 since contract code cannot be mutated on-chain.

---

## Privileged Function Summary

| Contract | Function | Privilege | Current Status |
|----------|----------|-----------|----------------|
| GistRegistry | `initialize` | One-time deploy | Protected (can only be called once) |
| GistRegistry | `set_admin` | Current admin | Protected |
| GistRegistry | `admin_expire_gist` | Admin | Protected |
| GistRegistry | `batch_expire` | Admin | Protected |
| GistRegistry | `post_gist` | Author | Protected |
| GistRegistry | `expire_gist` | Author | Protected |
| GistRegistry | `extend_gist_ttl` | Author | Protected |
| GistVault | `claim_tips` | Author | Protected |
| GistVault | `tip_author` | Tipper | Protected |
| LocationVerifier | `add_allowed_prefix` | Admin | Protected |
| LocationVerifier | `remove_allowed_prefix` | Admin | Protected |
| LocationVerifier | `update_boundaries` | Admin | Protected |
| LocationVerifier | `set_registry_address` | Admin | Protected |

---

## Out of Scope

- Off-chain indexer (TheGist-API) security
- IPFS content availability or integrity
- Client-side key management
- Network-level attacks on the Stellar/Soroban network itself
