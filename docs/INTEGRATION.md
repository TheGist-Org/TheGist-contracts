# Integration Guide

This guide explains how external systems — the indexer and the web client — are expected to integrate with the three TheGist contracts, and how to test everything locally in under 15 minutes.

## How the indexer should poll for events

`GistRegistry` publishes two on-chain events: `GistPostedEvent` and `GistExpiredEvent` (see [`GIST_REGISTRY.md`](./GIST_REGISTRY.md#event-reference) for full payload details). The indexer's job is to watch for these events rather than re-scanning all gists on a timer, since functions like `get_active_gist_count` and `get_gists_by_geohash` scan every gist ever created and get more expensive as the dataset grows.

Recommended polling pattern:

1. Track the last processed ledger sequence number.
2. Periodically query the RPC endpoint (`RPC_URL` from your environment) for events emitted by the `GistRegistry` contract ID, filtered by the topic pairs `("gist", "posted")` and `("gist", "expired")`, starting from the last processed ledger.
3. For each `GistPostedEvent`, fetch the full gist record via `GistRegistry.get_gist(gist_id)` if you need full details (the event itself only contains `gist_id`, `author`, `timestamp` — not the geohash or IPFS CID).
4. For each `GistExpiredEvent`, mark the corresponding `gist_id` as expired in your own index, keyed by `expired_by` to distinguish self-expiry from admin/batch expiry if that distinction matters to your application.
5. Update your last-processed-ledger checkpoint only after successfully handling a batch of events, so a crash mid-batch doesn't skip events on restart.

`LocationVerifier` does not currently emit any events — it has no `env.events().publish()` calls anywhere in its implementation. If you need to track changes to the allowed-prefix list, you'll need to poll `get_boundaries()` directly, or request that event emission be added to that contract.

## How the web client calls `post_gist`

There is no function literally named `verify_and_post` in the current contracts — the closest equivalent is calling `LocationVerifier.verify_geohash()` followed by `GistRegistry.post_gist()` as two separate calls, since the contracts do not call each other automatically (see [`CONTRACTS.md`](./CONTRACTS.md#how-they-interact)).

The expected client-side flow:

1. **Compute the geohash** for the user's location, encoded to exactly 7 characters (required by `post_gist`'s validation).
2. **Call `LocationVerifier.verify_geohash(geohash)`** using the `LOCATION_VERIFIER_CONTRACT_ID` from your environment. If this returns `false`, stop here and show the user a "not available in your region" message — do not proceed to step 3.
3. **Upload content to IPFS** and obtain the resulting CID, encoded as `Bytes` for the contract call.
4. **Call `GistRegistry.post_gist(ipfs_cid, geohash, author, ttl_or_expiry)`** using the `GIST_REGISTRY_CONTRACT_ID`. The `author` address must sign/authorize this transaction, since `post_gist` calls `author.require_auth()`.
5. **Handle errors** from `post_gist` — see the [error messages reference](./GIST_REGISTRY.md#error-messages-reference) for the exact panic messages your client should catch and translate into user-facing errors (e.g. `"geohash must be exactly 7 characters"`, `"expiry cannot exceed 168 hours from now"`).
6. **Listen for the `GistPostedEvent`** (or simply use the returned `gist_id` from the transaction result) to confirm success and update the UI.

## How to test against localnet

This repository's deployment scripts target testnet by default, but the same scripts work against a local Soroban network by changing your environment configuration.

1. **Start a local Soroban/Stellar network** (e.g. via the `stellar-cli`'s local network container, or `quickstart` image — refer to the official Soroban local development documentation for the exact container command, since that tooling evolves independently of this repository).
2. **Copy the environment template:**
```bash
   cp .env.contracts.example .env.contracts
```
3. **Edit `.env.contracts`** and change:
   - `NETWORK_NAME` to `"standalone"` (or whatever your local network is named)
   - `RPC_URL` to your local network's RPC endpoint (typically `http://localhost:8000/soroban/rpc` for a default local quickstart setup)
   - `NETWORK_PASSPHRASE` to match your local network's configured passphrase
4. **Run the deployment script:**
```bash
   ./scripts/deploy-testnet.sh
```
   Despite the filename, this script reads its target network from your `.env.contracts` file, so pointing the env vars at localnet is sufficient — no separate localnet-specific script exists in this repo as of this writing.
5. **Verify the deployment:**
```bash
   ./scripts/verify-deployment.sh
```
   This confirms `GistRegistry.get_version()` responds, `LocationVerifier` is correctly wired to the registry address, `GistVault.get_tip_balance()` responds (note: will return `0` regardless of input, since the vault is currently a stub — see [`GIST_VAULT.md`](./GIST_VAULT.md)), and the default geohash prefix (`DEFAULT_ALLOWED_PREFIX`) is active.
6. **Try a manual test post** using the deployed `GIST_REGISTRY_CONTRACT_ID` from your newly written `.env.contracts`, calling `post_gist` with a 7-character geohash that starts with your configured `DEFAULT_ALLOWED_PREFIX` (default is `u4pruy`).

If steps 1–6 complete without errors, you have a working local environment — this should take a new contributor well under 15 minutes if the local network image is already pulled/cached.

## Environment variable reference

From `.env.contracts.example`:

| Variable | Purpose | Example value |
|---|---|---|
| `NETWORK_NAME` | Which Stellar network to target | `testnet` |
| `RPC_URL` | Soroban RPC endpoint for the target network | `https://soroban-testnet.stellar.org` |
| `NETWORK_PASSPHRASE` | Network passphrase, must match the target network exactly | `Test SDF Network ; September 2015` |
| `DEPLOYER` | Local identity/keypair name used by `stellar-cli` to sign deployment transactions | `alice` |
| `DEPLOYER_ADDRESS` | The deployer's public address. Left empty in the example; populated by the deploy script or set manually | *(empty by default)* |
| `DEFAULT_ALLOWED_PREFIX` | The geohash prefix added to `LocationVerifier` automatically during deployment | `u4pruy` |
| `GIST_REGISTRY_CONTRACT_ID` | Deployed `GistRegistry` contract ID. Populated by `deploy-testnet.sh` | *(empty until deployed)* |
| `GIST_VAULT_CONTRACT_ID` | Deployed `GistVault` contract ID. Populated by `deploy-testnet.sh` | *(empty until deployed)* |
| `LOCATION_VERIFIER_CONTRACT_ID` | Deployed `LocationVerifier` contract ID. Populated by `deploy-testnet.sh` | *(empty until deployed)* |

**Important:** `.env.contracts` (without `.example`) contains real deployed contract IDs and should never be committed — only `.env.contracts.example` belongs in version control, per [`DEPLOYMENT.md`](./DEPLOYMENT.md).