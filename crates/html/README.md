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
allocator overrides scoped. Enable explicitly:

```sh
cargo test -p html --test alloc_guards --features "html5 count-alloc"
```
