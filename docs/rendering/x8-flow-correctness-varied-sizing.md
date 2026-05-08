# X8: Flow Correctness Under Varied Sizing Conditions

Last updated: 2026-05-08  
Status: implemented flow-correctness improvements for Milestone X issue 8

This document defines the normal-flow correctness refinements added after the
X1-X7 sizing model became the source of truth for used width and height
resolution.

Related code:
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/sizing.rs`
- `crates/layout/src/box_tree/tests/projection.rs`

Related documents:
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x2-structured-size-resolution-model-inputs.md`
- `docs/rendering/x3-width-height-resolution-supported-subset.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/x5-min-max-sizing-constraints.md`
- `docs/rendering/x6-percentage-sizing-targeted-subset.md`
- `docs/rendering/x7-shrink-to-fit-containing-size-dependent-sizing.md`

## Purpose

X1-X7 strengthened how individual boxes resolve sizes. X8 tightens the normal
flow pass that consumes those sizes:

- child available inline size must come from the parent resolved content box
- child containing-size percentage basis must remain the parent resolved
  content box even when margins narrow available inline space
- child border-box x position must include the parent's content start plus the
  child's margin start
- root-element children must follow the same content-box flow rules as ordinary
  block containers
- auto block-size must use the content contribution produced by inline content,
  block children, margins, and padding in a deterministic order

The goal is not to introduce new layout modes. It is to prevent later features
from building on a flow pass that still leaks viewport, border-box, or
transitional root-element assumptions into descendant sizing.

## Flow Placement Contract

The normal-flow refinement pass now materializes a small internal flow
placement model:

```text
LayoutBox resolved border-box size
  -> FlowContentBox
  -> NormalFlowChildInlineInput
  -> child SizeResolutionInput
```

`FlowContentBox` represents the resolved content-box inline start, content-box
inline size, and content-box block start for the container currently being
laid out.

`NormalFlowChildInlineInput` derives the child's border-box x coordinate, the
child's containing inline-size basis, and the child's margin-reduced available
inline size from that content box plus the child's physical margins. This
centralizes the rule used by document, root-element, ordinary block, and
inline-block pre-layout paths.

## Root Element Behavior

Before X8, the transitional `<html>` handling laid out block children from the
root border box. That meant root padding affected the root's own used size but
did not consistently affect descendant position or available width.

X8 removes that inconsistency for non-inline root elements. The root element's
children now flow from the root resolved content box, so padding, constrained
width, margins, and child auto/percentage sizing interact the same way they do
inside ordinary block containers.

The special `display: inline` document-element bridge remains explicitly
transitional and scoped to the existing root-element compatibility behavior.

## Sizing Propagation Invariants

The normal-flow pass must preserve these invariants:

- A parent's resolved content-box inline size is the source for child inline
  sizing inputs; child margins are subtracted only from the child's
  `AvailableSpace`.
- A child percentage width resolves against the parent resolved content-box
  size, not the original viewport width.
- Child margins narrow `AvailableSpace` for auto/stretch and shrink-to-fit
  behavior, but they must not narrow the `ContainingSize` used as the basis for
  child percentage widths.
- A parent max/min/shrink-to-fit result must propagate to descendants before
  descendant widths are resolved.
- Root-element padding shifts child flow positions and reduces child available
  width just like ordinary block-container padding.
- Parent auto block-size includes inline content height, in-flow block child
  margin boxes, and parent padding exactly once.

## Determinism And Tests

X8 adds layout-level regression coverage for:

- constrained parent content width propagating into descendant percentage width
- child margins narrowing available space without narrowing the percentage
  containing-size basis
- root-element children flowing from the root content box instead of the root
  border box
- root-element auto height including child margins and root padding while child
  x/y/width use the resolved content-box basis

Existing X3-X7 resolver and layout tests continue to cover explicit, auto,
percentage, min/max, intrinsic, and shrink-to-fit sizing inputs.

## Deferred Work

X8 intentionally does not implement:

- margin collapsing
- margin auto distribution
- float avoidance
- positioned layout
- overflow-created formatting contexts
- full inline-fragment-to-layout-box geometry synchronization
- border and `box-sizing`
- flex, grid, table, orthogonal writing-mode, or fragmentation flow behavior

Later milestones must extend the internal flow placement model rather than
passing raw viewport or border-box widths into descendant sizing paths.

## Exit Contract

X8 is complete while these remain true:

- normal-flow child sizing uses resolved parent content-box dimensions
- root-element block children follow ordinary content-box flow placement
- constrained parent sizes propagate deterministically to descendant sizing
- auto block-size uses content and child flow contributions consistently
- representative flow-correctness regressions pass
