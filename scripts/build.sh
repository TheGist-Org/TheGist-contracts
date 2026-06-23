#!/usr/bin/env bash
set -euo pipefail

ARTIFACTS_DIR="artifacts"
WASM="target/wasm32-unknown-unknown/release/the_gist_contracts.wasm"

echo "==> Building contracts (wasm32-unknown-unknown, release)..."
cargo build --target wasm32-unknown-unknown --release

mkdir -p "$ARTIFACTS_DIR"
cp "$WASM" "$ARTIFACTS_DIR/"

echo "==> Build complete"
echo "    Size: $(du -sh "$ARTIFACTS_DIR/the_gist_contracts.wasm" | cut -f1)"
