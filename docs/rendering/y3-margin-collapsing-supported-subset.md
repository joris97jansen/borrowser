# Y3: Margin Collapsing Supported Subset

Last updated: 2026-05-09  
Status: implemented adjacent block sibling margin collapse for Milestone Y issue 3

This document records the first implemented margin-collapse subset. It builds
on Y2's `FlowMargins` model by making collapse a named block-flow decision
instead of an accidental result of cursor arithmetic.

Related code:

- `crates/layout/src/flow.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/box_tree/tests/projection.rs`

Related documents:

- `docs/rendering/y1-advanced-flow-layout-architecture-contract.md`
- `docs/rendering/y2-structured-margin-handling.md`

## Supported Scope

Y3 supports collapsing between adjacent in-flow block siblings inside the same
normal-flow block container.

For two adjacent in-flow block siblings:

```text
previous border block-end
  + collapse(previous block-end margin, next block-start margin)
  = next border block-start
```

The collapsed margin uses the CSS positive/negative rule already modeled by
`CollapsedMargin`:

- if positive margins are present, use the largest positive margin
- if negative margins are present, add the most negative margin
- if only negative margins are present, the most negative margin wins
- zero participates as the missing positive or negative group

The first in-flow block child keeps its own block-start margin. The last
in-flow block child keeps its own block-end margin. Parent/child collapse is
not part of Y3.

## Layout Contract

`BlockFlowMarginCollapseCursor` owns block-axis sibling collapse for the current
supported subset. The normal-flow layout pass asks the cursor for each
block-level child's border block-start before laying that child out, then
records the child's border block-end and block-end margin after layout.

This keeps the sequence deterministic:

```text
parent content block-start
  -> next_in_flow_block(child margins)
  -> child layout
  -> finish_in_flow_block(child border size, child margins)
  -> parent auto content block-size
```

The cursor never collapses inline participants, atomic inline boxes, or
out-of-flow descendants. The current tree does not yet contain positioned
out-of-flow participation, floats, clearance, or fragmentation, so those remain
explicitly outside this implementation.

## Boundaries And Non-Goals

Y3 deliberately does not implement:

- parent block-start with first in-flow child collapse
- parent block-end with last in-flow child collapse
- empty-block self-collapse
- border or padding boundary checks for parent/child collapse
- clearance
- floats
- fragmentation
- positioned out-of-flow margin behavior
- margin-collapse debug records attached to layout boxes

Those cases require additional boundary and content preconditions before the
collapse category names from Y1 can become concrete layout decisions.

## Determinism

Sibling collapse decisions are pure functions of the previous sibling's
block-end margin and the next sibling's block-start margin. A
`MarginCollapseDecision` records the case, both input margins, and the collapsed
result so future debug surfaces can expose the same semantic data without
recomputing layout.

Negative collapsed margins may place a sibling border box above the previous
sibling's border block-end. Parent auto content block-size remains
non-negative by clamping the final content contribution at zero, matching the
existing sizing contract for block-axis used sizes.
