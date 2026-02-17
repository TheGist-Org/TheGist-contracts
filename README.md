# GistPin Contracts

This repo contains the **on-chain logic** for GistPin, implemented as
Soroban smart contracts in Rust.

Core responsibilities:

- Register gists as on-chain records
- Index gists by coarse location cell
- Provide verifiable metadata for off-chain indexers
- Future: on-chain tipping, staking, and moderation primitives

---

## Tech Stack

- **Language**: Rust
- **Smart contracts**: Soroban SDK
- **Tooling**:
  - `soroban-cli` for building, deploying, and invoking contracts
  - `cargo` for Rust builds and tests

---

## Project Layout

```bash
gistpin-contracts/
  ├─ src/
  │   └─ lib.rs          # Main contract implementation (GistRegistry, etc.)
  ├─ tests/              # Contract unit / integration tests
  ├─ Cargo.toml
  └─ README.md
```

We may later split multiple contracts into separate crates if needed
(e.g. gistpin-tipping, gistpin-moderation).

Contract Overview
GistRegistry (MVP)

Minimal data model:

• gistid: u64
• author: Option<Address>
• locationcell: u64 (coarse geospatial cell, e.g., geohash/S2-based)
• contenthash: Bytes (IPFS/Arweave CID)
• createdat: u64 (timestamp / ledger sequence)

Core methods (subject to evolution):

• postgist(author, locationcell, contenthash) -> u64
• getgist(gistid) -> GistRecord
• listgistsbycell(locationcell, cursor, limit) -> (Vec<GistRecord>, cursor)

Prerequisites
• Rust toolchain: rustup default stable
• Soroban CLI:
  - Install per official docs, e.g.:
    
    ```bash
    cargo install soroban-cli --locked
    ```

Local Development
Clone the repo

```bash
git clone https://github.com/gistpin/gistpin-contracts.git
cd gistpin-contracts
```

Build

```bash
cargo build
```

Test

```bash
cargo test
```

Run on a local Soroban network

Start a local network (check Soroban docs, example):

```bash
soroban local network start
```

Deploy contract (example):

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/gistpincontracts.wasm \
  --id gistpin-registry \
  --network local
```

Invoke methods (example):

```bash
soroban contract invoke \
  --id gistpin-registry \
  --network local \
  -- \
  postgist \
  --author <ADDRESSORNONE> \
  --locationcell 123456789 \
  --contenthash "bafybeihash..."
```

We will document exact CLI commands and contract IDs as they stabilize.

Contribution Guidelines
• If you are changing contract interfaces, open an issue first and link to
  the relevant design doc (in gistpin-meta).
• Keep public functions as small, explicit, and documented as possible.
• Cover new logic with tests in tests/.

For general contribution rules, see the global
CONTRIBUTING.md.
`

