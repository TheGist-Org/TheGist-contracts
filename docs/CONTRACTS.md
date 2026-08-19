# TheGist Contracts — Overview

TheGist is built on three Soroban smart contracts deployed to the Stellar network. This document explains what each contract does, how they relate to one another, and the order they must be deployed and initialized in.

## The three contracts

### 1. GistRegistry

The core contract. Stores "gists" — short, location-tagged posts. Each gist has:
- An IPFS content identifier (`ipfs_cid`) pointing to the actual content
- A 7-character geohash marking where it was posted
- An author address
- A timestamp and an expiry time
- An active/inactive flag

GistRegistry owns the full lifecycle of a gist: posting, querying, manual expiry by the author, admin expiry, batch admin expiry, and TTL extension. It also tracks an admin address that can be transferred and can forcibly expire any gist.

See [`GIST_REGISTRY.md`](./GIST_REGISTRY.md) for the full function reference.

### 2. LocationVerifier

A gatekeeper contract. It stores a list of allowed geohash prefixes and checks whether a given geohash falls inside one of them. It also stores a reference to the `GistRegistry` contract address, so it can be wired into the posting flow as a pre-check.

**Current status:** `LocationVerifier` is fully implemented, but as of this writing it is not yet called automatically by `GistRegistry.post_gist()`. It is a standalone contract that the indexer or client is expected to call before submitting a gist (see [`INTEGRATION.md`](./INTEGRATION.md)).

See [`LOCATION_VERIFIER.md`](./LOCATION_VERIFIER.md) for the full function reference.

### 3. GistVault

An optional tipping vault intended to let users send anonymous XLM tips to gist authors, which authors can later withdraw.

**Current status:** `GistVault` is fully implemented. `tip_author` verifies the gist exists in `GistRegistry` and that the recipient matches the gist's author before transferring tokens into escrow; `claim_tips` lets the author withdraw. An optional protocol fee (up to 10%) can be configured by the admin, atomically splitting each tip between a treasury address and the author. See [`GIST_VAULT.md`](./GIST_VAULT.md) for the full function reference.

## How they interact

- `LocationVerifier` holds a reference to `GistRegistry`'s address via `set_registry_address`. `LocationVerifier.verify_and_post` uses it to cross-contract call `GistRegistry.post_gist` directly, so a client can validate-and-post in a single transaction instead of two separate calls.
- `GistVault` holds a reference to `GistRegistry`'s address via `initialize` (or `set_registry_address`). `tip_author` cross-contract calls `GistRegistry.get_gist(gist_id)` to verify the gist exists, is active, and that the recipient matches the author before accepting a tip.
- Each contract is deployed as its own wasm artifact (see the repo's [Project Layout](../README.md#project-layout)); none of the three share a build.

In short: `GistRegistry` is the source of truth for gist data and is fully self-contained. `LocationVerifier` is a fully working validation step that can optionally post on the caller's behalf. `GistVault` is a fully working, independent tipping add-on.

## Deployment order and initialization sequence

Deploy and initialize in this order:

1. **Deploy `GistRegistry`**, then call `initialize(admin)` once with the admin address that will manage gist moderation. This can only be called once — a second call will panic with `"already initialized"`.
2. **Deploy `LocationVerifier`**, then call `__init()`. This sets up an empty allowed-prefix list if one doesn't already exist (safe to call multiple times).
3. **Wire them together**: call `LocationVerifier.set_registry_address(registry_address)` with the address from step 1, so `LocationVerifier` knows which registry it's paired with.
4. **Configure allowed regions**: call `LocationVerifier.add_allowed_prefix(prefix)` for each geohash prefix you want to allow.
5. **Deploy `GistVault`**, then call `initialize(token, admin, registry)` with the token address (e.g. native XLM SAC), the admin address, and the `GistRegistry` contract ID from step 1. This can only be called once — a second call will panic with `"already initialized"`.

For exact CLI commands and environment setup, see [`DEPLOYMENT.md`](./DEPLOYMENT.md) (existing deployment doc) and [`INTEGRATION.md`](./INTEGRATION.md) for localnet testing steps.