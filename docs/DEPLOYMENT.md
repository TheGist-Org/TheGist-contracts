# Deployment

This repository includes two helper scripts for Soroban testnet deployment:

- `scripts/deploy-testnet.sh` deploys all three contracts, wires `LocationVerifier` to `GistRegistry`, initializes the registry admin, and writes contract IDs to `.env.contracts`.
- `scripts/verify-deployment.sh` reloads `.env.contracts`, checks the registry version, verifies the wired registry address, and confirms the default geohash prefix is active.

## Prerequisites

- `rustup target add wasm32-unknown-unknown`
- `cargo install --locked stellar-cli --features opt`
- A funded Soroban testnet keypair for the deployer account

## Deploy

```bash
cp .env.contracts.example .env.contracts
./scripts/deploy-testnet.sh
```

The script:

1. Adds the testnet network if it is not already configured.
2. Builds the contract wasm artifact.
3. Creates or funds the deployer keypair.
4. Deploys `GistRegistry`, `GistVault`, and `LocationVerifier`.
5. Configures `LocationVerifier` with the deployed registry address.
6. Initializes `GistRegistry` with the deployer address as admin.
7. Adds the default geohash prefix.
8. Writes the resulting IDs to `.env.contracts`.

## Verify

```bash
./scripts/verify-deployment.sh
```

The verify script checks:

- `GistRegistry.get_version()`
- `LocationVerifier.get_registry_address()`
- `GistVault.get_tip_balance()`
- `LocationVerifier.verify_geohash()`

## Contract IDs

After a successful deployment, `.env.contracts` should contain:

- `GIST_REGISTRY_CONTRACT_ID`
- `GIST_VAULT_CONTRACT_ID`
- `LOCATION_VERIFIER_CONTRACT_ID`

Keep that file out of version control and commit only `.env.contracts.example`.
