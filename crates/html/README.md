# html crate: performance harness

## Parser API

The stable engine-facing API is the HTML5-backed parser facade:

```rust
let output = html::parse_document("<!doctype html><p>Hello</p>", html::HtmlParseOptions::default())?;
```

For streaming/chunked parsing:

```rust
let mut parser = html::HtmlParser::new(html::HtmlParseOptions::default())?;
parser.push_bytes(chunk)?;
parser.pump()?;
let batch = parser.take_patch_batch()?;
```

Notes:

- `html::parse_document` and `html::HtmlParser` are backed only by the HTML5
  tokenizer/tree-builder/session pipeline.
- The full cutover and migration guide lives in
  `docs/html5-parser-cutover.md`.
- The public facade exposes its own stable types (`HtmlParseOptions`,
  `HtmlParseError`, `HtmlParseCounters`, `HtmlParseEvent`) rather than raw
  `html::html5::*` backend types.
- The legacy tokenizer/tree-builder implementation has been removed. The HTML5
  facade is the only parser entrypoint and runtime backend.
- `ParseOutput.patches` contains the patches drained by `into_output()`. For
  one-shot `parse_document(...)` calls, that is the full patch history. For
  streaming sessions that already drained patches earlier, it is only the
  undrained remainder; `contains_full_patch_history` makes that explicit.
- Low-level `html::html5::*` exports remain available for tests, fuzzing, and
  specialized tooling; they are not the preferred engine contract.
- The legacy-vs-HTML5 parity contract is documented in
  `docs/html5/parser-parity-matrix.md`.

## Benchmarks

This crate uses Criterion for statistically robust micro-benchmarks that are
repeatable locally with stable methodology.

Run all HTML benches:

```sh
cargo bench -p html --features html5
```

Run only the HTML bench harness:

```sh
cargo bench -p html --bench html_bench --features html5
```

## Perf regression guards (CI)

Deterministic regression guards run under `cargo test` and validate HTML5 parse
counters, patch counts, node counts, and chunk-size stability. The heavier
cases are gated behind the `perf-tests` feature.

Run locally:

```sh
cargo test -p html --all-targets --features html5
```

Run the heavier perf guards (intended for CI):

```sh
cargo test -p html --all-targets --features "html5 perf-tests"
```

Note: `perf-tests` includes timing-based regression checks. Prefer running these
in a dedicated nightly perf workflow (optionally with `RUST_TEST_THREADS=1`)
rather than on every PR.

## Allocation guards (opt-in)

Allocation-count tests live in a dedicated integration test binary to keep
allocator overrides scoped. Its counters are process-global, so run the complete
target serially with locked dependencies and both required features:

```sh
RUST_TEST_THREADS=1 cargo test -p html --test alloc_guards --locked \
  --features html5,count-alloc -- --test-threads=1 --nocapture --format pretty --color never
```

The scheduled/manual `Perf` workflow runs this target in the independent
`html_alloc_guards` Linux job. A heavy-performance failure cannot skip that job;
allocation success does not conceal a heavy-performance failure or make the
overall workflow successful. Existing allocation measurements are visible through
`--nocapture`; the workflow does not add or change measurement instrumentation.

Scheduled runs and manual runs without an override test their immutable event
SHA. The single optional manual input `allocation_source_sha` accepts a full
lowercase commit SHA from the workflow's repository. It lets a reviewed workflow
definition qualify a separate, explicitly reviewed source commit without changing
that commit. A valid SHA establishes identity, not trust in arbitrary source.

The job records workflow, event, checkout and tree identities and the active
Rust 1.92.0 toolchain. Each execution builds into a fresh directory under
`$RUNNER_TEMP`; logs and source receipts also stay outside the checkout. Explicit
source overrides restore/save no Cargo caches. Source cleanliness is checked
before and after execution, including after test failure. Successful source
verification alone does not constitute successful allocation qualification.

Successful qualification also requires native libtest execution evidence. The
job lists the target first, requires all existing allocation guards (including
the pathological streaming guard), and compares that inventory with execution
records. The final summary must report every listed test passed, with none
failed, ignored or filtered out. The count is discovered, so future additions
are included rather than blocked by a fixed six-test limit.
Execution records support ordinary tests and Rust 1.92's `- should panic`
annotation, including interleaved `--nocapture` diagnostics from expected panics.

See [the #1471 qualification record](../../docs/ci/1471-html-allocation-qualification.md)
for reproduction, trust boundaries, evidence requirements and completed, verified
hosted manual and scheduled qualification. Manual evidence is not scheduled
qualification.
