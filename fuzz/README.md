# HTML5 Parser Fuzzing

`fuzz/corpus/html5_tokenizer/` contains the committed seed corpus for the HTML5
tokenizer fuzz harness. Triaged crashing inputs belong in
`fuzz/regressions/html5_tokenizer/`.

`fuzz/corpus/html5_tree_builder_tokens/` contains the committed seed corpus for
the synthetic token-stream HTML5 tree-builder fuzz harness. Triaged crashing
inputs belong in `fuzz/regressions/html5_tree_builder_tokens/`.

The tree-builder harness is intentionally byte-driven but not tokenizer-derived:
it decodes arbitrary bytes into a bounded synthetic token stream using a mix of
fixed HTML tag/attribute catalogs and fuzz-generated names, values, comments,
text, and doctypes. The harness appends `EOF` itself so replay remains
deterministic and the token-stream contract stays focused on tree-builder
recovery semantics rather than tokenizer fidelity.

For the tokenizer threat model, panic-free scope, enforced limits, and the
expected fuzz triage workflow, see
`docs/security/html5-tokenizer-hardening.md`.

Seed categories currently covered:
- partial tags and unterminated attribute/value tails
- broken comments and malformed doctypes
- dense `<` runs and script/rawtext lookalikes
- long attribute sequences
- invalid UTF-8 and NUL-heavy byte streams
- synthetic malformed token orderings and weird structural nesting

## Replay

Deterministic committed-input replay outside libFuzzer:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_tokenizer_corpus_deterministically
```

Equivalent Make target:

```sh
make test-html5-tokenizer-fuzz-corpus
```

Replay a single committed seed deterministically through normal test infrastructure:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_single_committed_seed_deterministically
```

If `cargo-fuzz` is installed, replay a specific seed through the actual fuzz target with:

```sh
cargo fuzz run html5_tokenizer fuzz/corpus/html5_tokenizer/<seed-name>
```

The deterministic replay test above replays both committed seed corpus entries
and committed regression inputs from `fuzz/regressions/html5_tokenizer/`.

Deterministic replay for the synthetic token-stream tree-builder harness:

```sh
cargo test -p html --features html5 --lib \
  html5::tree_builder::fuzz::tests::corpus::replay_committed_tree_builder_token_corpus_deterministically
```

Equivalent Make target:

```sh
make test-html5-tree-builder-token-fuzz-corpus
```

Replay a specific seed through the actual tree-builder fuzz target with:

```sh
cargo fuzz run html5_tree_builder_tokens fuzz/corpus/html5_tree_builder_tokens/<seed-name>
```

## CI Smoke

PR CI runs a short deterministic smoke lane around the actual tokenizer fuzz
target via:

```sh
make test-html5-tokenizer-fuzz-smoke
```

Current smoke budget:
- fixed seed: `1592653589`
- fixed runs: `128`
- libFuzzer per-input timeout: `5s`
- outer wall timeout: `90s`

On failure, the CI logs print:
- the fixed seed,
- the exact failing artifact path if libFuzzer materialized one, and
- direct-binary plus `cargo fuzz run` reproduction commands.

PR CI also runs a short deterministic smoke lane around the synthetic token
tree-builder fuzz target via:

```sh
make test-html5-tree-builder-token-fuzz-smoke
```

Current tree-builder smoke budget:
- fixed seed: `3141592653`
- fixed runs: `128`
- libFuzzer per-input timeout: `5s`
- outer wall timeout: `90s`

## Nightly / Long Run

Nightly and manually triggered perf CI runs a longer deterministic tokenizer
fuzz lane via:

```sh
make test-html5-tokenizer-fuzz-long
```

Current long-run budget:
- fixed seed: `2718281828`
- fixed runs: `20000`
- libFuzzer per-input timeout: `10s`
- outer wall timeout: `600s`

The nightly/manual workflow uses the same failure logging contract as the PR
smoke lane and uploads the crashing artifact on failure.

The equivalent long-run tree-builder lane is:

```sh
make test-html5-tree-builder-token-fuzz-long
```

## Triage

- Download the failure artifact from CI.
- Reproduce it locally using the logged direct-binary or `cargo fuzz run` command.
- Minimize the input and commit it to the matching regression directory:
  `fuzz/regressions/html5_tokenizer/` for tokenizer crashes and
  `fuzz/regressions/html5_tree_builder_tokens/` for synthetic tree-builder
  crashes.
- Re-run the matching deterministic replay test to lock the regression in.

Tokenizer replay:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_tokenizer_corpus_deterministically
```

Synthetic tree-builder replay:

```sh
cargo test -p html --features html5 --lib \
  html5::tree_builder::fuzz::tests::corpus::replay_committed_tree_builder_token_corpus_deterministically
```

## Regression Workflow

- Add minimized reusable seed inputs to the matching committed corpus when they
  improve steady-state coverage:
  `fuzz/corpus/html5_tokenizer/` for raw-byte tokenizer cases and
  `fuzz/corpus/html5_tree_builder_tokens/` for synthetic token-stream tree-builder
  cases.
- Keep crash or hang reproducers in the matching `fuzz/regressions/...`
  directory even if you also promote a minimized variant into the committed
  seed corpus.
- Use stable descriptive file names that describe the construct being stressed.
- Keep entries small and focused; prefer one failure mode per seed.
- Re-run the matching committed-input replay target before landing new corpus or
  regression entries.
