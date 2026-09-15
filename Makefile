.PHONY: build test fmt fmt-check lint run dashboard contract-test contract-build clean bench

build:
	cargo build --workspace

test:
	cargo test --workspace

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

run:
	cargo run -p stellartrace-api

dashboard:
	@echo "Serving dashboard/ at http://localhost:8000"
	cd dashboard && python3 -m http.server 8000

contract-test:
	cd contracts/flagged_accounts && cargo test

contract-build:
	cd contracts/flagged_accounts && cargo build --release --target wasm32-unknown-unknown

bench:
	cargo bench -p stellartrace-rules-engine
	cargo bench -p stellartrace-scoring

clean:
	cargo clean
	cd contracts/flagged_accounts && cargo clean
