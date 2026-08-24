# R7: Structured Resolved-Style Output

Last updated: 2026-08-15
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
- `docs/css/r8-cascade-style-resolution-debug-output.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`

## Implemented Result

R7 introduces the document-level structured cascade output:

- `ResolvedElementStyle`
- `ResolvedDocumentStyle`
- `resolve_document_styles(root, matching_environment, ...)`
- `try_resolve_document_styles_with_limits(root, matching_environment, ...)`

`resolve_document_styles(...)` is now the core cascade integration path for a
`Node::Document`, an explicit CSS-owned `SelectorMatchingEnvironment`, and an
ordered stylesheet list. It returns
`Result<ResolvedDocumentStyle, StyleResolutionError>`, preserves hardening
failures explicitly, and does not mutate `html::Node`.

AF4b removes generic root inference. These authoritative entry points use
fallible document projection construction. Invalid root kind, nested document,
ambiguous document-element identity, selector-ID exhaustion, and reported
projection capacity/reservation failures propagate through
`StyleResolutionError::SelectorDomBuild`. The styled-element limit retains its
separate `LimitExceeded(StyledElementsPerDocument)` meaning.

All cascade entry points that initiate selector matching require the matching
environment explicitly. There is no environment-less or default semantic
state, and missing document metadata must not imply `NoQuirks`.

The old `attach_styles(...)` function still exists, but it is now explicitly a
legacy projection and compatibility downgrade path:

1. resolve structured document styles
2. project authored winner values into `Node::Element::style`
3. leave inherited and initial/default entries out of the string vector so the
   bridge-phase computed-style path keeps its existing inheritance/default
   behavior
4. clear legacy projected style vectors instead of fabricating a resolved-style
   result when document style resolution fails

The bridge keeps its unit return type. For historical compatibility it chooses
the explicit document constructor for document input and explicit element-
subtree construction for element input. Leaf roots and any build, resolution,
or projection failure clear stale vectors. This is a deliberate compatibility
exception; authoritative R7 APIs always expose typed failures.

Cascade winner selection, inheritance, and defaulting no longer depend on that
mutation path.

## Structured Output Shape

`ResolvedDocumentStyle` contains the immutable
`SelectorMatchingEnvironment` under which selector resolution occurred and one
ordered `ResolvedElementStyle` per selector-DOM element.

Each element entry records:

- stable selector-DOM element id for the style pass
- canonical parser-created element namespace
- canonical element name
- total per-element `ResolvedStyle`

The namespace and local name form the semantic element identity carried by
the retained CSS artifact. A same-spelled element in another namespace is not
eligible for incremental resolved-style reuse.

The per-element `ResolvedStyle` remains the R1-R6 contract object:

- every supported property appears exactly once
- authored winners carry source, priority, and specified value
- inherited entries are explicit
- initial/default entries are explicit

AF7 refines this as a total specified-value/defaulting source-resolution
artifact. Inherited entries are symbolic and do not retain parent resolved or
specified values.

## Resolution Pipeline

For each element in selector-DOM document order, R7 performs:

1. match each model stylesheet style rule against the element through
   `SelectorMatchingContext` carrying the explicit matching environment
2. materialize matched stylesheet declarations as `CascadeRuleInput`
3. independently consume ordered neutral selector-DOM attribute facts through
   the CSS-wide effective-attribute helper and materialize the element's inline
   `style` attribute as an inline
   `CascadeRuleInput`
4. resolve authored winners into `CascadeWinnerSet`
5. resolve inheritance/default fill into `ResolvedStyle`
6. store the result in `ResolvedDocumentStyle`

AF7 source resolution derives typed parent presence directly from selector-DOM
topology and does not retain a parent `ResolvedStyle` lookup map. Document
order still places parents before children, which is required when the later
computed pass reads the immediate parent `ComputedStyle`.

The retained environment is part of CSS-owned semantic reuse validity. An
incremental or prefix resolved-style reuse attempt must reject an environment
mismatch before reusing any prior entries. Browser retained-style keys remain
lifecycle/cache eligibility checks and do not substitute for this CSS semantic
compatibility check.

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
- integrated `resolve_document_styles_debug_snapshot(...)` traces for
  candidate ordering, authored overrides, inheritance, and defaulting
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
