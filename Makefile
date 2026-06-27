ARTIFACTS_DIR := artifacts

.PHONY: build optimize test clean

build:
	cargo build --target wasm32-unknown-unknown --release
	mkdir -p $(ARTIFACTS_DIR)
	cp target/wasm32-unknown-unknown/release/the_gist_contracts.wasm $(ARTIFACTS_DIR)/
	@echo "Build complete. Artifacts in $(ARTIFACTS_DIR)/"

optimize: build
	@command -v wasm-opt >/dev/null 2>&1 || { echo "wasm-opt not found. Install binaryen: https://github.com/WebAssembly/binaryen"; exit 1; }
	bash scripts/optimize.sh

test:
	cargo test --workspace

clean:
	cargo clean
	rm -rf $(ARTIFACTS_DIR)
