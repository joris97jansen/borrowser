# Q7: Add Deterministic Selector Matching Debug Output And Regression Coverage

Last updated: 2026-08-16
Status: implemented

This document is the source-of-truth contract for Milestone Q issue 7:
providing stable debug output and regression coverage for selector matching
behavior across representative selector and DOM combinations.

Related code:
- `crates/css/src/selectors/matching.rs`
- `crates/css/src/selectors/matching/debug.rs`
- `crates/css/src/selectors/matching/dom_index.rs`
- `crates/css/src/selectors/matching/result.rs`
- `crates/css/src/selectors/matching/tests.rs`

Related documents:
- `docs/css/q1-selector-matching-architecture.md`
- `docs/css/q5-combinator-complex-selector-matching.md`
- `docs/css/q6-validity-specificity-match-results.md`
- `docs/css/af4b-selector-dom-query-contract.md`
- `docs/css/af4c-html-host-language-selector-comparison.md`

## Implemented Result

Q7 adds an integrated selector-matching snapshot surface through:

- `SelectorDomIndex::to_matching_debug_snapshot(matching_environment, ...)`

The matching environment is an explicit input to this surface. It is the
CSS-owned immutable `SelectorMatchingEnvironment` containing the parser-selected
`DocumentMode`; it does not contain Browser/runtime identity or DOM generation.
Selector semantics remain owned by CSS.

After successful selector-DOM construction, this snapshot combines, in one
deterministic output:

- the selector parse-result snapshot body
- the validated selector DOM projection snapshot body
- one selector-match outcome per indexed element in document order

That makes it possible to inspect not just whether a selector matched, but how
matching behaved across a whole DOM case.

## Snapshot Shape

The snapshot format is stable and versioned.

It records:

- the explicit matching environment
- selector parse state and selector IR structure
- explicit document or element-subtree projection provenance
- actual document-element identity, independently from parentlessness
- the validated selector DOM facts used by the matcher
- per-target match outcomes in document order
- explicit matchability and specificity data for each target

AF4a's integrated selector-matching snapshot is `version: 2` and includes the
deterministic line:

```text
matching-environment: document-mode=<no-quirks|limited-quirks|quirks>
```

The environment line exposes only the semantic document mode. It does not
expose Browser/runtime identity or DOM generation. Lower-level selector parse,
DOM, and match-outcome snapshot formats retain their own existing versions.

AF4b advances the integrated selector-matching snapshot to `version: 3`
because its embedded DOM body now records projection provenance, actual
document-element identity, forward sibling/direct-child links, neutral ordered
attributes, and exact owner-grouped direct text.

This keeps the debug surface aligned with the internal selector subsystem
models rather than inventing a separate ad hoc representation.

AF4c changes environment-dependent match outcomes without changing the
snapshot schema. In particular, otherwise equivalent NoQuirks/LimitedQuirks
and Quirks cases may differ for `#id` and `.class`, while `[id...]` and
`[class...]` continue to follow attribute-selector value policy. HTML default
attribute-value policy may likewise change a match outcome. These are semantic
changes represented by existing outcome fields, so the integrated snapshot
remains `version: 3`.

## Determinism Requirements

Q7 snapshot output is deterministic by contract:

- selector parse state is serialized through the existing stable selector
  snapshot body
- DOM structure is serialized through the deterministic `SelectorDomIndex`
  projection
- the matching environment is serialized in a fixed field and canonical mode
  spelling
- target elements are evaluated in document order
- each target uses the stable `SelectorListMatchOutcome` snapshot body
- equivalent valid DOM constructions that project to the same selector DOM
  produce the same integrated snapshot

AF4b adds parent, previous/next element sibling, direct element-child, ordered
neutral attribute, and exact owner-grouped direct-text facts. Source DOM IDs and
the global direct-text arena layout are not snapshot identities or whole-DOM
text order. Debug convenience APIs that construct an index return `Result`;
invalid structure is a typed build error, never a successful string containing
an error line or an empty projection.

## Regression Coverage

Q7 adds exact-snapshot regression tests for representative cases:

- simple selector lists
- compound selector matching on one element
- complex selector matching with combinator traversal
- invalid selector input propagated through the integrated debug surface
- unsupported selector input propagated through the integrated debug surface
- representative AF4c NoQuirks-versus-Quirks outcomes that demonstrate ID/
  class behavior without attribute-selector leakage

These tests are intentionally exact string snapshots so future matcher work
cannot silently change debug behavior or output ordering.

## Scope And Non-Goals

Q7 does not:

- replace the lower-level DOM or match-outcome snapshots
- add cascade or computed-style debug output
- introduce separate fixture file formats outside the existing Rust regression
  surface

The integrated snapshot is an additional maintenance surface for later selector
and cascade milestones, not a replacement for the underlying model-level
snapshots.

## AF4e document/style integration diagnostic

AF4e adds a separate higher-level CSS-owned diagnostic for the cross-product
of document elements, stylesheets, rules, and selectors. It lives above the
core matcher so `selectors::matching` does not depend on stylesheet or cascade
integration types. It uses the authoritative matcher directly and records
unmatched, invalid, unsupported, declaration-free, and non-contributing rules;
it is not a cascade candidate/winner report.

This production surface is explicitly bounded by stylesheet, rule, element,
selector-evaluation, report-record, report-storage, serialized-byte, matcher-traversal, and
selector-DOM construction limits. Failure discards partial success and emits a
stable top-level terminal envelope. See the AF4e closeout contract for the
field and ordering guarantees.
