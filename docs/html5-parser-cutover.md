# HTML5 Parser Cutover Guide

This document is the developer-facing guide for Borrowser's HTML parser after
Milestone M.

The cutover is complete:

- the HTML5 parser is the only parser backend
- the stable engine-facing entrypoints are `html::parse_document(...)` and
  `html::HtmlParser`
- runtime, parity harnesses, perf guards, and allocation guards all run on the
  HTML5 path

This guide covers:

- current parser architecture
- stable public API usage
- remaining feature flags
- intentional behavior differences from the retired legacy parser
- migration notes for contributors updating old code
- debugging and verification workflows

## Architecture

The shipped parser stack is:

1. `HtmlParser` / `parse_document(...)`
2. `Html5ParseSession`
3. HTML5 tokenizer
4. HTML5 tree builder
5. `DomPatch` emission plus internal patch-mirror validation

Important properties of the current architecture:

- there is no runtime parser mode switch
- there is no legacy tokenizer/tree-builder fallback
- the stable API is façade-owned and does not expose raw `html::html5::*` types
- one-shot and streaming parsing share the same HTML5 backend and patch mirror

Relevant code:

- [`crates/html/src/parser.rs`](../crates/html/src/parser.rs)
- [`crates/html/src/html5/session`](../crates/html/src/html5/session)
- [`crates/runtime_parse/src/state.rs`](../crates/runtime_parse/src/state.rs)

## Public API

### One-shot parsing

Use `parse_document(...)` when the full input is already available:

```rust
use html::{HtmlParseOptions, parse_document};

let output = parse_document(
    "<!doctype html><p>Hello</p>",
    HtmlParseOptions::default(),
)?;

assert!(output.contains_full_patch_history);
```

Contract:

- backed only by the HTML5 parser pipeline
- deterministic for fixed input
- returns the materialized DOM, emitted patches, counters, and parse events
- `ParseOutput::patches` is the full patch history in the one-shot path

### Streaming / chunked parsing

Use `HtmlParser` when input arrives incrementally:

```rust
use html::{HtmlParseOptions, HtmlParser};

let mut parser = HtmlParser::new(HtmlParseOptions::default())?;

parser.push_bytes(b"<div><span>hel")?;
parser.pump()?;
let first_batch = parser.take_patch_batch()?;

parser.push_bytes(b"lo</span></div>")?;
parser.finish()?;
let output = parser.into_output()?;
```

Contract:

- `push_bytes(...)` appends raw bytes
- `push_str(...)` appends already-decoded UTF-8 text
- `pump()` advances the parser until it reaches a stable stop point
- `finish()` is required when no more input will arrive
- `take_patches()` drains all currently available patches as one ordered vector
- `take_patch_batch()` drains the next atomic patch batch
- `into_output()` consumes the parser and materializes the DOM mirror

Important streaming notes:

- `finish()` is required for EOF-sensitive cases such as open RAWTEXT/RCDATA
  containers that still hold buffered text
- if patches were already drained before `into_output()`, the final
  `ParseOutput::patches` contains only the undrained remainder
- `ParseOutput::contains_full_patch_history` tells callers whether the patch
  vector is complete for the session

### Error surface

The stable error type is `HtmlParseError`:

- `Decode`
  byte-stream decoding failed in a way the façade reports as terminal
- `Invariant`
  an engine invariant was violated, including use after parser poison
- `PatchValidation(String)`
  the internal patch mirror rejected emitted patches

If patch validation fails during streaming drain, the parser becomes terminally
poisoned. Any later mutating or draining call returns
`HtmlParseError::Invariant`.

### Events and counters

The façade exposes:

- `HtmlParseCounters`
- `HtmlParseEvent`
- `HtmlErrorPolicy`

These are stable façade-owned types. Downstream code should not depend on raw
`html::html5::*` event or counter types.

## Feature Flags

The parser backend is no longer feature-selectable, but the crate still has
feature flags for tooling, tests, and internal diagnostics.

Current `html` crate flags:

- `html5`
  default feature; enables the HTML5 parser modules and public façade
- `html5-fuzzing`
  enables fuzz harness support used by committed fuzz/corpus tests
- `perf-tests`
  enables heavier perf regression guards
- `count-alloc`
  enables allocation-count test lanes
- `dom-snapshot`
  enables snapshot/test support used by parity and WPT slices
- `test-harness`
  enables additional harness helpers
- `parse-guards`
  enables internal parse-entry/output guard counters used by regression tests
- `parser_invariants`
  enables stricter invariant checks used by hardening/fuzz lanes
- `internal-api`
  exposes selected internal IDs for engine-internal consumers
- `debug-stats`
  enables additional debug statistics
- `html5-entities`
  reserved entity-related support flag used by HTML5 internals/tooling

Operational guidance:

- normal consumers should use the default feature set
- do not treat feature flags as parser backend selectors
- if you disable default features, the public parser façade is not available

## Known Behavior Differences

The retired legacy parser is not the correctness oracle anymore. The current
policy is documented in
[`docs/html5/parser-parity-matrix.md`](./html5/parser-parity-matrix.md).

### Must match guarantees

These remain compatibility requirements:

- deterministic DOM structure for the supported subset
- patch determinism and materializability
- chunked vs whole-input equivalence for supported inputs
- UTF-8 preservation, entity behavior, and RAWTEXT/text-mode stability where
  explicitly covered by the parity corpus

### Intentional differences

These are allowed to differ from the retired parser:

- malformed markup recovery where HTML5 recovery rules are spec-driven
- quirks/no-quirks tree-construction behavior when HTML5 differs from the old
  simplified parser
- exact parse-error diagnostics and recovery bookkeeping

Named examples already covered by targeted tests:

- stray end-tag recovery
- quirks table behavior that keeps an open `<p>`
- no-quirks table behavior that closes an open `<p>`

When adding a new intended difference:

1. add or update golden-corpus parity metadata
2. document the justification in the parity matrix
3. add a targeted regression test

## Migration Notes

### Replace old entrypoints

Old code patterns should be migrated as follows:

- legacy tokenize/build flows
  replace with `html::parse_document(...)` or `html::HtmlParser`
- direct runtime parser selection
  remove it; runtime is HTML5-only
- direct dependence on `html::html5::*` in product code
  replace with façade-owned public types where possible

### Streaming migration rules

If old code consumed parser output incrementally:

- keep the chunking model
- explicitly call `finish()` at EOF
- use `take_patch_batch()` if atomic patch boundaries matter
- use `take_patches()` if one ordered currently-available vector is sufficient
- only rely on `ParseOutput::patches` as full history in the one-shot path, or
  when `contains_full_patch_history` is `true`

### Parse errors and diagnostics

Old code that relied on backend error types should switch to:

- `HtmlParseError` for terminal failures
- `HtmlParseEvent` from `parse_errors()` for surfaced parse diagnostics
- `HtmlParseCounters` for aggregate metrics

## Debugging And Verification

### Golden and parity tests

Useful parser verification commands:

```sh
cargo test -p html --lib
cargo test -p html --test parity_contract --features "html5 dom-snapshot"
cargo test -p browser --test patch_parity --test patch_stream_parity
cargo test -p runtime_parse
```

Use these when:

- a DOM difference appears between chunked and whole-input parsing
- a patch-ordering regression is suspected
- a parity-matrix change needs proof

### WPT and corpus slices

Useful HTML5 slice commands:

```sh
cargo test -p html --test wpt_html5 --features "html5 dom-snapshot"
cargo test -p html --test wpt_html5_tokenizer --features html5
cargo test -p html --test wpt_html5_tree_builder --features "html5 dom-snapshot"
```

### Fuzzing and hardening

Relevant docs:

- [`docs/security/html5-tokenizer-hardening.md`](./security/html5-tokenizer-hardening.md)
- [`docs/html5/rawtext-script-stability.md`](./html5/rawtext-script-stability.md)

Common local commands:

```sh
make test-html5-tokenizer-fuzz-smoke
make test-html5-tokenizer-script-data-fuzz-smoke
make test-html5-tokenizer-rawtext-fuzz-smoke
make test-html5-tokenizer-rcdata-fuzz-smoke
```

When triaging a parser bug:

1. reproduce it with the smallest possible input
2. decide whether it belongs in the golden corpus, parity contract, WPT slice,
   or fuzz regression corpus
3. add the narrowest deterministic regression first
4. then fix the parser behavior

### Perf and allocation guards

Useful commands:

```sh
cargo test -p html --lib --features "html5 perf-tests"
cargo test -p html --test alloc_guards --features "html5 count-alloc"
cargo bench -p html --bench html_bench --features html5 --no-run
```

Use these when changing:

- tokenizer scanning behavior
- tree-builder patch emission
- chunk-drain behavior
- text-mode handling
- internal patch-mirror materialization

## Related Documents

- [`docs/html5/parser-parity-matrix.md`](./html5/parser-parity-matrix.md)
- [`docs/html5/dompatch-contract.md`](./html5/dompatch-contract.md)
- [`docs/html5/node-identity-contract.md`](./html5/node-identity-contract.md)
- [`docs/html5/invariants.md`](./html5/invariants.md)
- [`docs/security/html5-tokenizer-hardening.md`](./security/html5-tokenizer-hardening.md)
- [`crates/html/README.md`](../crates/html/README.md)
