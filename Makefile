.PHONY: bench test test-full lint doc install clean validate

# Run criterion benchmarks
bench:
	cargo bench

# Run all tests
test:
	cargo test --workspace

# Run all tests including ignored
test-full:
	cargo test --workspace -- --include-ignored

# Run lints (clippy + fmt)
lint:
	cargo clippy --workspace -- -D warnings
	cargo fmt --all -- --check

# Build and open documentation
doc:
	cargo doc --workspace --no-deps --open

# Install the CLI
install:
	cargo install --path crates/forgetest-cli

# Validate all eval sets
validate:
	cargo run --bin forgetest -- validate --eval-set eval-sets/

# Clean build artifacts
clean:
	cargo clean
