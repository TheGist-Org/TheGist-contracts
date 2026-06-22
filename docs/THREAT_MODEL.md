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
- GistVault (when implemented) must use `i128::checked_add` for all balance updates and
  reject deposits that would overflow the author's pending balance.

**Residual Risk:** Low in GistRegistry (fully mitigated). GistVault overflow protection
depends on correct implementation.

---

## Threat 4 — Location Boundary Manipulation (Open Write on LocationVerifier)

**Vector:** Any account calls `add_allowed_prefix`, `update_boundaries`, or
`set_registry_address` without authentication, overwriting geohash boundary definitions or
pointing the verifier at a malicious registry.

**Impact:** Region restrictions bypassed; spam gists accepted from any coordinate; or
GistRegistry association corrupted.

**Mitigations (required before production):**
- Add an admin address to LocationVerifier (identical pattern to GistRegistry).
- Gate `add_allowed_prefix`, `update_boundaries`, and `set_registry_address` behind
  `admin.require_auth()` and stored-address verification.

**Residual Risk:** High in current code. These functions are completely unprotected.

---

## Threat 5 — Contract Upgrade Attack

**Vector:** If an upgrade mechanism is added in the future, an attacker who controls the
admin key could deploy a malicious WASM that redefines contract behaviour (e.g., allows
arbitrary withdrawals from GistVault).

**Impact:** Complete contract compromise post-upgrade.

**Mitigations:**
- There is currently **no upgrade function** in any contract; this vector does not exist
  today.
- If an upgrade function is added, it must be gated behind the admin key and ideally a
  time-lock or multi-sig to give users time to verify the new WASM hash.
- All upgrade transactions should be announced off-chain in the project's governance channel
  before execution.
- The new WASM hash should be published and independently verifiable against the source
  build.

**Residual Risk:** Not applicable until an upgrade mechanism is introduced.

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
| GistVault | `withdraw_tips` | Author (intended) | **UNIMPLEMENTED** |
| LocationVerifier | `add_allowed_prefix` | Admin (intended) | **UNPROTECTED** |
| LocationVerifier | `update_boundaries` | Admin (intended) | **UNPROTECTED** |
| LocationVerifier | `set_registry_address` | Admin (intended) | **UNPROTECTED** |

---

## Out of Scope

- Off-chain indexer (TheGist-API) security
- IPFS content availability or integrity
- Client-side key management
- Network-level attacks on the Stellar/Soroban network itself
