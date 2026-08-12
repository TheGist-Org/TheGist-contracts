#!/usr/bin/env bash
set -euo pipefail

ARTIFACTS_DIR="artifacts"
CONTRACTS=(gist_registry gist_vault location_verifier)

for c in "${CONTRACTS[@]}"; do
  input="$ARTIFACTS_DIR/$c.wasm"
  output="$ARTIFACTS_DIR/$c.optimized.wasm"

  if [ ! -f "$input" ]; then
    echo "Error: $input not found. Run 'make build' first."
    exit 1
  fi

  echo "==> Optimizing $c.wasm with wasm-opt..."
  before=$(du -sh "$input" | cut -f1)
  wasm-opt -Oz --strip-debug "$input" -o "$output"
  after=$(du -sh "$output" | cut -f1)

  echo "    Before: $before"
  echo "    After:  $after"
  echo "==> Optimized WASM written to $output"
done
