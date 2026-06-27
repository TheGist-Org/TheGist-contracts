#!/usr/bin/env bash
set -euo pipefail

ARTIFACTS_DIR="artifacts"
INPUT="$ARTIFACTS_DIR/the_gist_contracts.wasm"
OUTPUT="$ARTIFACTS_DIR/the_gist_contracts.optimized.wasm"

if [ ! -f "$INPUT" ]; then
  echo "Error: $INPUT not found. Run 'make build' first."
  exit 1
fi

echo "==> Optimizing WASM with wasm-opt..."
BEFORE=$(du -sh "$INPUT" | cut -f1)
wasm-opt -Oz --strip-debug "$INPUT" -o "$OUTPUT"
AFTER=$(du -sh "$OUTPUT" | cut -f1)

echo "    Before: $BEFORE"
echo "    After:  $AFTER"
echo "==> Optimized WASM written to $OUTPUT"
