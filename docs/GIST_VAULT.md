# GistVault

`GistVault` is an optional tipping vault, intended to let users send anonymous XLM tips to gist authors, which authors can then withdraw.

## ⚠️ Current implementation status: stub

As of this writing, **`GistVault` is not functional**. Every function exists with its correct public signature, but each body is empty or contains only placeholder logic:

```rust
pub fn __init(env: Env) {
    // Placeholder for initialization logic
}

pub fn deposit_tip(env: Env, gist_id: u64, amount: U256) {
    // Placeholder for deposit logic
}

pub fn withdraw_tips(env: Env, author: Address) {
    // Placeholder for withdrawal logic
}

pub fn get_tip_balance(env: Env, author: Address) -> U256 {
    // Placeholder for balance query
    U256::from_u128(&env, 0)
}
```

No XLM is actually transferred, escrowed, or tracked. `deposit_tip` does not move any funds. `get_tip_balance` always returns `0` regardless of any prior deposits, because nothing is ever stored. `withdraw_tips` does nothing.

This document describes the **intended design** based on the function signatures and the issue requirements, so contributors have a target to implement against — not the current behavior of the deployed contract. Do not rely on this contract for any tipping functionality until it has been implemented and this notice is removed.

## Function reference (signatures only — behavior not yet implemented)

### `__init(env: Env)`
Intended to initialize the vault's storage (e.g. zeroing balances, setting up any admin/config). Currently a no-op.
- **Inputs:** none beyond the environment
- **Output:** none
- **Errors:** none currently (no validation exists since there's no logic)

### `deposit_tip(env: Env, gist_id: u64, amount: U256)`
Intended to let a tipper deposit XLM, associated with a specific gist (and therefore its author), into the vault. Currently does nothing — no funds are moved, no state is recorded.
- **Inputs:** `gist_id: u64` — the gist being tipped; `amount: U256` — the tip amount
- **Output:** none
- **Errors:** none currently — notably, there is no check that `gist_id` actually exists in `GistRegistry`, and no check on `amount` (e.g. no minimum/maximum enforcement), because no logic has been written yet

### `withdraw_tips(env: Env, author: Address)`
Intended to let an author withdraw their accumulated tip balance. Currently does nothing.
- **Inputs:** `author: Address`
- **Output:** none
- **Errors:** none currently

### `get_tip_balance(env: Env, author: Address) -> U256`
Intended to return the author's withdrawable tip balance.
- **Inputs:** `author: Address`
- **Output:** currently **always** `U256::from_u128(&env, 0)`, regardless of input or any prior activity
- **Errors:** none

## Open design questions (for whoever implements this contract)

The issue this documentation was written for asks for the following to be explained, but none of it can be answered honestly from the current code, since the logic doesn't exist yet:

- **How tips flow from tipper to recipient** — not yet defined. Presumably `deposit_tip` would need to either hold XLM in contract-controlled escrow or use a token-transfer call, and associate the amount with the gist's `author` (looked up from `GistRegistry`, implying a cross-contract call that doesn't currently exist anywhere in this codebase).
- **XLM amount limits and why** — not yet defined. No minimum or maximum is enforced anywhere in the current code.
- **Claim process explanation** — not yet defined. `withdraw_tips` takes only an `author` address with no amount, suggesting a "withdraw full balance" design, but this is not confirmed since the body is empty.

**Recommendation:** before this contract is implemented, the design questions above should be resolved (ideally in a design doc or the GitHub issue that tracks `GistVault`'s implementation) and this file should be rewritten to document the actual finished behavior, function by function, in the same style as [`GIST_REGISTRY.md`](./GIST_REGISTRY.md) and [`LOCATION_VERIFIER.md`](./LOCATION_VERIFIER.md).