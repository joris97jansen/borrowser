# R7: Structured Resolved-Style Output

Last updated: 2026-04-17  
Status: contract and code implemented

This document is the source-of-truth contract for Milestone R issue 7:
replacing ad hoc DOM-attached style mutation as the core cascade output with a
structured resolved-style surface.

Related code:
- `crates/css/src/cascade.rs`
- `crates/css/src/cascade/contract.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/r1-cascade-architecture-style-resolution-contract.md`
- `docs/css/r3-core-cascade-winner-resolution.md`
- `docs/css/r5-inheritance-behavior-supported-properties.md`
- `docs/css/r6-initial-default-value-handling.md`

## Implemented Result

R7 introduces the document-level structured cascade output:

- `ResolvedElementStyle`
- `ResolvedDocumentStyle`
- `resolve_document_styles(...)`

`resolve_document_styles(...)` is now the core cascade integration path for a
DOM root and an ordered stylesheet list. It returns resolved styles in
selector-DOM document order and does not mutate `html::Node`.

The old `attach_styles(...)` function still exists, but it is now explicitly a
legacy projection:

1. resolve structured document styles
2. project authored winner values into `Node::Element::style`
3. leave inherited and initial/default entries out of the string vector so the
   bridge-phase computed-style path keeps its existing inheritance/default
   behavior

Cascade winner selection, inheritance, and defaulting no longer depend on that
mutation path.

## Structured Output Shape

`ResolvedDocumentStyle` contains one `ResolvedElementStyle` per selector-DOM
element.

Each element entry records:

- stable selector-DOM element id for the style pass
- canonical element name
- total per-element `ResolvedStyle`

The per-element `ResolvedStyle` remains the R1-R6 contract object:

- every supported property appears exactly once
- authored winners carry source, priority, and specified value
- inherited entries are explicit
- initial/default entries are explicit

## Resolution Pipeline

For each element in selector-DOM document order, R7 performs:

1. match each model stylesheet style rule against the element through
   `SelectorMatchingContext`
2. materialize matched stylesheet declarations as `CascadeRuleInput`
3. materialize the element's inline `style` attribute as an inline
   `CascadeRuleInput`
4. resolve authored winners into `CascadeWinnerSet`
5. resolve inheritance/default fill into `ResolvedStyle`
6. store the result in `ResolvedDocumentStyle`

Parent resolved styles are available before child resolution because selector
DOM element ids are assigned in document order and parent elements precede
children.

## Legacy Projection Boundary

`html::Node::style` remains a compatibility surface only.

The projection into `Node::style` records authored winners only, serialized as
`(property, value)` pairs. It deliberately does not serialize:

- inherited entries
- initial/default entries
- unsupported/custom/invalid declarations

That keeps current computed-style and layout behavior stable while making the
structured cascade output the engine-owned result.

## Inline Style Handling

Inline style attributes are represented with:

- `InlineStyleRuleRef`
- `InlineStyleDeclarationRef`
- `CascadeSpecificity::InlineStyle`

Inline styles do not rely on a sentinel rule order for precedence. Their
author-level priority comes from `CascadeSpecificity::InlineStyle`; their
rule-order field is a normal deterministic order assigned by the document
integration pass after stylesheet rules have been enumerated for the element.

The model layer does not yet expose a first-class declaration-list parse
entrypoint. Until that exists, R7 keeps inline declaration materialization
localized inside the cascade integration layer while still converting inline
style attributes into structured model declarations before they enter
candidate/winner resolution.

## Determinism Requirements

R7 establishes these invariants:

- structured style resolution does not mutate the DOM
- document style entries are in selector-DOM document order
- stylesheet rule order is deterministic across stylesheet insertion order and
  rule source order
- inline style rule order is a stable tie-break value, not a precedence
  sentinel
- inline style scope ids are stable within a style resolution pass
- legacy DOM style mutation, when used, is only a projection from structured
  resolved styles
- debug snapshots for document-level resolved styles are stable

## Representative Interactions Covered By Tests

The test surface covers:

- structured cascade output without DOM mutation
- selector matching through the structured selector engine rather than legacy
  selector projection
- parent-to-child inheritance through resolved styles
- inline style attributes entering structured cascade resolution
- deterministic `ResolvedDocumentStyle` debug snapshots
- legacy `attach_styles(...)` projecting structured winners back into
  `Node::style`

## Non-Goals

R7 does not:

- remove `html::Node::style`
- make computed style consume `ResolvedStyle`
- introduce user-agent stylesheet sources
- cache resolved styles across DOM or stylesheet mutations
- optimize storage beyond deterministic contract surfaces

Those remain follow-up work for the computed-values and runtime integration
milestones.

## Exit Condition For This Issue

This issue is complete when the cascade engine can produce structured resolved
styles for a DOM tree without relying on string-vector DOM mutation, and when
the remaining mutation bridge is only a compatibility projection from that
structured output.

That contract now exists and is covered by integration tests.
