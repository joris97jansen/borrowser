# P4: Implement Specificity Calculation

Last updated: 2026-04-10  
Status: complete

This document records the implementation of Milestone P issue P4:
deterministic selector specificity calculation for Borrowser's supported
selector subset.

Related code:
- `crates/css/src/selectors/mod.rs`
- `crates/css/src/selectors/tests.rs`
- `crates/css/src/selectors/serialize.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/p1-selector-architecture.md`
- `docs/css/p2-selector-ir-data-structures.md`
- `docs/css/p3-selector-parser-core-subset.md`

## Implemented Result

Borrowser now exposes selector specificity as a selector-IR concern owned by
`css::selectors`.

Specificity is implemented for:

- parsed complex selectors
- compound selectors
- type selectors, including universal selectors
- subclass selectors
- id selectors
- class selectors
- supported attribute selectors

The specificity model is represented by `Specificity`, a strongly typed CSS
tuple `(a, b, c)` with saturating arithmetic.

Specificity is intentionally defined per parsed selector, not per selector
list. A selector list may contain multiple selectors with different
specificities, so `SelectorList` does not expose a single `specificity()`
shortcut.

## Architectural Boundary

Specificity is:

- derived from selector IR rather than from raw source strings
- independent from DOM matching
- independent from cascade winner resolution
- deterministic for identical selector IR

`css::cascade` remains a consumer of selector specificity rather than its
owner.

## Supported Subset Rules

For the currently supported selector subset:

- named type selectors contribute `(0, 0, 1)`
- universal selectors contribute `(0, 0, 0)`
- id selectors contribute `(1, 0, 0)`
- class selectors contribute `(0, 1, 0)`
- supported attribute selectors contribute `(0, 1, 0)`
- combinators do not contribute specificity directly
- complex-selector specificity is the sum of its compound-selector
  specificities

Unsupported and invalid selector lists do not expose usable selector
specificity.

## Regression Coverage

Representative tests cover:

- specificity across supported selector components
- specificity for parsed selectors built from real syntax-layer input
- saturating arithmetic for hostile or extreme inputs
- stable selector snapshots that include specificity labels

## Exit Criteria

P4 is complete when:

- specificity calculation exists for the supported selector subset
- specificity is derived from selector IR
- specificity behavior is deterministic
- specificity logic remains separate from matching and cascade logic
- representative specificity tests pass

Repository status:

- the P4 specificity issue is complete and may be treated as closed
- the next Milestone P work should wire selector parse results into the
  stylesheet rule/model path
- selector matching and cascade winner resolution remain intentionally out of
  scope for that step
