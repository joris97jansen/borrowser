# Q7: Add Deterministic Selector Matching Debug Output And Regression Coverage

Last updated: 2026-04-14  
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

## Implemented Result

Q7 adds an integrated selector-matching snapshot surface through:

- `SelectorDomIndex::to_matching_debug_snapshot(...)`

This snapshot combines, in one deterministic output:

- the selector parse-result snapshot body
- the normalized selector DOM snapshot body
- one selector-match outcome per indexed element in document order

That makes it possible to inspect not just whether a selector matched, but how
matching behaved across a whole DOM case.

## Snapshot Shape

The snapshot format is stable and versioned.

It records:

- selector parse state and selector IR structure
- the normalized selector DOM used by the matcher
- per-target match outcomes in document order
- explicit matchability and specificity data for each target

This keeps the debug surface aligned with the internal selector subsystem
models rather than inventing a separate ad hoc representation.

## Determinism Requirements

Q7 snapshot output is deterministic by contract:

- selector parse state is serialized through the existing stable selector
  snapshot body
- DOM structure is serialized through the deterministic `SelectorDomIndex`
  projection
- target elements are evaluated in document order
- each target uses the stable `SelectorListMatchOutcome` snapshot body
- equivalent DOM constructions that normalize to the same selector DOM produce
  the same integrated snapshot

## Regression Coverage

Q7 adds exact-snapshot regression tests for representative cases:

- simple selector lists
- compound selector matching on one element
- complex selector matching with combinator traversal
- invalid selector input propagated through the integrated debug surface

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
