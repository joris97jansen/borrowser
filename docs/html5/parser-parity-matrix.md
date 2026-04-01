# HTML Parser Parity Matrix

This document defines the parser parity contract for the HTML5 cutover.

Legacy parity is no longer "match everything the old parser did." The contract
is split into:

- must-match guarantees that protect engine behavior
- may-differ cases where HTML5 spec correctness intentionally replaces legacy
  behavior

The fixture-level source of truth lives in `html::golden_corpus` metadata via
`ParityCategory` and `LegacyParity`.

## Must Match

### Supported subset DOM structure

For the supported, non-malformed subset, HTML5 output must remain deterministic
and chunk-invariant.

Enforced by:

- `crates/html/src/golden_corpus/tests/mod.rs`
- `crates/html/src/golden_corpus/tests/invariants.rs`
- `crates/html/tests/wpt_html5.rs`
- `crates/html/tests/html5_golden_tree_builder`

Guarantee:

- whole-input and chunked parsing produce equivalent DOM structure for supported
  fixtures
- required tags, attributes, entity decoding, RAWTEXT handling, and UTF-8 text
  preservation remain stable

### Patch determinism

Incremental patch output must remain deterministic for the same input and chunk
plan.

Enforced by:

- `crates/browser/tests/patch_parity.rs`
- `crates/browser/tests/patch_stream_parity.rs`
- `crates/html/src/streaming_parity.rs`

Guarantee:

- patch ordering is stable
- patch batches remain materializable
- patch-driven DOM state matches the one-shot parse result

### Chunked vs full equivalence

Streaming delivery must not change semantic output.

Enforced by:

- `crates/html/src/golden_corpus/tests/mod.rs`
- `crates/html/src/streaming_parity.rs`
- `crates/runtime_parse/src/tests/runtime.rs`

Guarantee:

- chunked parsing matches whole-input parsing for the supported subset
- runtime materialization remains equivalent across delivery modes

## May Differ (Intentional)

### Malformed markup recovery

Malformed markup recovery does not need to match the legacy parser. HTML5
recovery rules are allowed to differ when the behavior is spec-driven and the
result remains deterministic within the HTML5 pipeline.

Covered by:

- golden fixture `recovery_stray_end_tag`
- `crates/html/tests/parity_contract.rs`
- `crates/html/src/html5/tree_builder/tests/recovery.rs`

Justification:

- stray end-tag handling is defined by HTML5 tree-construction recovery rules
- legacy recovery quirks are not a product contract

### Spec-correct quirks behavior

Document-mode and quirks-mode behavior may intentionally differ from the legacy
parser when HTML5 requires different tree-construction behavior.

Covered by:

- golden fixtures `quirks_table_keeps_open_p`
- golden fixture `no_quirks_table_closes_open_p`
- `crates/html/tests/parity_contract.rs`
- `crates/html/src/html5/tree_builder/tests/quirks.rs`

Justification:

- quirks/no-quirks insertion behavior is a spec-correct HTML5 contract
- legacy document-mode behavior is not preserved when it conflicts with HTML5

### Error recovery diagnostics

Exact parse-error kinds, counts, and recovery bookkeeping are not legacy parity
guarantees.

Covered by:

- `html::HtmlParser::parse_errors()`
- `crates/html/src/html5/tree_builder/tests/recovery.rs`

Justification:

- the engine guarantees deterministic HTML5 diagnostics for a given input
- it does not guarantee that legacy and HTML5 report the same recovery events

## Policy

`LegacyParity::MustMatch` means a fixture is part of the engine's compatibility
contract and must remain stable across whole-input and chunked HTML5 parsing.

If a `MustMatch` fixture is still temporarily marked `AllowedToFail`, that is
tracked as rollout debt rather than a stable exception. Those fixtures must be
tagged `parity-debt` until the gap is closed.

`LegacyParity::MayDiffer` means the fixture is an intentional difference from
legacy behavior. These cases still require deterministic HTML5 behavior, but
they do not block the cutover on legacy equivalence.

When a new parity-sensitive fixture is added, it must declare:

- which parity category it belongs to
- whether it must match legacy or may differ intentionally
- a reason when the difference is intentional
