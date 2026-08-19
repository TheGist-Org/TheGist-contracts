# Deployment

This repository includes two helper scripts for Soroban testnet deployment:

- `scripts/deploy-testnet.sh` deploys all three contracts, wires `LocationVerifier` to `GistRegistry`, initializes the registry admin, and writes contract IDs to `.env.contracts`.
- `scripts/verify-deployment.sh` reloads `.env.contracts`, checks the registry version, verifies the wired registry address, and confirms the default geohash prefix is active.

## Prerequisites

- `rustup target add wasm32v1-none`
- `cargo install --locked stellar-cli --features opt`
- A funded Soroban testnet keypair for the deployer account

## Deploy

```bash
cp .env.contracts.example .env.contracts
./scripts/deploy-testnet.sh
```

The script:

1. Adds the testnet network if it is not already configured.
2. Builds all three contract wasm artifacts (one crate per contract).
3. Creates or funds the deployer keypair.
4. Deploys `GistRegistry`, `GistVault`, and `LocationVerifier` as three separate contract
   instances, each from its own wasm.
5. Initializes `GistRegistry` and `LocationVerifier` with the deployer address as admin, and
   `GistVault` with the native XLM token (SAC) address.
6. Configures `LocationVerifier` with the deployed registry address.
7. Adds the default geohash prefix.
8. Writes the resulting IDs to `.env.contracts`.

## Verify

```bash
./scripts/verify-deployment.sh
```

The verify script checks:

- `GistRegistry.get_version()`
- `LocationVerifier.get_registry_address()`
- `GistVault.get_pending_balance()`
- `LocationVerifier.verify_geohash()`

## Contract IDs

After a successful deployment, `.env.contracts` should contain:

- `GIST_REGISTRY_CONTRACT_ID`
- `GIST_VAULT_CONTRACT_ID`
- `LOCATION_VERIFIER_CONTRACT_ID`

Keep that file out of version control and commit only `.env.contracts.example`.

## Fee-bump sponsorship (gasless tips)

A sponsoring account can wrap a user's tip transaction in a Stellar **fee-bump transaction**
so the tipper doesn't need to hold XLM for gas. This is the recommended pattern for
production clients:

1. The **sponsor** account pays the network fee and submits the fee-bump transaction.
2. The **tipper** signs only the inner transaction (the `tip_author` or `tip_authors` call).
3. The fee-bump wrapper is submitted by the sponsor's account, so the tipper never needs
   a funded account for gas — only for the tip amount itself.

### Example (Stellar CLI)

```bash
# 1. Build the inner transaction (tipper signs this)
stellar tx build \
  --source <TIPPER_ACCOUNT> \
  --network testnet \
  -- \
  contract invoke \
    --id <VAULT_CONTRACT_ID> \
    --source <TIPPER_ACCOUNT> \
    -- \
    tip_author \
      --tipper <TIPPER_ADDRESS> \
      --recipient <AUTHOR_ADDRESS> \
      --gist_id <GIST_ID> \
      --amount <AMOUNT> \
      --idempotency_key <KEY>

# 2. Sign with the tipper's key
stellar tx sign --source <TIPPER_SECRET_KEY>

# 3. Wrap in a fee-bump and submit via the sponsor
stellar tx bump-fee \
  --base-fee 100_000 \
  --sponsor <SPONSOR_ACCOUNT> \
  --network testnet

stellar tx send --network testnet
```

> **Note:** This repo defines and demonstrates the pattern via the existing
> deploy/verify scripts. Actually running a sponsor service is out of scope — it is an
> application-level concern for `TheGist-web` / `TheGist-mobile` to implement.
