# LocationVerifier

`LocationVerifier` is a gatekeeper contract that checks whether a geohash falls within an allowed set of geographic regions. It is used to enforce region-scoped deployments or to reject gists posted from disallowed locations, before they are submitted to `GistRegistry`.

This contract is fully implemented (not a stub).

## Storage layout

`LocationVerifier` uses two instance storage keys, defined by the internal `DataKey` enum:

| Key | Type stored | Purpose |
|---|---|---|
| `DataKey::RegistryAddress` | `Address` | The `GistRegistry` contract address this verifier is paired with. Optional — may be unset. |
| `DataKey::AllowedPrefixes` | `Vec<String>` | The list of geohash prefixes currently allowed. Starts empty. |

Both are stored in **instance storage** (not temporary), so they persist for the lifetime of the contract instance and are not subject to TTL expiry like `GistRegistry`'s gist data.

## Geohash format explanation

A geohash is a string that encodes a geographic area: the longer the string, the smaller (more precise) the area it represents. This contract works on a **prefix match** basis, not exact match: a geohash is considered allowed if it *starts with* any one of the stored allowed prefixes.

For example, if `"u4p"` is an allowed prefix, then a submitted geohash of `"u4pruyd"` would match, because `"u4pruyd"` starts with `"u4p"`.

The matching is implemented with a fixed-size 64-byte buffer (`matches_prefix`), and explicitly returns `false` if the prefix is longer than the geohash being checked — a prefix can never be longer than the string it's supposed to be a prefix of.

## How to configure allowed regions

There is no single "set the region" call. Instead, you build the allow-list incrementally:

- `add_allowed_prefix(prefix: String)` — appends one new prefix to the existing list. Call this once per region you want to allow. There is no way to remove a single prefix once added; you'd need to use `update_boundaries` to reset the whole list (see below).
- `update_boundaries(boundaries: String)` — **replaces** the entire allowed-prefix list with a single new prefix. Unlike `add_allowed_prefix`, this is destructive: any previously added prefixes are discarded. Despite the plural-sounding name, it only stores one prefix at a time.

In practice: use `add_allowed_prefix` repeatedly during initial setup to build a multi-region allow-list. Use `update_boundaries` only if you intend to wipe the list down to a single region.

## Full function reference

### `__init(env: Env)`
Initializes the contract. If `AllowedPrefixes` has not already been set, initializes it to an empty list. Safe to call more than once — it is a no-op if already initialized.
- **Inputs:** none beyond the environment
- **Output:** none
- **Errors:** none

### `set_registry_address(env: Env, registry_address: Address)`
Stores the `GistRegistry` contract address for later reference.
- **Inputs:** `registry_address: Address`
- **Output:** none
- **Errors:** none. Does not validate that the address is actually a deployed `GistRegistry` contract.

### `get_registry_address(env: Env) -> Option<Address>`
Reads the stored registry address, if one has been set.
- **Inputs:** none beyond the environment
- **Output:** `Some(Address)` if set, `None` if `set_registry_address` was never called
- **Errors:** none

### `add_allowed_prefix(env: Env, prefix: String)`
Appends a new geohash prefix to the allow-list.
- **Inputs:** `prefix: String`
- **Output:** none
- **Errors:** none. No validation on prefix length or format — an empty string or an overly long string will be accepted and stored as-is.

### `verify_geohash(env: Env, geohash: String) -> bool`
Checks whether the given geohash starts with any currently allowed prefix.
- **Inputs:** `geohash: String`
- **Output:** `true` if the geohash matches at least one allowed prefix, `false` otherwise (including when the allow-list is empty)
- **Errors:** none — returns `false` rather than panicking on no match

### `update_boundaries(env: Env, boundaries: String)`
Replaces the entire allowed-prefix list with a single prefix.
- **Inputs:** `boundaries: String`
- **Output:** none
- **Errors:** none. This is destructive — any existing prefixes added via `add_allowed_prefix` are lost.

### `get_boundaries(env: Env) -> String`
Returns the first prefix in the allowed-prefix list.
- **Inputs:** none beyond the environment
- **Output:** the first stored prefix as a `String`, or the literal string `"{}"` if the list is empty
- **Errors:** none

## Cross-contract call flow

As of this writing, `LocationVerifier` does not make any cross-contract calls, and `GistRegistry` does not call into `LocationVerifier`. The `RegistryAddress` stored via `set_registry_address` is currently informational only — it records which registry this verifier is meant to be paired with, but no function in either contract automatically invokes the other.

The intended flow (to be implemented by the calling client or indexer, not by the contracts themselves) is: