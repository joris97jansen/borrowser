HTML_ENTITIES_URL := https://html.spec.whatwg.org/entities.json
HTML_ENTITIES_JSON := crates/html/data/entities.json
HTML_ENTITIES_GEN := crates/html/src/entities_html5.rs
HTML_ENTITIES_TOOL := crates/html/tools/generate_entities_html5.py

.PHONY: format fmt-check lint lint-html5 lint-html5-hardening test test-html5-legacy test-html5-toggle test-html5-dom-golden test-html5-patch-golden test-html5-smoke-real-pages test-html5-tokenizer-fuzz-corpus test-html5-tokenizer-fuzz-smoke test-html5-tokenizer-fuzz-long test-html5-tree-builder-token-fuzz-corpus test-html5-tree-builder-token-fuzz-smoke test-html5-tree-builder-token-fuzz-long test-html5-pipeline-fuzz-corpus test-html5-pipeline-regressions test-html5-pipeline-fuzz-smoke test-html5-pipeline-fuzz-long print-html5-pipeline-regression-snapshot test-wpt-tree-builder build build-html5 build-release build-release-html5 run run-workspace run-example ci html-entities-update html-entities-generate html-entities-check cuc cuc-diff

# Format all crates in place
format:
	cargo fmt --all

# Check formatting only (matches CI)
fmt-check:
	cargo fmt --all -- --check

# Lint only (alias for clippy, matches CI)
lint:
	cargo clippy --workspace --all-targets --locked -- -D warnings

# Lint with html5 feature enabled (matches CI)
lint-html5:
	cargo clippy --workspace --all-targets --features html5 --locked -- -D warnings

# Lint the html5 hardening/fuzz feature set (matches CI)
lint-html5-hardening:
	cargo clippy --workspace --all-targets --features "html5 html5-fuzzing parser_invariants" --locked -- -D warnings

# Run all tests
test:
	BORROWSER_HTML_PARSER=legacy cargo test --workspace --all-targets --locked

# Run tests with html5 feature enabled but legacy runtime toggle
test-html5-legacy:
	BORROWSER_HTML_PARSER=legacy cargo test --workspace --all-targets --features html5 --locked

# Run tests with html5 feature enabled and html5 runtime toggle
test-html5-toggle:
	BORROWSER_HTML_PARSER=html5 cargo test --workspace --all-targets --features html5 --locked

# Run HTML5 semantic DOM golden fixtures (whole/chunked/fuzz)
test-html5-dom-golden:
	cargo test -p html --test html5_golden_tree_builder --features "html5 dom-snapshot" --locked

# Run HTML5 patch-log golden fixtures (whole/chunked/fuzz)
test-html5-patch-golden:
	cargo test -p html --test html5_golden_tree_builder_patches --features html5 --locked

# Run HTML5 smoke corpus with real-world-style pages/snippets
test-html5-smoke-real-pages:
	cargo test -p html --test html5_smoke_real_pages --features "html5 dom-snapshot" --locked

# Replay the committed HTML5 tokenizer fuzz corpus deterministically outside libFuzzer
test-html5-tokenizer-fuzz-corpus:
	cargo test -p html --features html5 --lib --locked \
		html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_tokenizer_corpus_deterministically

# Run a short deterministic tokenizer fuzz smoke against the actual fuzz target
test-html5-tokenizer-fuzz-smoke:
	bash ./tools/ci/html5_tokenizer_fuzz_smoke.sh

# Run a longer deterministic tokenizer fuzz lane for nightly/manual use
test-html5-tokenizer-fuzz-long:
	HTML5_TOKENIZER_FUZZ_LABEL='html5 tokenizer fuzz nightly' \
	HTML5_TOKENIZER_FUZZ_ARTIFACT_BASENAME='html5_tokenizer_fuzz_failure_nightly' \
	HTML5_TOKENIZER_FUZZ_SMOKE_SEED=2718281828 \
	HTML5_TOKENIZER_FUZZ_SMOKE_RUNS=20000 \
	HTML5_TOKENIZER_FUZZ_SMOKE_INPUT_TIMEOUT_SEC=10 \
	HTML5_TOKENIZER_FUZZ_SMOKE_WALL_TIMEOUT_SEC=600 \
	bash ./tools/ci/html5_tokenizer_fuzz_smoke.sh

# Replay the committed HTML5 tree-builder synthetic-token fuzz corpus deterministically outside libFuzzer
test-html5-tree-builder-token-fuzz-corpus:
	cargo test -p html --features html5 --lib --locked \
		html5::tree_builder::fuzz::tests::corpus::replay_committed_tree_builder_token_corpus_deterministically

# Run a short deterministic synthetic-token fuzz smoke against the actual tree-builder fuzz target
test-html5-tree-builder-token-fuzz-smoke:
	bash ./tools/ci/html5_tree_builder_tokens_fuzz_smoke.sh

# Run a longer deterministic synthetic-token fuzz lane for nightly/manual use
test-html5-tree-builder-token-fuzz-long:
	HTML5_TREE_BUILDER_TOKEN_FUZZ_LABEL='html5 tree-builder token fuzz nightly' \
	HTML5_TREE_BUILDER_TOKEN_FUZZ_ARTIFACT_BASENAME='html5_tree_builder_token_fuzz_failure_nightly' \
	HTML5_TREE_BUILDER_TOKEN_FUZZ_SMOKE_SEED=1618033988 \
	HTML5_TREE_BUILDER_TOKEN_FUZZ_SMOKE_RUNS=20000 \
	HTML5_TREE_BUILDER_TOKEN_FUZZ_SMOKE_INPUT_TIMEOUT_SEC=10 \
	HTML5_TREE_BUILDER_TOKEN_FUZZ_SMOKE_WALL_TIMEOUT_SEC=600 \
	bash ./tools/ci/html5_tree_builder_tokens_fuzz_smoke.sh

# Replay the committed HTML5 end-to-end pipeline fuzz corpus deterministically outside libFuzzer
test-html5-pipeline-fuzz-corpus:
	cargo test -p html --features html5 --lib --locked \
		html5::fuzz::tests::corpus::replay_committed_html5_pipeline_corpus_deterministically

# Replay committed HTML5 pipeline regression snapshots against their matching inputs
test-html5-pipeline-regressions:
	cargo test -p html --test html5_pipeline_regressions --features "html5 html5-fuzzing dom-snapshot parser_invariants" --locked

# Run a short deterministic HTML5 end-to-end pipeline fuzz smoke against the actual fuzz target
test-html5-pipeline-fuzz-smoke:
	bash ./tools/ci/html5_pipeline_fuzz_smoke.sh

# Run a longer deterministic HTML5 end-to-end pipeline fuzz lane for nightly/manual use
test-html5-pipeline-fuzz-long:
	HTML5_PIPELINE_FUZZ_LABEL='html5 pipeline fuzz nightly' \
	HTML5_PIPELINE_FUZZ_ARTIFACT_BASENAME='html5_pipeline_fuzz_failure_nightly' \
	HTML5_PIPELINE_FUZZ_SMOKE_SEED=1732050807 \
	HTML5_PIPELINE_FUZZ_SMOKE_RUNS=20000 \
	HTML5_PIPELINE_FUZZ_SMOKE_INPUT_TIMEOUT_SEC=10 \
	HTML5_PIPELINE_FUZZ_SMOKE_WALL_TIMEOUT_SEC=600 \
	bash ./tools/ci/html5_pipeline_fuzz_smoke.sh

# Render a stable HTML5 pipeline regression snapshot from a corpus/regression input
print-html5-pipeline-regression-snapshot:
	@test -n "$(INPUT)" || (echo "usage: make $@ INPUT=fuzz/corpus/html5_pipeline/<name> [NAME=<snapshot-label>]" && exit 1)
	cargo run -p html --features "html5 html5-fuzzing dom-snapshot parser_invariants" --bin html5_pipeline_regression_snapshot --locked -- "$(INPUT)" $(if $(NAME),$(NAME),)

# Run WPT tree-construction slice (tokenizer + tree builder -> DOM snapshot)
test-wpt-tree-builder:
	cargo test -p html --test wpt_html5_tree_builder --features "html5 dom-snapshot" --locked

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
	@$(MAKE) fmt-check
	@$(MAKE) lint
	@$(MAKE) lint-html5
	@$(MAKE) lint-html5-hardening
	@$(MAKE) test
	@$(MAKE) test-html5-legacy
	@$(MAKE) test-html5-toggle
	@$(MAKE) test-html5-dom-golden
	@$(MAKE) test-html5-patch-golden
	@$(MAKE) test-html5-smoke-real-pages
	@$(MAKE) test-html5-tokenizer-fuzz-smoke
	@$(MAKE) test-html5-tree-builder-token-fuzz-smoke
	@$(MAKE) test-html5-pipeline-regressions
	@$(MAKE) test-html5-pipeline-fuzz-smoke
	@$(MAKE) test-wpt-tree-builder
	@$(MAKE) build
	@$(MAKE) build-html5
	@$(MAKE) build-release
	@$(MAKE) build-release-html5
	@$(MAKE) html-entities-check

loc:
	git ls-files | xargs wc -l

llf:
	rg --files -g '**/*.rs' | xargs wc -l | sort -nr | head -n 30

# Copy each tracked file with unstaged changes and each untracked file as
# "path:" followed by its current contents. This excludes staged-only tracked
# changes and skips deleted paths.
cuc:
	@command -v pbcopy >/dev/null 2>&1 || (echo "error: pbcopy not found" && exit 1)
	@files="$$( { \
		git diff --name-only; \
		git ls-files --others --exclude-standard; \
	} | sort -u )"; \
	if [ -z "$$files" ]; then \
		echo "No unstaged or untracked files."; \
		exit 0; \
	fi; \
	tmp="$$(mktemp)"; \
	trap 'rm -f "$$tmp"' EXIT; \
	count=0; \
	for file in $$files; do \
		if [ ! -f "$$file" ]; then \
			continue; \
		fi; \
		count=$$((count + 1)); \
		printf '%s:\n' "$$file" >> "$$tmp"; \
		cat -- "$$file" >> "$$tmp"; \
		printf '\n\n' >> "$$tmp"; \
	done; \
	pbcopy < "$$tmp"; \
	printf 'Copied %s unstaged/untracked file(s) to the clipboard.\n' "$$count"

# Copy only unstaged tracked hunks plus untracked files as zero-context diffs.
cuc-diff:
	@command -v pbcopy >/dev/null 2>&1 || (echo "error: pbcopy not found" && exit 1)
	@tmp="$$(mktemp)"; \
	trap 'rm -f "$$tmp"' EXIT; \
	git diff --no-ext-diff --no-color --unified=0 --relative > "$$tmp"; \
	git ls-files --others --exclude-standard -z | \
	while IFS= read -r -d '' file; do \
		[ -e "$$file" ] || continue; \
		[ ! -s "$$tmp" ] || printf '\n' >> "$$tmp"; \
		git diff --no-index --no-ext-diff --no-color --unified=0 -- /dev/null "$$file" >> "$$tmp"; \
		rc=$$?; \
		if [ "$$rc" -gt 1 ]; then \
			exit "$$rc"; \
		fi; \
	done; \
	if [ ! -s "$$tmp" ]; then \
		echo "No unstaged or untracked changes."; \
		exit 0; \
	fi; \
	pbcopy < "$$tmp"; \
	echo "Copied unstaged/untracked diff hunks to the clipboard."
