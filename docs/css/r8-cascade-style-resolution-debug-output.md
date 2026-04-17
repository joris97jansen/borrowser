# R8: Cascade And Style-Resolution Debug Output

Last updated: 2026-04-17  
Status: contract and code implemented

This document is the source-of-truth contract for Milestone R issue 8: stable
debug output and regression coverage for cascade candidate evaluation, winner
resolution, inheritance/defaulting, and final resolved styles.

Related code:
- `crates/css/src/cascade.rs`
- `crates/css/src/cascade/contract.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/r1-cascade-architecture-style-resolution-contract.md`
- `docs/css/r2-structured-cascade-inputs-candidate-model.md`
- `docs/css/r3-core-cascade-winner-resolution.md`
- `docs/css/r5-inheritance-behavior-supported-properties.md`
- `docs/css/r6-initial-default-value-handling.md`
- `docs/css/r7-structured-resolved-style-output.md`

## Implemented Result

R8 provides stable debug surfaces for the cascade pipeline:

- `cascade_evaluation_debug_snapshot(...)`
- `CascadeWinnerSet::to_debug_snapshot()`
- `ResolvedStyle::to_debug_snapshot()`
- `ResolvedDocumentStyle::to_debug_snapshot()`
- `resolve_document_styles_debug_snapshot(...)`

These snapshots are for regression testing and engine triage. They are not
presentation-oriented CSS serialization and they must not depend on Rust
derived `Debug` output.

## Cascade Evaluation Snapshot

`cascade_evaluation_debug_snapshot(...)` traces the rule-input to winner
portion of cascade.

It records:

- matched `CascadeRuleInput` entries
- rule source, origin, specificity, and rule order
- declaration source, declaration order, importance, property category,
  applicability, and specified value
- source-order candidates
- cascade-order candidates
- final authored winners

This is the primary surface for answering why one authored declaration beat
another.

## Style Resolution Snapshots

`ResolvedStyle::to_debug_snapshot()` remains the per-element final style
surface. It records each supported property in canonical order and makes the
source explicit:

- `winner(...)`
- `inherited`
- `initial(...)`

`ResolvedDocumentStyle::to_debug_snapshot()` records the final resolved style
for each element in selector-DOM document order.

`resolve_document_styles_debug_snapshot(...)` is the integrated trace. For each
element it records:

1. cascade candidate evaluation
2. winner output
3. final resolved style after inheritance/default fill

This surface is intentionally verbose because it is for debugging regressions
in later cascade, computed-style, and runtime cutover work.

## Determinism Requirements

R8 establishes these snapshot invariants:

- every snapshot starts with `version: 1`
- output order is canonical or document-order, never hash-map order
- source identities use stable stylesheet, rule, declaration, and inline-style
  ids
- declaration values are serialized through the structured model value surface
- unsupported/custom/invalid declarations remain visible in evaluation traces
  but do not produce candidates
- inherited and initial/default entries are explicit in resolved-style output
- document-level traces do not mutate the DOM
- snapshot label grammar is a maintained contract; changes to labels such as
  `supported(...)`, `author-normal`, or `rule-input[...]` must be treated as
  contract changes and updated in docs and regression tests together

## Regression Coverage

The regression surface now covers:

- declaration filtering and candidate materialization
- source-order versus cascade-order candidate views
- `!important` override behavior
- selector specificity and rule/declaration ordering through existing winner
  tests
- parent-to-child inheritance in integrated document traces
- default/initial fill in final resolved styles
- stable document-level resolved-style snapshots

Exact string snapshot tests are intentional. They make changes to the debug
grammar explicit and force future milestone work to update the contract when
observable cascade reasoning changes.

## Non-Goals

R8 does not:

- add computed-value debug output
- make debug output optimized for runtime performance
- replace model or selector snapshot surfaces
- serialize full CSSOM or presentation-facing CSS text

## Exit Condition For This Issue

This issue is complete when Borrowser can produce deterministic snapshots that
explain:

- which declarations entered cascade
- which declarations became candidates
- how candidates were ordered
- which declarations won
- which properties inherited
- which properties defaulted
- and what final resolved style was produced

That contract now exists and is covered by regression tests.
