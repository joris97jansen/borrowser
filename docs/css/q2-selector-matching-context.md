# Q2: Introduce Selector Matching Context And DOM Query Abstraction

Last updated: 2026-04-14  
Status: implemented

This document is the source-of-truth contract for Milestone Q issue 2:
introducing the selector matching context abstraction and the centralized DOM
query helpers future selector evaluation will use.

Historical scope note:

- this document records the Q2 landing boundary for the matcher-facing
  context/query layer
- later Q issues extended the matcher built on top of that boundary
- current matcher behavior is therefore broader than the original Q2 landing
  scope, but the context contract documented here remains the same

Milestone Q issue 1 established the DOM-facing selector matching contract in
`SelectorMatchDom`, explicit matchability handling, deterministic match-result
surfaces, and a deterministic owned-tree DOM adapter. Q2 builds on that by
introducing the matcher-facing context/query layer that sits between raw DOM
access and future selector evaluation logic.

At Q2 landing time, this issue did not yet implement full selector matching.
It defined and shipped the query/context surface the later matcher must use.

Related code:
- `crates/css/src/selectors/matching.rs`
- `crates/css/src/selectors/matching/context.rs`
- `crates/css/src/selectors/matching/result.rs`
- `crates/css/src/selectors/matching/dom_index.rs`
- `crates/css/src/selectors/mod.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/q1-selector-matching-architecture.md`
- `docs/css/p1-selector-architecture.md`
- `docs/css/p2-selector-ir-data-structures.md`

## Implemented Result

Milestone Q issue 2 now has an explicit matcher-facing query layer:

- `SelectorMatchingContext<'a, D>`
- `AncestorElements<'a, D>`
- `PreviousSiblingElements<'a, D>`

The context centralizes:

- parent and previous-sibling access
- nearest-first ancestor traversal
- nearest-first previous-sibling traversal
- child/descendant/sibling relationship queries
- element name lookup
- id/class/attribute lookup
- supported simple-selector query helpers for:
  - type selectors
  - id selectors
  - class selectors
  - attribute selectors

This means later selector matching can be written against one coherent context
surface rather than scattering DOM access and simple-selector semantics across
multiple matcher branches.

## Implementation Organization

Q2 originally landed as one cohesive `matching.rs` module. That was deliberate
for the architecture stage where the stable seams were still being defined.

After Q5 delivered full complex-selector evaluation for the supported selector
IR, the matcher was split along those now-stable seams:

- `matching/result.rs`
- `matching/context.rs`
- `matching/dom_index.rs`
- `matching/tests.rs`

This means the Q2 contract remains intact, but it no longer lives in a single
file.

## Why This Exists

Q1 intentionally stopped at the DOM contract and match-result surfaces. That
was the right architecture-first move, but it still left a practical gap:

- future matcher code would have had to call `SelectorMatchDom` directly
- relationship walks like ancestor or previous-sibling traversal could have
  ended up duplicated in multiple places
- simple-selector DOM query semantics could have drifted between matcher
  branches

Q2 closes that gap by defining one canonical query surface for supported
selector matching.

## Layer Boundary

The selector matching stack is now:

1. `SelectorMatchDom`
   - raw DOM-provider contract
   - storage-agnostic
   - does not define matcher helper semantics
2. `SelectorMatchingContext`
   - matcher-facing query layer
   - owns traversal helpers and simple-selector query semantics
   - at Q2 landing time, did not yet evaluate complex selectors directly
3. later matcher implementation
   - combines selector IR plus context queries into full selector evaluation
4. later cascade work
   - consumes `SelectorListMatchOutcome`

Normative rule:

- future selector evaluation should use `SelectorMatchingContext`
  rather than issuing ad hoc DOM traversals against `SelectorMatchDom`

## Matching Context Contract

`SelectorMatchingContext` is a lightweight borrowed wrapper over one
`SelectorMatchDom` implementation.

Its responsibilities are:

- expose the DOM provider to selector evaluation in a constrained way
- centralize DOM relationship helpers
- centralize supported simple-selector query behavior
- keep matcher logic independent from one concrete DOM representation

It does not:

- own DOM storage
- cache selector results
- evaluate full complex selectors
- resolve cascade winners

### Relationship Queries

The context provides explicit helpers for the relationships needed by the
current supported selector subset:

- `parent_element(element)`
- `previous_sibling_element(element)`
- `ancestor_elements(element)`
- `previous_sibling_elements(element)`
- `is_child_of(element, parent)`
- `is_descendant_of(element, ancestor)`
- `is_next_sibling_of(element, sibling)`
- `is_subsequent_sibling_of(element, sibling)`

Traversal guarantees:

- ancestor traversal is nearest-first
- previous-sibling traversal is nearest-first
- the subject element itself is excluded from both iterators
- traversal is element-only because the underlying DOM contract is
  element-only

These guarantees are normative because later combinator matching depends on
them.

### Identity

The context exposes `same_element(left, right)` as the centralized identity
comparison helper for matcher code.

The identity source remains `SelectorMatchDom::ElementId`. The matcher should
not infer identity from debug strings, storage slots, or any incidental DOM
representation detail.

### Simple-Selector Query Helpers

The context now owns the matcher-facing helpers for the supported simple
selector subset:

- `matches_type_selector(...)`
- `matches_id_selector(...)`
- `matches_class_selector(...)`
- `matches_attribute_selector(...)`
- `matches_subclass_selector(...)`

This is the required place for those semantics. Future matcher code should not
re-encode them inline.

## Supported Query Semantics

### Type Selectors

For Borrowser's current HTML path:

- universal selectors always match
- named type selectors use ASCII case-insensitive comparison against the
  canonical element name surface

This keeps type matching aligned with the current HTML-oriented engine
behavior.

### ID And Class Selectors

For the current supported subset:

- id selector value matching is exact and case-sensitive
- class selector token matching is exact and case-sensitive
- class tokenization uses selector/HTML whitespace splitting rules

### Attribute Selectors

The context implements the current supported attribute selector subset:

- existence: `[attr]`
- exact: `[attr=value]`
- includes: `[attr~=value]`
- dash-match: `[attr|=value]`
- prefix: `[attr^=value]`
- suffix: `[attr$=value]`
- substring: `[attr*=value]`

Current behavior:

- attribute-name lookup is delegated through the DOM provider contract
- exact matching compares the full effective attribute value
- includes matching treats the attribute value as a whitespace-separated token
  list
- includes matching fails for an empty selector value or a selector value that
  contains selector whitespace
- prefix/suffix/substring matching fail for an empty selector value
- dash-match follows the exact-or-hyphen-prefix rule

These semantics are centralized in the context so the later matcher can depend
on one implementation path.

## Test Surface

Q2 adds unit coverage for:

- nearest-first ancestor traversal
- nearest-first previous-sibling traversal
- relationship helper behavior
- type/id/class selector query behavior
- supported attribute matcher behavior, including representative edge cases

This is intentionally query-layer coverage, not full matcher coverage. Full
selector evaluation belongs to later issues.

## Extension Points

Q2 is designed for future selector growth:

- future selector classes can add new context query helpers without changing
  the raw DOM-provider contract unnecessarily
- future DOM providers can implement `SelectorMatchDom` and reuse the same
  matcher-facing context
- future matcher optimization can change internal evaluation strategy without
  changing the query-layer semantics

This keeps the context appropriate for later pseudo-classes, structural
queries, or backend-specific DOM providers.

## Non-Goals

Q2 does not:

- implement full complex-selector evaluation
- replace the temporary cascade matcher yet
- add caching or invalidation
- introduce computed-style or cascade logic into the selector subsystem

Those remain later Milestone Q work.

## Exit Criteria

Q2 is complete when:

- a selector matching context abstraction exists in code
- DOM access for matching is routed through explicit query interfaces
- matcher dependencies on DOM structure are centralized
- context/query behavior is testable and documented

Repository status:

- the Q2 selector matching context issue is complete and may be treated as
  closed
- later matcher implementation should be written against
  `SelectorMatchingContext` plus `SelectorListMatchBuilder`
## AE11 namespace context

`SelectorMatchingContext` carries an optional typed
`SelectorNamespaceConstraint`. Author rule inputs use `Unconstrained`; HTML UA
rule groups use `Exact(ElementNamespace::Html)`. The constraint is evaluated
at every compound reached during right-to-left combinator traversal, not only
for the original candidate. Consequently foreign lookalikes cannot satisfy
HTML-only UA compounds in selectors such as `html body`, `html .notice`,
`body > *`, or `.notice`. Type matching remains ASCII-insensitive for HTML and
case-sensitive for canonical foreign local names. Unprefixed attribute queries
remain limited to `AttributeNamespace::None`.

This internal context does not implement CSS `@namespace`, prefixed selectors,
or a public namespace-aware selector API.

The deterministic selector-DOM debug surface is `version: 2`. Each indexed
element records its selector identity as `namespace=<html|svg|mathml>` plus the
exact canonical `local` name, followed by its existing index, parent, and
previous-element-sibling fields. This prevents HTML, SVG, and MathML
lookalikes from becoming indistinguishable in matching diagnostics; adjusted
SVG case such as `foreignObject` is preserved exactly.
