HTML_ENTITIES_URL := https://html.spec.whatwg.org/entities.json
HTML_ENTITIES_JSON := crates/html/data/entities.json
HTML_ENTITIES_GEN := crates/html/src/entities_html5.rs
HTML_ENTITIES_TOOL := crates/html/tools/generate_entities_html5.py

.PHONY: fmt lint lint-html5 test test-html5-legacy test-html5-toggle build build-html5 build-release build-release-html5 run run-workspace run-example ci html-entities-update html-entities-generate html-entities-check

# Format all crates
format:
	cargo fmt --all -- --check

# Lint only (alias for clippy, matches CI)
lint:
	cargo clippy --workspace --all-targets --locked -- -D warnings

# Lint with html5 feature enabled (matches CI)
lint-html5:
	cargo clippy --workspace --all-targets --features html5 --locked -- -D warnings

# Run all tests
test:
	BORROWSER_HTML_PARSER=legacy cargo test --workspace --all-targets --locked

# Run tests with html5 feature enabled but legacy runtime toggle
test-html5-legacy:
	BORROWSER_HTML_PARSER=legacy cargo test --workspace --all-targets --features html5 --locked

# Run tests with html5 feature enabled and html5 runtime toggle
test-html5-toggle:
	BORROWSER_HTML_PARSER=html5 cargo test --workspace --all-targets --features html5 --locked

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

# Update the vendored HTML5 entities snapshot and regenerate Rust table
html-entities-update:
	@command -v curl >/dev/null 2>&1 || (echo "error: curl not found" && exit 1)
	@echo "Fetching HTML5 entities snapshot..."
	@curl -fsSL "$(HTML_ENTITIES_URL)" -o "$(HTML_ENTITIES_JSON)"
	@$(MAKE) html-entities-generate
	@echo "Done. Review git diff and commit the updated snapshot + generated file."

# Regenerate Rust table from the vendored entities.json (no network)
html-entities-generate:
	@command -v python3 >/dev/null 2>&1 || (echo "error: python3 not found" && exit 1)
	@test -f "$(HTML_ENTITIES_JSON)" || (echo "error: missing $(HTML_ENTITIES_JSON)" && exit 1)
	@echo "Generating $(HTML_ENTITIES_GEN)..."
	@python3 "$(HTML_ENTITIES_TOOL)"
	@echo "Generated."

# CI-friendly: ensure generated file matches the vendored snapshot
html-entities-check:
	@command -v python3 >/dev/null 2>&1 || (echo "error: python3 not found" && exit 1)
	@tmp="$$(mktemp)"; \
	HTML5_ENTITIES_OUTPUT="$$tmp" python3 "$(HTML_ENTITIES_TOOL)"; \
	diff -u "$$tmp" "$(HTML_ENTITIES_GEN)" >/dev/null || \
		(echo "error: generated entities table is out of date. Run: make html-entities-generate" && exit 1); \
	rm -f "$$tmp"

build-html5:
	cargo build --workspace --all-targets --features html5 --locked

build-release:
	cargo build --workspace --release --locked

build-release-html5:
	cargo build --workspace --release --features html5 --locked

# Full CI-equivalent pipeline
ci:
	@$(MAKE) format
	@$(MAKE) lint
	@$(MAKE) lint-html5
	@$(MAKE) test
	@$(MAKE) test-html5-legacy
	@$(MAKE) test-html5-toggle
	@$(MAKE) build
	@$(MAKE) build-html5
	@$(MAKE) build-release
	@$(MAKE) build-release-html5
	@$(MAKE) html-entities-check

loc:
	git ls-files | xargs wc -l
