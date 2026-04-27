# HTML5 and CSS Fuzzing

`fuzz/corpus/html5_tokenizer/` contains the committed seed corpus for the HTML5
tokenizer fuzz harness. Triaged crashing inputs belong in
`fuzz/regressions/html5_tokenizer/`.

`fuzz/corpus/html5_tokenizer_script_data/` contains the committed seed corpus
for the targeted HTML5 tokenizer script-data fuzz harness. Triaged crashing
inputs belong in `fuzz/regressions/html5_tokenizer_script_data/`.

`fuzz/corpus/html5_tokenizer_rawtext/` contains the committed seed corpus for
the targeted HTML5 tokenizer RAWTEXT fuzz harness. Triaged crashing inputs
belong in `fuzz/regressions/html5_tokenizer_rawtext/`.

`fuzz/corpus/html5_tokenizer_rcdata/` contains the committed seed corpus for the
targeted HTML5 tokenizer RCDATA fuzz harness. Triaged crashing inputs belong in
`fuzz/regressions/html5_tokenizer_rcdata/`.

`fuzz/corpus/html5_tree_builder_tokens/` contains the committed seed corpus for
the synthetic token-stream HTML5 tree-builder fuzz harness. Triaged crashing
inputs belong in `fuzz/regressions/html5_tree_builder_tokens/`.

`fuzz/corpus/html5_pipeline/` contains the committed seed corpus for the
end-to-end HTML5 pipeline fuzz harness (bytes -> tokenizer -> tree builder ->
patches). Triaged crashing inputs belong in `fuzz/regressions/html5_pipeline/`.

The tree-builder harness is intentionally byte-driven but not tokenizer-derived:
it decodes arbitrary bytes into a bounded synthetic token stream using a mix of
fixed HTML tag/attribute catalogs and fuzz-generated names, values, comments,
text, and doctypes. The harness appends `EOF` itself so replay remains
deterministic and the token-stream contract stays focused on tree-builder
recovery semantics rather than tokenizer fidelity.

The end-to-end pipeline harness is the realistic attacker-model lane: it feeds
arbitrary bytes through the UTF-8 decoder and tokenizer with seeded chunking,
streams emitted tokens into the tree builder one token at a time, applies
tree-builder tokenizer controls immediately, drains patches incrementally, and
checks tokenizer + DOM/patch invariants during streaming rather than only at
EOF.

`fuzz/corpus/css_tokenizer/` contains the committed seed corpus for the CSS
tokenizer fuzz harness. Triaged crashing inputs belong in
`fuzz/regressions/css_tokenizer/`.

`fuzz/corpus/css_parser/` contains the committed seed corpus for the CSS
stylesheet parser fuzz harness. Triaged crashing inputs belong in
`fuzz/regressions/css_parser/`.

`fuzz/corpus/css_selector_parser/` contains the committed seed corpus for the
CSS selector-parser fuzz harness. Triaged crashing inputs belong in
`fuzz/regressions/css_selector_parser/`.

`fuzz/corpus/css_selector_matching/` contains the committed seed corpus for the
CSS selector-matching fuzz harness. Triaged crashing inputs belong in
`fuzz/regressions/css_selector_matching/`.

`fuzz/corpus/css_cascade/` contains the committed seed corpus for the CSS
cascade-resolution fuzz harness. Triaged crashing inputs belong in
`fuzz/regressions/css_cascade/`.

`fuzz/corpus/css_values/` contains the committed seed corpus for the CSS
property/value fuzz harness. Triaged crashing inputs belong in
`fuzz/regressions/css_values/`.

The CSS harnesses are byte-driven like the HTML ones, but the CSS syntax layer
currently starts from decoded UTF-8 text instead of a streaming byte decoder.
Arbitrary fuzz bytes are therefore decoded deterministically with
`String::from_utf8_lossy(...)` before entering the tokenizer/parser. The harness
still derives a stable seed from the original bytes so failures have consistent
metadata and replay identity.

Milestone T boundary for CSS:
- T4 introduces the CSS tokenizer/parser fuzz harnesses, committed corpus
  replay, regression staging directories, and local deterministic smoke
  workflows.
- T5 extends that workflow with dedicated selector parser, selector matching,
  cascade resolution, and property/value fuzz harnesses so complex downstream
  CSS pipeline paths are exercised through deterministic, bounded replay.
- The T5 selector and value harnesses also enforce structured determinism and
  invariant checks in addition to crash/hang detection, so fuzz targets panic
  on harness-level invariant failures instead of silently discarding summaries.
- T7 adds GitHub Actions coverage for promoted CSS fuzz regressions plus
  deterministic CSS syntax and pipeline smoke lanes with fixed seeds, bounded
  run counts, and failure artifacts that include direct repro commands in the
  job log.

For the tokenizer threat model, panic-free scope, enforced limits, and the
expected fuzz triage workflow, see
`docs/security/html5-tokenizer-hardening.md`.

Seed categories currently covered:
- partial tags and unterminated attribute/value tails
- broken comments and malformed doctypes
- dense `<` runs and script/rawtext lookalikes
- direct script-data close-tag storms, near-miss `</script>` tails, and escaped-script families
- direct RAWTEXT near-miss `</style>` tails and whitespace/partial close candidates
- direct RCDATA near-miss `</title>` and `</textarea>` tails plus entity-bearing payloads
- long attribute sequences
- invalid UTF-8 and NUL-heavy byte streams
- synthetic malformed token orderings and weird structural nesting
- end-to-end chunked byte streams that exercise text-mode controls and patch streaming

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

Deterministic committed-input replay for the targeted script-data tokenizer
harness:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_script_data_corpus_deterministically
```

Equivalent Make target:

```sh
make test-html5-tokenizer-script-data-fuzz-corpus
```

Replay a specific script-data seed through the actual fuzz target with:

```sh
cargo fuzz run html5_tokenizer_script_data fuzz/corpus/html5_tokenizer_script_data/<seed-name>
```

Deterministic committed-input replay for the targeted RAWTEXT tokenizer
harness:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_rawtext_corpus_deterministically
```

Equivalent Make target:

```sh
make test-html5-tokenizer-rawtext-fuzz-corpus
```

Replay a specific RAWTEXT seed through the actual fuzz target with:

```sh
cargo fuzz run html5_tokenizer_rawtext fuzz/corpus/html5_tokenizer_rawtext/<seed-name>
```

Deterministic committed-input replay for the targeted RCDATA tokenizer
harness:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_rcdata_corpus_deterministically
```

Equivalent Make target:

```sh
make test-html5-tokenizer-rcdata-fuzz-corpus
```

Replay a specific RCDATA seed through the actual fuzz target with:

```sh
cargo fuzz run html5_tokenizer_rcdata fuzz/corpus/html5_tokenizer_rcdata/<seed-name>
```

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

Deterministic replay for the end-to-end HTML5 pipeline harness:

```sh
cargo test -p html --features html5 --lib \
  html5::fuzz::tests::corpus::replay_committed_html5_pipeline_corpus_deterministically
```

Equivalent Make target:

```sh
make test-html5-pipeline-fuzz-corpus
```

Replay a specific seed through the actual pipeline fuzz target with:

```sh
cargo fuzz run html5_pipeline fuzz/corpus/html5_pipeline/<seed-name>
```

Deterministic replay for the CSS tokenizer harness:

```sh
cargo test -p css --features css-fuzzing --lib \
  syntax::fuzz::tests::corpus::replay_committed_css_tokenizer_corpus_deterministically
```

Equivalent Make target:

```sh
make test-css-tokenizer-fuzz-corpus
```

Replay a specific CSS tokenizer seed through the actual fuzz target with:

```sh
cargo fuzz run css_tokenizer fuzz/corpus/css_tokenizer/<seed-name>
```

Deterministic replay for the CSS stylesheet parser harness:

```sh
cargo test -p css --features css-fuzzing --lib \
  syntax::fuzz::tests::corpus::replay_committed_css_parser_corpus_deterministically
```

Equivalent Make target:

```sh
make test-css-parser-fuzz-corpus
```

Replay a specific CSS parser seed through the actual fuzz target with:

```sh
cargo fuzz run css_parser fuzz/corpus/css_parser/<seed-name>
```

Deterministic replay for the CSS selector parser harness:

```sh
cargo test -p css --features css-fuzzing --lib \
  selectors::fuzz::tests::replay_committed_selector_parser_corpus_deterministically
```

Equivalent Make target:

```sh
make test-css-selector-parser-fuzz-corpus
```

Replay a specific CSS selector parser seed through the actual fuzz target with:

```sh
cargo fuzz run css_selector_parser fuzz/corpus/css_selector_parser/<seed-name>
```

Deterministic replay for the CSS selector matching harness:

```sh
cargo test -p css --features css-fuzzing --lib \
  selectors::fuzz::tests::replay_committed_selector_matching_corpus_deterministically
```

Equivalent Make target:

```sh
make test-css-selector-matching-fuzz-corpus
```

Replay a specific CSS selector matching seed through the actual fuzz target
with:

```sh
cargo fuzz run css_selector_matching fuzz/corpus/css_selector_matching/<seed-name>
```

Deterministic replay for the CSS cascade harness:

```sh
cargo test -p css --features css-fuzzing --lib \
  cascade::fuzz::tests::replay_committed_css_cascade_corpus_deterministically
```

Equivalent Make target:

```sh
make test-css-cascade-fuzz-corpus
```

Replay a specific CSS cascade seed through the actual fuzz target with:

```sh
cargo fuzz run css_cascade fuzz/corpus/css_cascade/<seed-name>
```

Deterministic replay for the CSS property/value harness:

```sh
cargo test -p css --features css-fuzzing --lib \
  computed::fuzz::tests::replay_committed_css_values_corpus_deterministically
```

Equivalent Make target:

```sh
make test-css-values-fuzz-corpus
```

Replay a specific CSS values seed through the actual fuzz target with:

```sh
cargo fuzz run css_values fuzz/corpus/css_values/<seed-name>
```

Render a stable pipeline regression snapshot from a committed corpus/regression
input:

```sh
make print-html5-pipeline-regression-snapshot \
  INPUT=fuzz/regressions/html5_pipeline/<seed-name>
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

PR CI also runs a short deterministic smoke lane around the targeted script-data
tokenizer fuzz target via:

```sh
make test-html5-tokenizer-script-data-fuzz-smoke
```

Current script-data smoke budget:
- fixed seed: `1123581321`
- fixed runs: `128`
- libFuzzer per-input timeout: `5s`
- outer wall timeout: `90s`

PR CI also runs a short deterministic smoke lane around the targeted RAWTEXT
tokenizer fuzz target via:

```sh
make test-html5-tokenizer-rawtext-fuzz-smoke
```

Current RAWTEXT smoke budget:
- fixed seed: `2654435761`
- fixed runs: `128`
- libFuzzer per-input timeout: `5s`
- outer wall timeout: `90s`

PR CI also runs a short deterministic smoke lane around the targeted RCDATA
tokenizer fuzz target via:

```sh
make test-html5-tokenizer-rcdata-fuzz-smoke
```

Current RCDATA smoke budget:
- fixed seed: `2246822519`
- fixed runs: `128`
- libFuzzer per-input timeout: `5s`
- outer wall timeout: `90s`

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

PR CI also runs a short deterministic smoke lane around the end-to-end pipeline
fuzz target via:

```sh
make test-html5-pipeline-fuzz-smoke
```

CSS smoke lanes are available in the local development workflow now:

```sh
make test-css-tokenizer-fuzz-smoke
make test-css-parser-fuzz-smoke
make test-css-selector-parser-fuzz-smoke
make test-css-selector-matching-fuzz-smoke
make test-css-cascade-fuzz-smoke
make test-css-values-fuzz-smoke
```

Local script defaults:
- CSS tokenizer fixed seed: `1873819023`
- CSS parser fixed seed: `2718281828`
- CSS selector parser fixed seed: `1873819023`
- CSS selector matching fixed seed: `1873819023`
- CSS cascade fixed seed: `1873819023`
- CSS values fixed seed: `1873819023`
- fixed runs: `128`
- libFuzzer per-input timeout: `5s`
- outer wall timeout: `90s`

GitHub Actions T7 overrides keep the same bounded budget shape but use explicit
per-lane seeds for the downstream CSS jobs:
- CSS tokenizer fixed seed: `1873819023`
- CSS parser fixed seed: `2718281828`
- CSS selector parser fixed seed: `3141592653`
- CSS selector matching fixed seed: `1618033988`
- CSS cascade fixed seed: `1414213562`
- CSS values fixed seed: `2449489742`
- fixed runs: `128`
- libFuzzer per-input timeout: `5s`
- outer wall timeout: `90s`

On failure, the CSS smoke scripts print the fixed seed, exact corpus and
regression directories, the failure artifact path if libFuzzer materialized
one, and direct-binary plus `cargo fuzz run` reproduction commands.

Current pipeline smoke budget:
- fixed seed: `1414213562`
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

The equivalent long-run targeted script-data lane is:

```sh
make test-html5-tokenizer-script-data-fuzz-long
```

The equivalent long-run targeted RAWTEXT lane is:

```sh
make test-html5-tokenizer-rawtext-fuzz-long
```

The equivalent long-run targeted RCDATA lane is:

```sh
make test-html5-tokenizer-rcdata-fuzz-long
```

The equivalent long-run tree-builder lane is:

```sh
make test-html5-tree-builder-token-fuzz-long
```

The equivalent long-run end-to-end pipeline lane is:

```sh
make test-html5-pipeline-fuzz-long
```

## Triage

- Download the failure artifact from CI.
- Reproduce it locally using the logged direct-binary or `cargo fuzz run` command.
- Minimize the input and commit it to the matching regression directory:
  `fuzz/regressions/html5_tokenizer/` for tokenizer crashes and
  `fuzz/regressions/html5_tokenizer_script_data/` for targeted script-data tokenizer
  crashes,
  `fuzz/regressions/html5_tokenizer_rawtext/` for targeted RAWTEXT tokenizer
  crashes,
  `fuzz/regressions/html5_tokenizer_rcdata/` for targeted RCDATA tokenizer
  crashes,
  `fuzz/regressions/html5_tree_builder_tokens/` for synthetic tree-builder
  crashes, and `fuzz/regressions/html5_pipeline/` for end-to-end pipeline
  crashes.
- For end-to-end pipeline regressions, render and commit the matching stable
  snapshot under
  `crates/html/tests/regressions/html5_pipeline/<seed-name>.snap`:

```sh
make print-html5-pipeline-regression-snapshot \
  INPUT=fuzz/regressions/html5_pipeline/<seed-name> \
  > crates/html/tests/regressions/html5_pipeline/<seed-name>.snap
```

- Re-run the matching deterministic replay test to lock the regression in.
- Re-run the pipeline regression snapshot lane after updating or adding `.snap`
  files:

```sh
cargo test -p html --test html5_pipeline_regressions \
  --features "html5 html5-fuzzing dom-snapshot parser_invariants"
```

Tokenizer replay:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_tokenizer_corpus_deterministically
```

Targeted script-data tokenizer replay:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_script_data_corpus_deterministically
```

Targeted RAWTEXT tokenizer replay:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_rawtext_corpus_deterministically
```

Targeted RCDATA tokenizer replay:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_rcdata_corpus_deterministically
```

Synthetic tree-builder replay:

```sh
cargo test -p html --features html5 --lib \
  html5::tree_builder::fuzz::tests::corpus::replay_committed_tree_builder_token_corpus_deterministically
```

End-to-end pipeline replay:

```sh
cargo test -p html --features html5 --lib \
  html5::fuzz::tests::corpus::replay_committed_html5_pipeline_corpus_deterministically
```

## Regression Workflow

- Add minimized reusable seed inputs to the matching committed corpus when they
  improve steady-state coverage:
  `fuzz/corpus/html5_tokenizer/` for raw-byte tokenizer cases and
  `fuzz/corpus/html5_tokenizer_script_data/` for direct script-data tokenizer cases and
  `fuzz/corpus/html5_tokenizer_rawtext/` for direct RAWTEXT tokenizer cases and
  `fuzz/corpus/html5_tokenizer_rcdata/` for direct RCDATA tokenizer cases and
  `fuzz/corpus/html5_tree_builder_tokens/` for synthetic token-stream tree-builder
  cases, and `fuzz/corpus/html5_pipeline/` for end-to-end byte-stream pipeline
  cases.
- Keep crash or hang reproducers in the matching `fuzz/regressions/...`
  directory even if you also promote a minimized variant into the committed
  seed corpus.
- Use `fuzz/corpus/css_tokenizer/` and `fuzz/regressions/css_tokenizer/` for
  CSS tokenizer byte inputs, and `fuzz/corpus/css_parser/` plus
  `fuzz/regressions/css_parser/` for CSS stylesheet parser byte inputs.
- Use `fuzz/corpus/css_selector_parser/` plus
  `fuzz/regressions/css_selector_parser/` for selector-parser byte inputs,
  `fuzz/corpus/css_selector_matching/` plus
  `fuzz/regressions/css_selector_matching/` for selector-matching byte inputs,
  `fuzz/corpus/css_cascade/` plus `fuzz/regressions/css_cascade/` for
  cascade-resolution byte inputs, and `fuzz/corpus/css_values/` plus
  `fuzz/regressions/css_values/` for property/value byte inputs.
- Promote stabilized CSS findings that should become permanent regression
  oracles into `crates/css/tests/regressions/css_fuzz/`. Each promoted fixture
  is a directory with versioned metadata (`meta.txt`), exact replay bytes
  (`input.bin`), and a committed stable harness summary (`summary.txt`).
  Summary fields, including `termination`, use explicit lowercase stable labels
  rather than Rust `Debug` output.
- Use stable descriptive file names that describe the construct being stressed.
- Keep entries small and focused; prefer one failure mode per seed.
- Re-run the matching committed-input replay target before landing new corpus or
  regression entries.
- For pipeline regressions, keep the input bytes in `fuzz/regressions/html5_pipeline/`
  and the snapshot in `crates/html/tests/regressions/html5_pipeline/` with the
  same base name so the regression test can pair them deterministically.
- For targeted RAWTEXT/script findings that should become stable parser
  regressions, promote the minimized input into
  `crates/html/tests/regressions/html5/rawtext_script/` with:
  - `meta.txt` carrying `tool`, `seed`, `date`, `issue`, `mode`, and `guard`,
    where `issue` must be either a URL or a stable issue id such as
    `#1234`, `BOR-123`, or `Milestone-L/L5`,
  - `input.html` as the minimized stable HTML wrapper,
  - `tokens.txt` and/or `dom.txt` as the locked oracle.

Verify promoted RAWTEXT/script regressions with:

```sh
cargo test -p html --test html5_rawtext_script_regressions \
  --features "html5 dom-snapshot parser_invariants"
```

That harness checks both whole-input and deterministic every-boundary chunked
execution before the regression lands.

Verify promoted CSS fuzz regressions with:

```sh
cargo test -p css --features css-fuzzing --test css_fuzz_regressions --locked
```

Render or refresh a promoted CSS regression summary with:

```sh
cargo run -p css --features css-fuzzing --bin css_fuzz_regression_summary -- \
  --tool <css_tokenizer|css_parser|css_selector_parser|css_selector_matching|css_cascade|css_values> \
  --profile <default|selector-limit-zero> \
  --input crates/css/tests/regressions/css_fuzz/<fixture>/input.bin --seed <u64>
```

The promoted CSS regression fixtures are intentionally version-controlled and
checked in normal test runs so fuzz-discovered failures remain reproducible even
without libFuzzer.
