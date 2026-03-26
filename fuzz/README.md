# HTML5 Tokenizer Fuzzing

`fuzz/corpus/html5_tokenizer/` contains the committed seed corpus for the HTML5
tokenizer fuzz harness. Triaged crashing inputs belong in
`fuzz/regressions/html5_tokenizer/`.

Seed categories currently covered:
- partial tags and unterminated attribute/value tails
- broken comments and malformed doctypes
- dense `<` runs and script/rawtext lookalikes
- long attribute sequences
- invalid UTF-8 and NUL-heavy byte streams

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

## Triage

- Download the failure artifact from CI.
- Reproduce it locally using the logged direct-binary or `cargo fuzz run` command.
- Minimize the input and commit it to `fuzz/regressions/html5_tokenizer/`.
- Re-run the deterministic replay test to lock the regression in:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_tokenizer_corpus_deterministically
```

## Regression Workflow

- Add the minimal reproducing bytes as a new file in `fuzz/corpus/html5_tokenizer/`.
- Use a stable descriptive file name that describes the construct being stressed.
- Keep corpus entries small and focused; prefer one failure mode per seed.
- Re-run the committed-input replay test above before landing the regression seed.
