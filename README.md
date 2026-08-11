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

This repo is a Cargo workspace with one crate per contract, so each contract builds to its
own `.wasm` artifact — a single deployed Soroban contract can only export one contract's
functions, so the three contracts can't share a build.

```bash
cargo build --workspace --target wasm32-unknown-unknown --release
```

Compiled `.wasm` artifacts are output to `target/wasm32-unknown-unknown/release/`:
`gist_registry.wasm`, `gist_vault.wasm`, `location_verifier.wasm`. `make build` copies them
into `artifacts/`.

---

## Test

```bash
cargo test --workspace
```

Unit and integration tests live alongside each contract crate, under
`contracts/<contract>/tests/`. All new contract logic must be covered before opening a PR.

---

## Deploy to Soroban Testnet

Use the scripted flow in [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md).

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

Each contract is its own crate in a Cargo workspace, so it can be built and deployed as an
independent `.wasm` artifact:

```
TheGist-contracts/
├── contracts/
│   ├── gist-registry/
│   │   ├── src/lib.rs        # GistRegistry contract
│   │   └── tests/gist_registry_test.rs
│   ├── gist-vault/
│   │   ├── src/lib.rs        # GistVault tipping contract
│   │   ├── src/gist_vault_test.rs
│   │   └── tests/gist_vault_test.rs
│   └── location-verifier/
│       ├── src/lib.rs        # LocationVerifier contract
│       └── tests/location_verifier_test.rs
├── Cargo.toml                 # workspace root
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
