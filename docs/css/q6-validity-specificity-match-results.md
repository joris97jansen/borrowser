# Q6: Integrate Selector Validity And Specificity With Matching Results

Last updated: 2026-04-14  
Status: implemented

This document is the source-of-truth contract for Milestone Q issue 6:
integrating selector validity state and selector specificity into Borrowser's
selector matching result model so later cascade work can consume it without
reinterpreting selector state.

Related code:
- `crates/css/src/selectors/matching.rs`
- `crates/css/src/selectors/matching/result.rs`
- `crates/css/src/selectors/matching/context.rs`
- `crates/css/src/selectors/matching/tests.rs`

Related documents:
- `docs/css/q1-selector-matching-architecture.md`
- `docs/css/q5-combinator-complex-selector-matching.md`
- `docs/css/p4-specificity-calculation.md`
- `docs/css/p5-invalid-selector-handling.md`

## Implemented Result

Q6 is implemented through the selector matching result surface:

- `SelectorMatchability`
- `MatchedSelector`
- `SelectorListMatchBuilder`
- `SelectorListMatchOutcome`

Together these types establish a deterministic contract for one selector list
matched against one target element.

## Validity Integration

Selector validity and support state remain explicit in the matching result:

- `Parsed`
- `Unsupported`
- `Invalid`

Normative rules:

- parsed selector lists produce `Parsed` outcomes
- parser-level unsupported selector input produces `Unsupported` outcomes
- invalid selector input produces `Invalid` outcomes
- unsupported and invalid outcomes never carry matched selectors
- unsupported and invalid outcomes expose no usable specificity

This prevents invalid selectors from collapsing into an ordinary no-match case.

## Specificity Integration

Specificity is attached only to selector entries that actually matched the
target element.

Each `MatchedSelector` carries:

- the authoritative `selector_index` identity within the selector list
- the selector's IR-derived `specificity`

`SelectorListMatchOutcome` additionally exposes:

- `matched_selectors()` for stable source-ordered matched entries
- `highest_specificity()` derived from the matched entries only

That last rule is important: unmatched selectors with higher specificity do not
contribute to the outcome's effective specificity surface.

## Cascade Boundary

Q6 intentionally stops at the match-result boundary.

The selector subsystem now provides later cascade work with:

- explicit matchability/validity state
- the set of matched selector entries
- per-match selector specificity
- deterministic source-order identity through `selector_index`

Q6 does not resolve declaration winners or compute styles. Cascade remains a
consumer of this surface rather than an owner of selector validity or
specificity semantics.

## Debug And Regression Surface

The debug snapshot for selector match outcomes explicitly records:

- matchability state
- whether any selector matched
- highest specificity, or `none` when no usable specificity exists
- each matched selector entry with selector index and specificity

That makes invalid, unsupported, and parsed-no-match outcomes unambiguous in
regression snapshots.

## Determinism Requirements

Q6 matching results are deterministic by contract:

- matched selector entries are kept in selector-list source order
- duplicate selector indices are coalesced deterministically
- conflicting specificity for one selector index is invalid internal state
- specificity values are taken from selector IR, not recomputed ad hoc
- non-matchable outcomes never leak stale matched-selector or specificity data

## Regression Coverage

Q6 coverage includes:

- explicit matchability-state tests
- deterministic matched-selector ordering
- duplicate selector-index specificity invariants
- explicit snapshots for parsed/no-match, unsupported, and invalid outcomes
- verification that highest specificity comes from actual matches only

## Non-Goals

Q6 does not:

- implement cascade winner resolution
- interpret declarations
- add selector invalidation or performance caching

Those remain later Milestone Q work.
