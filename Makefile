ARTIFACTS_DIR := artifacts
WASM_DIR := target/wasm32-unknown-unknown/release
CONTRACTS := gist_registry gist_vault location_verifier

.PHONY: build optimize test clean

build:
	cargo build --workspace --target wasm32-unknown-unknown --release
	mkdir -p $(ARTIFACTS_DIR)
	$(foreach c,$(CONTRACTS),cp $(WASM_DIR)/$(c).wasm $(ARTIFACTS_DIR)/;)
	@echo "Build complete. Artifacts in $(ARTIFACTS_DIR)/"

optimize: build
	@command -v wasm-opt >/dev/null 2>&1 || { echo "wasm-opt not found. Install binaryen: https://github.com/WebAssembly/binaryen"; exit 1; }
	bash scripts/optimize.sh

test:
	cargo test --workspace

clean:
	cargo clean
	rm -rf $(ARTIFACTS_DIR)
