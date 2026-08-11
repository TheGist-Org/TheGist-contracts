#!/usr/bin/env bash
set -euo pipefail

ARTIFACTS_DIR="artifacts"
WASM_DIR="target/wasm32-unknown-unknown/release"
CONTRACTS=(gist_registry gist_vault location_verifier)

echo "==> Building contracts (wasm32-unknown-unknown, release)..."
cargo build --workspace --target wasm32-unknown-unknown --release

mkdir -p "$ARTIFACTS_DIR"
for c in "${CONTRACTS[@]}"; do
  cp "$WASM_DIR/$c.wasm" "$ARTIFACTS_DIR/"
done

echo "==> Build complete"
for c in "${CONTRACTS[@]}"; do
  echo "    $c.wasm: $(du -sh "$ARTIFACTS_DIR/$c.wasm" | cut -f1)"
done
