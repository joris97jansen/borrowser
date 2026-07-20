# Q8: Document Selector Matching Invariants And Future Extension Hooks

Last updated: 2026-04-14  
Status: implemented

This document is the Milestone Q closeout contract for Borrowser's selector
matching engine.

It records:

- the stable selector matching contract now implemented in the repository
- the invariants later milestones must preserve
- the DOM assumptions selector matching is allowed to rely on
- the supported selector scope for Milestone Q
- the extension hooks future selector classes and optimizations must use
- the explicit handoff boundary into later cascade work

Related code:
- `crates/css/src/selectors/matching.rs`
- `crates/css/src/selectors/matching/context.rs`
- `crates/css/src/selectors/matching/result.rs`
- `crates/css/src/selectors/matching/dom_index.rs`
- `crates/css/src/selectors/matching/debug.rs`
- `crates/css/src/selectors/matching/tests.rs`

Related documents:
- `docs/css/q1-selector-matching-architecture.md`
- `docs/css/q2-selector-matching-context.md`
- `docs/css/q3-simple-selector-matching.md`
- `docs/css/q4-compound-selector-matching.md`
- `docs/css/q5-combinator-complex-selector-matching.md`
- `docs/css/q6-validity-specificity-match-results.md`
- `docs/css/q7-selector-matching-debug-output.md`
- `docs/css/p1-selector-architecture.md`
- `docs/css/p4-specificity-calculation.md`
- `docs/css/p5-invalid-selector-handling.md`
- `docs/css/p6-unsupported-selector-handling.md`

## Implemented Subsystem Boundary

At the end of Milestone Q, the selector matching stack is:

1. selector parsing and IR in `css::selectors`
2. DOM/query access through `SelectorMatchDom` and `SelectorMatchingContext`
3. selector evaluation through the Milestone Q matcher
4. match-result reporting through `SelectorListMatchOutcome`
5. debug/regression output through the stable snapshot surfaces

Normative ownership rules:

- `css::selectors` owns selector structure, specificity, matchability, and
  matching semantics
- DOM providers own tree storage and expose only the selector-facing facts the
  matcher is allowed to consume
- cascade is a consumer of selector match results; it does not reinterpret
  selector validity, reparses selector text, or re-derive selector specificity

## DOM Interaction Assumptions

Selector matching is defined over an element-only, acyclic DOM view.

The matcher is allowed to depend on exactly these DOM-facing facts:

- nearest parent element
- nearest previous element sibling
- canonical element name
- deterministic attribute presence lookup
- deterministic attribute value lookup

Current DOM invariants:

- non-element nodes never match selectors directly
- descendant and child traversal operate over element ancestors only
- adjacent and general sibling traversal operate over previous element
  siblings only
- text, comment, and document nodes are skipped for selector axes
- the owned `SelectorDomIndex` adapter normalizes unexpected nested document
  nodes by splicing their children into the surrounding element traversal
  frame
- current HTML-backed matching relies on canonical lowercase element names and
  deterministic, adapter-defined duplicate-attribute collapse

Future DOM providers must preserve those selector-facing invariants explicitly.

## Supported Milestone Q Selector Scope

Milestone Q fully matches the parsed selector subset established by Milestone P:

- universal selectors
- named type selectors
- id selectors
- class selectors
- supported attribute selectors
- compound selectors on one element
- complex selectors with:
  - descendant combinators
  - child combinators
  - adjacent-sibling combinators
  - general-sibling combinators

Selectors outside that parser/model subset remain parser-level `Unsupported`
or `Invalid` inputs rather than being partially reinterpreted by matching.

## Matching Invariants

### Input And Matchability

- selector matching consumes selector IR, not raw selector source reparsing
- parsed selector lists are matchable
- unsupported selector input remains explicitly unsupported
- invalid selector input remains explicitly invalid
- unsupported or invalid selector input must never collapse into an ordinary
  parsed no-match result

### Evaluation

- selector lists are evaluated in source order
- complex selectors are evaluated right-to-left
- the rightmost compound is tested against the subject element first
- compound selectors evaluate as conjunctions on one element
- type/universal matching runs before subclass matching within one compound
- subclass selectors are evaluated in source order
- ancestor and previous-sibling searches are nearest-first
- descendant and general-sibling matching backtrack across structural
  candidates until the remaining left-hand selector chain succeeds or
  candidates are exhausted
- optimized implementations added later must remain observationally equivalent
  to these semantics

### Result Model

- `selector_index` is the authoritative source-order identity for one selector
  entry inside a selector list
- match results report only selectors that actually matched the target element
- specificity is taken directly from selector IR
- highest specificity is derived from actual matched entries only
- matched entries are deduplicated by `selector_index`
- conflicting specificity for the same `selector_index` is invalid internal
  state
- unsupported and invalid outcomes never carry matched selectors or usable
  specificity

### Determinism

- equivalent DOM construction paths that expose the same selector-facing axes
  and effective attribute/name surface must produce the same results
- equivalent raw selector formatting must produce the same results once parsed
  into the same selector IR
- selector DOM ids used by the owned adapter are document-order ids derived
  from the selector-facing projection, not borrowed from incidental source node
  ids
- debug and regression surfaces are versioned and deterministic

## Debug And Regression Contract

Milestone Q now has three stable selector-matching debug surfaces:

- selector parse snapshots
- selector DOM snapshots
- selector match-outcome snapshots

Q7 adds the integrated selector-matching snapshot, which combines:

- selector parse result
- normalized selector DOM
- one selector-match outcome per indexed element in document order

Regression coverage exists for:

- simple selector cases
- compound selector cases
- complex selector cases
- invalid and unsupported propagation
- specificity/result-shape invariants
- equivalent DOM construction paths
- equivalent raw selector formatting

Future matcher work should extend these deterministic surfaces rather than
adding ad hoc, unstable diagnostics.

## Extension Hooks

Future selector work should attach to the subsystem through explicit seams
rather than modifying unrelated cascade or DOM code.

### DOM-Side Extension

If a new selector class needs additional DOM facts, extend:

- `SelectorMatchDom`
- `SelectorMatchingContext`

Normative rule:

- new selector dependencies must be added as explicit DOM/query contract
  surface
- new selector semantics must not smuggle dependencies in through layout,
  event, or cascade state

Examples of later DOM-side expansion:

- CSS namespace-prefix/default-namespace resolution beyond AE11's typed
  element-namespace query
- structural pseudo-class predicates
- stateful pseudo-class predicates
- scoped-tree or shadow-boundary semantics if Borrowser later grows them

### Element-Local Selector Extension

If a new selector class still matches on one element, it should extend the
element-local matcher surfaces:

- selector IR in `css::selectors`
- simple-selector query helpers in `SelectorMatchingContext`
- compound-selector dispatch in the matcher
- specificity accounting in the selector IR layer
- selector parse/debug snapshots

### Structural Selector Extension

If a new selector class introduces additional structural semantics, it should
extend:

- complex-selector evaluation rules
- traversal helpers in `SelectorMatchingContext`
- deterministic debug/regression coverage for the new traversal semantics

Examples:

- additional combinators
- relative selector semantics
- later structural pseudo-classes that require explicit tree-axis queries

### Result-Surface Extension

If later milestones need more selector metadata at match time, extend the
result surface deliberately through:

- `MatchedSelector`
- `SelectorListMatchOutcome`
- the stable snapshot serializers

Normative rule:

- extend the selector result contract only for selector-owned data
- do not mix declaration winner state, computed style data, or layout-facing
  caches into selector match outcomes

## Intentionally Deferred Beyond Milestone Q

Milestone Q does not implement:

- cascade winner resolution
- computed style generation
- selector invalidation
- selector matching caches
- traversal pruning heuristics
- CSS `@namespace` and namespace-selector syntax
- pseudo-classes
- functional pseudo-classes
- pseudo-elements
- relative selectors
- nesting selector semantics
- shadow-DOM or scoped-tree selector behavior

These remain later work and must build on the stable contracts above rather
than weakening them.

## Cascade Handoff

Milestone Q hands later cascade work a stable selector-owned input surface:

- explicit parse/matchability state
- deterministic matched selector entries
- authoritative `selector_index`
- IR-derived specificity
- deterministic debug/regression artifacts

That is the clean handoff point into cascade winner resolution:

- selector matching is complete for the current supported subset
- selector semantics are no longer owned implicitly by temporary cascade code
- later cascade work can focus on rule/declaration ordering and winning-value
  selection rather than selector-engine ambiguity

## Completion

Milestone Q can now be treated as complete.

The selector matching engine is no longer just working code. It is a documented
subsystem with:

- explicit DOM assumptions
- explicit matching invariants
- explicit supported scope
- explicit extension hooks
- explicit deferred work
- explicit cascade handoff semantics
