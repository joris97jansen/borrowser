# Q8: Document Selector Matching Invariants And Future Extension Hooks

Last updated: 2026-08-16
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
- `crates/css/src/selectors/matching/comparison.rs`
- `crates/css/src/selectors/matching/host_language.rs`
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
- `docs/css/af4b-selector-dom-query-contract.md`
- `docs/css/af4c-html-host-language-selector-comparison.md`
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

- actual document-element identity
- nearest parent element
- nearest previous element sibling
- nearest next element sibling
- canonical element local name and namespace
- ordered neutral attribute namespace/local-name/value facts
- ordinary direct element children
- exact ordinary direct text children

Current DOM invariants:

- non-element nodes never match selectors directly
- descendant and child traversal operate over element ancestors only
- adjacent and general sibling traversal operate over previous element
  siblings only
- text, comment, processing-instruction, and doctype nodes are skipped for
  element sibling axes
- the root document is the projection container and is outside element sibling
  axes
- the owned `SelectorDomIndex` adapter rejects unexpected nested document nodes
  as typed construction failures before they can be represented on any axis;
  they are never skipped through, flattened, or normalized
- current HTML-backed matching relies on canonical element names and ordered
  neutral attributes; CSS owns effective lookup and selector comparison policy
- template-associated fragment contents stay outside the host's ordinary child
  and text axes

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

AF4a extension invariant: document-mode metadata is an explicit context
dependency. A missing or changed environment is an invariant failure for
matching/reuse, not a selector-visible NoQuirks fallback. Browser identity and
DOM generation remain outside the CSS environment.

AF4c comparison invariants:

- CSS selects comparison policy centrally and executes operators only after a
  typed policy has been selected
- HTML type-selector and unqualified attribute-selector names lowercase ASCII
  on the selector/request side only and then compare identically to the exact
  actual name; foreign names compare exactly
- host-language name matching is not modeled as symmetric ASCII-insensitive
  value equality
- ID/class values are ASCII-insensitive only in `DocumentMode::Quirks`;
  `NoQuirks` and `LimitedQuirks` remain sensitive
- Quirks ID/class policy is document-wide and is not namespace-gated
- attribute selectors named `id` or `class` never inherit ID/class selector
  quirks behavior
- attribute-value policy is selected from candidate element namespace plus the
  complete effective attribute namespace and exact actual local name, never
  raw selector spelling
- the canonical 46-name HTML insensitive-value inventory is an exact, static,
  allocation-free lookup and applies only to unqualified attributes on HTML
  elements
- operator execution preserves the independent empty-value semantics of `=`,
  `~=`, `|=`, `^=`, `$=`, and `*=`
- CSS whitespace remains exactly TAB, LF, FF, CR, and SPACE
- ASCII-insensitive value comparison folds ASCII only, preserves identical
  non-ASCII code points, and does not perform Unicode case folding
- ordinary matching neither normalizes stored values nor allocates lowercase
  copies, comparison strategies, or a heap-backed attribute-name set
- AF4c covers parser-created HTML documents; XML-document semantics and DOM
  mutation remain outside the contract

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
  and neutral attribute/name facts must produce the same results
- equivalent raw selector formatting must produce the same results once parsed
  into the same selector IR
- policy must not be reconstructed from raw authored selector spelling after
  effective attribute lookup
- selector DOM ids used by the owned adapter are document-order ids derived
  from the selector-facing projection, not borrowed from incidental source node
  ids
- document-element identity is explicit and is not inferred from the absence of
  an element parent
- a valid document may contain no document element; multiple direct document
  elements are rejected as ambiguous
- explicit element-subtree roots have no document-element identity
- debug and regression surfaces are versioned and deterministic

## Debug And Regression Contract

Milestone Q now has three stable selector-matching debug surfaces:

- selector parse snapshots
- selector DOM snapshots
- selector match-outcome snapshots

Q7 adds the integrated selector-matching snapshot, which combines:

- selector parse result
- validated selector DOM projection facts
- one selector-match outcome per indexed element in document order

Regression coverage exists for:

- simple selector cases
- compound selector cases
- complex selector cases
- invalid and unsupported propagation
- specificity/result-shape invariants
- neutral element-sibling facts across non-element node boundaries
- typed nested-document rejection at projection construction
- equivalent raw selector formatting

Future matcher work should extend these deterministic surfaces rather than
adding ad hoc, unstable diagnostics.

AF4b makes index construction fallible and explicit. Authoritative document
paths use `try_from_document`; isolated tests use the test-only unbounded
element-subtree seam, while legacy `attach_styles` uses the crate-private
bounded subtree path. Both subtree paths declare the same closed-subtree
provenance. There is no generic `from_root`, nested-document normalization, or
leaf-to-empty-projection fallback. Selector build failures remain typed through
matching, cascade, computed style, style-tree reconstruction, and Browser
callers; they are not selector no-match, unsupported/invalid state,
incremental-unavailable state, or debug text.

The full refined invariant and complexity contract is
`docs/css/af4b-selector-dom-query-contract.md`.

## Extension Hooks

Future selector work should attach to the subsystem through explicit seams
rather than modifying unrelated cascade or DOM code.

### DOM-Side Extension

If a new selector class needs additional neutral facts, extend the narrowest
CSS-facing input/query contract supplied by the subsystem that legitimately
owns those facts. Tree-local facts may extend `SelectorMatchDom` and
`SelectorMatchingContext`; future dynamic state is not automatically assigned
to the parser-created selector-DOM adapter.

Normative rule:

- selector inputs expose only the minimum neutral facts required
- each fact comes from its legitimate tree, runtime, input, focus, navigation,
  or document-state owner through an explicit query/input boundary
- storage, layout, and runtime implementation details remain behind those
  ownership boundaries
- selector and pseudo-class interpretation remains inside CSS
- new selector semantics must not be smuggled into HTML, Browser/runtime,
  Layout, Paint, cascade state, or generic DOM providers

Examples of legitimate neutral-fact expansion:

- CSS namespace-prefix/default-namespace resolution beyond AE11's typed
  element-namespace query
- additional tree relationships, document identity/provenance, exact
  child/content facts, or scoped-tree/shadow-boundary information
- neutral runtime, input, focus, navigation, or document-state facts exposed
  through a separate explicit selector-matching input/query contract where
  appropriate

Providers must not answer pseudo-specific questions such as
`is_first_child_for_css`, `is_empty_for_css`, `matches_root`,
`matches_nth_child`, `matches_hover`, `matches_focus`, or `is_active_for_css`.
CSS derives structural and stateful pseudo-class meaning from neutral facts.

### Element-Local Selector Extension

If a new selector class still matches on one element, it should extend the
element-local matcher surfaces:

- selector IR in `css::selectors`
- simple-selector query helpers in `SelectorMatchingContext`
- compound-selector dispatch in the matcher
- specificity accounting in the selector IR layer
- selector parse/debug snapshots
- selector invalidation dependency surfaces when the new semantics require it

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
- pseudo-classes beyond AF4d's typed tree-structural subset
- functional pseudo-classes
- pseudo-elements
- relative selectors
- nesting selector semantics
- shadow-DOM or scoped-tree selector behavior
- attribute selector `i` and `s` modifiers
- standards-conformant CSS escape decoding for selector semantic names/values
- XML-document selector matching

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
