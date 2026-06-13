# TheGist Contracts

Soroban smart contracts in Rust — the core on-chain logic of the TheGist protocol.

Every gist posted through TheGist is anchored here. No backend can forge, censor, or silently delete a record stored in these contracts.

---

## Contracts

### GistRegistry

The primary contract. Every gist posted to TheGist is an entry in the GistRegistry.

| Field | Type | Description |
|-------|------|-------------|
| `gist_id` | `u64` | Auto-incrementing on-chain ID |
| `ipfs_cid` | `Bytes` | IPFS content identifier for the gist body |
| `geohash` | `String` | Geohash at precision 7 (~150m × 150m cell) |
| `author` | `Address` | Stellar address of the signing keypair |
| `timestamp` | `u64` | Ledger timestamp at submission |
| `expiry` | `u64` | Expiry timestamp (default: 24h from post) |

All writes require a valid Stellar signature. The contract emits a `GistPosted` event on every successful write, which the indexer (TheGist-API) consumes.

---

### GistVault

An optional tipping vault. Users can send XLM tips to gist authors anonymously via Soroban escrow — no direct wallet-to-wallet transfer required. The author can withdraw accumulated tips at any time. The sender's identity is not linked to the recipient's identity on-chain beyond the transaction itself.

---

### LocationVerifier

Validates that a submitted geohash falls within an allowed geographic boundary. Used to enforce region-scoped deployments or to prevent spam from coordinates that don't correspond to real locations. Boundary definitions are stored as contract data and can be updated by the contract admin.

---

## Prerequisites

- **Rust** — [rustup.rs](https://rustup.rs)
- **wasm32 target**: `rustup target add wasm32-unknown-unknown`
- **Soroban CLI**: `cargo install --locked stellar-cli --features opt`

---

## Build

```bash
cargo build --target wasm32-unknown-unknown --release
```

Compiled `.wasm` artifacts are output to `target/wasm32-unknown-unknown/release/`.

---

## Test

```bash
cargo test
```

Unit and integration tests live in `tests/`. All new contract logic must be covered before opening a PR.

---

## Deploy to Soroban Testnet

```bash
# Configure the testnet network
stellar network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"

# Fund a test identity
stellar keys generate --global alice
stellar keys fund alice --network testnet

# Deploy GistRegistry
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/gist_registry.wasm \
  --source alice \
  --network testnet

# Deploy GistVault
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/gist_vault.wasm \
  --source alice \
  --network testnet

# Deploy LocationVerifier
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/location_verifier.wasm \
  --source alice \
  --network testnet
```

Each command outputs a contract ID. Copy these into the `TheGist-API` `.env` and the web/mobile client environment variables.

---

## Contract Addresses (Testnet)

| Contract | Address |
|----------|---------|
| GistRegistry | `TBD` |
| GistVault | `TBD` |
| LocationVerifier | `TBD` |

> These will be populated after the initial testnet deployment.

---

## Project Layout

```
TheGist-contracts/
├── src/
│   ├── gist_registry.rs     # GistRegistry contract
│   ├── gist_vault.rs        # GistVault tipping contract
│   ├── location_verifier.rs # LocationVerifier contract
│   └── lib.rs               # Crate root
├── tests/
│   ├── gist_registry_test.rs
│   ├── gist_vault_test.rs
│   └── location_verifier_test.rs
├── Cargo.toml
└── README.md
```

---

## Contributing

- If you are changing a public contract interface, open an issue in [theGist-Meta](https://github.com/TheGist-Org/theGist-Meta) before implementing — interface changes affect every client.
- All new behaviour must have test coverage in `tests/`.
- Keep contract functions small, explicit, and free of unnecessary state.

For global contribution rules, see [CONTRIBUTING.md](https://github.com/TheGist-Org/theGist-Meta/blob/main/CONTRIBUTING.md).

---

## License

MIT
