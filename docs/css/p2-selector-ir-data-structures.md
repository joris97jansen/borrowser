# P2: Introduce Selector IR Data Structures

Last updated: 2026-04-09  
Status: complete

This document records the implementation of Milestone P issue P2:
introducing the production selector intermediate representation (IR) data
structures defined by P1.

Related code:
- `crates/css/src/selectors/mod.rs`
- `crates/css/src/selectors/serialize.rs`
- `crates/css/src/selectors/tests.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/p1-selector-architecture.md`

## Implemented Result

The selector subsystem now has a concrete IR surface suitable for parser output
and later matching work.

Implemented pieces:

- explicit selector-list, complex-selector, compound-selector, combined-selector,
  combinator, type-selector, subclass-selector, attribute-selector, and
  specificity types
- invariant-enforcing construction APIs for the structural selector IR nodes
- read-only accessors for the public IR surface
- deterministic snapshot serialization for selector IR and selector parse
  outcomes
- unit tests covering valid construction, structure access, combinator/list
  ordering, and invalid structural construction

## Structural Invariants

The P2 IR now enforces the core structural rules expected of parsed selectors:

- parsed selector lists are non-empty
- parsed compound selectors are non-empty
- selector identifiers are non-empty
- child spans must stay within their owning node span
- selector parts in one node must come from the same input
- selector parts in one node must be monotonic in source order

These invariants belong to the selector IR itself, not to DOM matching or
cascade logic.

## Boundary

P2 remains data-structure work only.

It does not yet:

- parse selector IR from syntax-layer preludes
- attach selector IR to `StyleRule`
- perform selector matching
- use selector IR in cascade winner resolution

Those are later Milestone P issues.

## Exit Criteria

P2 is complete when:

- selector IR exists in code
- IR structure is explicit and testable
- selector lists and combinators are represented correctly
- IR is suitable for parser output and future matching
- IR behavior is covered by unit tests

Repository status:

- the P2 selector IR data-structures issue is complete and may be treated as
  closed
- the next Milestone P work should parse structured syntax-layer preludes into
  this IR and wire `SelectorListParseResult` into the rule/model path
- matching and cascade winner resolution remain intentionally out of scope for
  that next parser/model integration step
