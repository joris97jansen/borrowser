.PHONY: fmt lint clippy test build run run-workspace run-example ci

# Format all crates
fmt:
	cargo fmt --all -- --check

# Lint only (alias for clippy, matches CI)
lint:
	cargo clippy --workspace --all-targets --locked -- -D warnings

# Explicit clippy target (same as lint, for clarity)
clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings

# Run all tests
test:
	cargo test --workspace --all-targets --locked

# Build all targets (debug)
build:
	cargo build --workspace --all-targets --locked

run:
	cargo run

# Run the main browser binary (debug)
run-workspace:
	cargo run --workspace

# Run a specific example (usage: make run-example EXAMPLE=foo)
run-example:
	cargo run --example $(EXAMPLE)

# Full CI-equivalent pipeline
ci:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --locked -- -D warnings
	cargo test --workspace --all-targets --locked
	cargo build --workspace --all-targets --locked
