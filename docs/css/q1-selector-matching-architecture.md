# Q1: Define Selector Matching Architecture And DOM Contract

Last updated: 2026-04-11  
Status: architecture contract implemented

This document is the source-of-truth contract for Milestone Q issue 1:
selector matching architecture, DOM interaction rules, matching invariants,
and the extension boundaries later selector-matching work must follow.

Milestone P established Borrowser's selector IR, specificity model, and the
explicit `Parsed | Unsupported | Invalid` selector parse-result contract.
Milestone Q builds on that foundation to define how parsed selectors are
matched against DOM elements without coupling selector semantics to one DOM
storage format, parser mode, or cascade implementation detail.

This issue does not finish selector evaluation. It defines the architecture
and code contract the later matching implementation must obey.

Related code:
- `crates/css/src/selectors/mod.rs`
- `crates/css/src/selectors/matching.rs`
- `crates/css/src/lib.rs`
- `crates/browser/src/dom_store/arena.rs`
- `crates/html/src/types.rs`

Related documents:
- `docs/css/p1-selector-architecture.md`
- `docs/css/p2-selector-ir-data-structures.md`
- `docs/css/p4-specificity-calculation.md`
- `docs/css/p5-invalid-selector-handling.md`
- `docs/css/p6-unsupported-selector-handling.md`
- `docs/css/p8-selector-model-integration.md`
- `docs/html5/node-identity-contract.md`

## Implemented Result

Milestone Q issue 1 now has an explicit in-repository architecture contract
for selector matching:

- a DOM adapter boundary in `css::selectors::matching::SelectorMatchDom`
- a deterministic element-only DOM indexing surface for the owned
  `html::Node` tree path in `SelectorDomIndex`
- an explicit match-result surface in `SelectorListMatchOutcome`
- a deterministic matched-result construction path in
  `SelectorListMatchBuilder`
- a defined integration rule from selector parse result state into selector
  matchability through `SelectorMatchability`
- deterministic debug snapshots for the selector DOM index and selector match
  outcomes

This means later matching work can now be implemented against a stable DOM
contract and result surface rather than reaching directly into ad hoc DOM
details or re-inventing selector applicability behavior inside cascade code.

## Why This Exists

Before Q1, the repository had:

1. a production selector IR in `css::selectors`
2. a temporary compatibility matcher inside `css::cascade`
3. two relevant DOM shapes:
   - the owned `html::Node` snapshot tree
   - the runtime patch-applier arena in `browser::dom_store`

That left a major architectural gap: full selector matching needs parent and
sibling traversal, but `css` cannot depend directly on the browser crate's
runtime arena, and the owned tree does not expose parent links directly.

Without an explicit matching contract, later work would be pushed toward one
of two bad outcomes:

- selector matching grows against one incidental DOM representation and later
  needs to be rewritten for another
- matching semantics leak into cascade code again through temporary bridging

Q1 exists to close that gap before the full matcher lands.

## Layer Boundary

The selector matching boundary is now:

1. `css::syntax`
   - owns tokenization and syntax recovery
   - does not own selector matching
2. `css::selectors`
   - owns selector IR
   - owns selector specificity
   - owns `Parsed | Unsupported | Invalid` selector state
   - owns the DOM-facing selector matching contract
   - does not own cascade winner resolution or computed style generation
3. DOM providers
   - expose the minimal element relationships and attribute/name lookups
     required by matching through `SelectorMatchDom`
   - may store the DOM however they want as long as they honor the contract
4. later cascade work
   - consumes selector match results plus selector specificity
   - resolves declaration winners
   - stays separate from selector-evaluation mechanics

Normative rule:

- selector evaluation consumes selector IR plus a `SelectorMatchDom`
  implementation
- selector evaluation must not depend on parser-internal state, raw selector
  source reparsing, or one specific DOM storage layout

## DOM Contract

Selector matching is defined over elements only.

The engine is allowed to depend on exactly these DOM-facing facts:

- nearest parent element
- nearest previous element sibling
- canonical element name
- deterministic attribute presence lookup
- deterministic attribute value lookup

The current code contract is the `SelectorMatchDom` trait:

- `parent_element(element)`
- `previous_sibling_element(element)`
- `element_name(element)`
- `has_attribute(element, name)`
- `attribute_value(element, name)`

Everything else is intentionally out of contract for Q1.

Selector matching is not allowed to depend on:

- raw parser insertion-mode state
- DOM builder open-element stacks
- node allocation order outside the adapter contract
- text/comment/document nodes as match subjects
- style data already attached to the DOM
- computed values
- layout state
- browser event/input state

### Element Axes

For the matching contract:

- selectors match elements only
- document nodes never match selectors
- text/comment nodes never match selectors
- child and descendant traversal use element ancestors only
- adjacent and general sibling traversal use previous element siblings only
- non-element siblings are skipped for sibling combinators

This is the required invariant that keeps matching stable across the owned
tree path and the runtime arena path even though the underlying node storage
differs.

### Name And Attribute Semantics

For Borrowser's current HTML DOM:

- element names are canonical lowercase ASCII tag names
- attribute-name lookup is ASCII case-insensitive
- attribute-value comparison for the Milestone Q supported selector subset is
  exact and case-sensitive unless a later selector feature explicitly changes
  that rule
- duplicate stored attributes must resolve deterministically through the DOM
  adapter; the owned `html::Node` adapter uses the first matching attribute in
  storage order

This is important for determinism: selector matching depends on the adapter's
effective name/value surface, not on incidental parser recovery history.

The canonical-name assumption is explicit, not incidental. The current owned
HTML path relies on the HTML layer's atom/canonicalization guarantees:

- `crates/html/src/types.rs` documents canonical lowercase tag/attribute name
  storage for the current `AtomTable`
- `crates/html/src/html5/shared/atom.rs` documents ASCII-lowercasing for the
  HTML atomization path

The selector adapter therefore treats non-canonical HTML element names as an
upstream invariant violation rather than silently normalizing them.

Duplicate-attribute resolution is adapter policy, not a universal raw-storage
rule. The trait requires deterministic effective attribute lookup. Each DOM
provider defines how ambiguous storage is collapsed into that effective view.

## Matching Contract

### Matchability

Selector parse state directly constrains matching:

- `Parsed` selector lists are matchable
- `Unsupported` selector lists are explicit non-matchable inputs
- `Invalid` selector lists are explicit non-matchable inputs

Q1 formalizes that with `SelectorMatchability` and
`SelectorListParseResult::matchability()`.

Normative rule:

- unsupported or invalid selector lists must not be silently downgraded to
  `NotMatched`
- matching consumers must be able to distinguish:
  - parsed but not matched
  - unsupported and therefore not matchable
  - invalid and therefore not matchable

### Match Result Shape

`SelectorListMatchOutcome` is the selector-engine result surface for one
selector list against one target element.

Its guarantees are:

- matchability state is explicit
- matched selector entries are carried in source-order index form
- `selector_index` is the authoritative source-order identity for one selector
  list entry
- matched selector entries are deduplicated by selector index
- duplicate selector indices with differing specificity are invalid internal
  state
- specificity is attached per matched selector entry
- highest matched specificity is derived from the matched entries, not
  recomputed through a separate code path

This is the required handoff shape for later cascade work because one rule may
contain multiple comma-separated selectors that all match the same element.

The intended matcher-side construction path is `SelectorListMatchBuilder`:

- the matcher records one selector-list hit at a time
- selector indices are coalesced as they are recorded
- source order is preserved by construction
  because `selector_index` is the ordering identity, not discovery order
- conflicting specificity for one selector index remains a debug-time engine
  invariant violation

The lower-level duplicate-normalization path remains only as a defensive
backstop inside the selector subsystem, not as the preferred matcher contract.

## Evaluation Strategy

The normative matching strategy for Milestone Q is:

1. evaluate selector lists in source order
2. evaluate one complex selector right-to-left from the target element
3. evaluate one compound selector as a conjunction of its simple selectors
4. on combinators, move along the required DOM axis and continue matching the
   selector segment to the left

High-level rules:

- the rightmost compound is tested against the subject element first
- descendant combinators walk parent elements until a match is found or the
  root is reached
- child combinators test only the nearest parent element
- next-sibling combinators test only the nearest previous element sibling
- subsequent-sibling combinators walk previous element siblings until a match
  is found or siblings are exhausted

Within one compound selector, evaluation is deterministic:

- type or universal selector first when present
- subclass selectors after that in source order

Matching may short-circuit on failure, but any optimized implementation must
remain observationally equivalent to this normative algorithm.

## Supported Scope For Milestone Q

Q1 locks the matching scope for the first implementation of the Milestone Q
engine.

In scope:

- matching parsed selector IR from Milestone P
- selector lists
- complex selectors
- compound selectors
- combinators:
  - descendant
  - child
  - next sibling
  - subsequent sibling
- simple selectors:
  - universal
  - named type
  - id
  - class
  - attribute existence
  - attribute match operators already in the selector IR
- integration of selector validity/support state with matching
- deterministic debug and regression surfaces for DOM indexing and match
  outcomes

Out of scope for Q1 and still deferred beyond this issue:

- pseudo-classes
- functional pseudo-classes
- pseudo-elements
- namespaces
- relative selectors
- nesting selector `&`
- forgiving selector lists
- selector matching caches or invalidation systems
- cascade winner resolution
- computed style generation
- layout-facing selector optimization

## Determinism Requirements

The selector engine must behave identically when:

- the same selector IR is provided through different parse-call paths
- the DOM was built through different construction paths but exposes the same
  element relationships and effective attribute/name surface through
  `SelectorMatchDom`

Required invariants:

- selector list order is source order
- matched selector indices are reported in source order
- specificity values come from the selector IR and are stable
- element axes are defined only in terms of parent element and previous
  element sibling
- matching never depends on raw node ids assigned by one DOM builder path
- matching never depends on text/comment node placement except insofar as
  sibling traversal skips them deterministically
- unsupported and invalid selector states remain distinguishable from ordinary
  no-match results

## Extension Points

Q1 intentionally leaves explicit places for later selector-class expansion.

### DOM Surface Extension

The core `SelectorMatchDom` trait is minimal because the supported subset only
needs parent/sibling traversal and attribute/name queries.

Later selector classes may extend matching through additional DOM-side
contracts, for example:

- namespace resolution
- structural pseudo-class predicates
- stateful pseudo-classes
- shadow-DOM or scoped-tree boundaries later if Borrowser grows them

Those additions must extend the DOM contract explicitly. They must not smuggle
new dependencies in through unrelated cascade or layout APIs.

### Selector Evaluation Extension

The matching architecture is explicitly split into:

- compound simple-selector evaluation
- combinator axis traversal
- selector-list result aggregation

Future selector classes should attach to one of those surfaces rather than
rewriting the whole engine contract.

### Regression Surface Extension

Q1 introduces deterministic snapshots for:

- indexed selector DOM shape
- selector match outcomes

Later matching work must preserve these stable surfaces or replace them with
strictly better explicit serializers. Rust derived `Debug` is not the
contract.

## Owned Tree Adapter

`SelectorDomIndex` is the Q1 adapter for the existing owned `html::Node`
surface.

It exists for two reasons:

- the owned tree does not store parent links directly
- matching must not depend on `Node::id()` assignment behavior

`SelectorDomIndex` therefore:

- walks the owned tree iteratively
- indexes elements in document order
- stores only selector-relevant data
- assigns its own deterministic element ids independent from DOM node ids
- skips non-element nodes for match subjects and sibling axes
- treats canonical lowercase HTML element names as an adapter invariant
- normalizes unexpected nested `Node::Document` values by splicing their
  children into the surrounding traversal frame while preserving the current
  element-axis context

This is intentionally a clean adapter layer, not the permanent proof that all
matching must use an indexed tree. Later DOM providers may implement
`SelectorMatchDom` differently as long as the observable contract is the same.

## Non-Goals

Q1 does not:

- replace the temporary compatibility matcher inside `css::cascade`
- implement full selector evaluation
- decide cascade winner ordering
- decide computed-style attachment strategy
- introduce selector caches, bloom filters, or invalidation machinery
- bind the selector engine permanently to `html::Node` or to the browser DOM
  arena

Those belong to later Milestone Q issues.

## Exit Criteria

Q1 is complete when:

- selector matching architecture is documented
- the DOM interaction contract is explicit in code and docs
- selector matchability rules are explicit
- deterministic match-result and selector-DOM debug surfaces exist
- Milestone Q scope and non-goals are unambiguous
- future extension points are identified

Repository status:

- the Q1 selector matching architecture issue is complete and may be treated
  as closed
- later Milestone Q work should implement selector evaluation against
  `SelectorMatchDom` and return `SelectorListMatchOutcome`
- cascade migration should consume those results rather than extending the
  current compatibility matcher further
