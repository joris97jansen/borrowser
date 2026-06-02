# Y6: Out-of-Flow Layout Participation Groundwork

Last updated: 2026-06-01  
Status: implemented out-of-flow participant tracking for Milestone Y issue 6

Y6 establishes the layout-owned representation for generated boxes that are
removed from normal flow but still need to participate in a later positioned
layout pass. It builds on Y1's advanced flow vocabulary and Y5's positioned
containing-block metadata. It does not implement final positioned geometry.

Related code:

- `crates/layout/src/flow.rs`
- `crates/layout/src/phase.rs`
- `crates/layout/src/document.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/inline/tokens.rs`
- `crates/layout/src/inline/intrinsic.rs`
- `crates/layout/src/debug.rs`
- `crates/layout/src/box_tree/builder.rs`
- `crates/layout/src/box_tree/tests/projection.rs`

Related documents:

- `docs/rendering/y1-advanced-flow-layout-architecture-contract.md`
- `docs/rendering/y3-margin-collapsing-supported-subset.md`
- `docs/rendering/y5-positioned-containing-block-logic.md`

## Ownership

CSS owns parsing, cascade, and computed-value normalization for `position`.
Layout consumes the computed `css::Position` and maps it to
`PositioningScheme`, `FlowParticipation`, and `OutOfFlowKind`.

Layout owns the frame-local out-of-flow participant registry. The registry is
collected from projected `LayoutBox` metadata after normal-flow geometry has
run. It must not inspect DOM or CSS declarations directly, and it must not
recompute positioned containing blocks.

Paint and browser/runtime orchestration consume layout output. They must not
rediscover out-of-flow behavior from DOM ancestry or raw CSS declarations.

## Supported Scope

For Y6:

- `position: absolute` is represented as
  `FlowParticipation::OutOfFlow(OutOfFlowKind::AbsolutelyPositioned)`
- `position: fixed` is represented as
  `FlowParticipation::OutOfFlow(OutOfFlowKind::FixedPositioned)`
- `position: static`, `position: relative`, and `position: sticky` remain
  `FlowParticipation::InFlow`

Out-of-flow boxes remain in the generated box tree and the projected layout
tree. They are not dropped, suppressed, or converted into display-none
behavior.

Normal-flow layout ignores out-of-flow boxes for parent auto-size
contribution, sibling block placement, sibling margin collapse, and inline
token contribution in the supported subset.

## Participant Registry

`OutOfFlowLayoutParticipant` records:

- the generated `BoxId`
- the `OutOfFlowKind`
- the resolved `PositionedContainingBlockId`

`PositionedContainingBlockId` is not optional in the Y6 registry. Y5 must
resolve it before Y6 records the participant. If an out-of-flow box lacks that
metadata, the layout phase has violated the Y5/Y6 handoff contract.

`LayoutPhaseOutput::out_of_flow_participants()` exposes participants in
deterministic layout-tree preorder. This is a future positioning handoff
surface, not an executable positioning queue.

## Debug And Determinism

Layout debug snapshots retain out-of-flow boxes in the normal box listing with
their `position`, `flow`, and `positioned-cb` metadata. When out-of-flow
participants exist, snapshots also include a participant list:

```text
out-of-flow-participants: 2
out-of-flow[0]: box-id=b3 kind=absolute positioned-cb=b2
out-of-flow[1]: box-id=b5 kind=fixed positioned-cb=b0
```

The participant list is deterministic for a fixed generated box tree because
box IDs and layout-tree traversal order are deterministic.

## Deferred Work

Y6 deliberately does not implement:

- final absolute or fixed geometry
- `top`, `right`, `bottom`, or `left` offset resolution
- static-position capture
- relative visual offsets
- sticky positioning behavior
- fixed viewport anchoring
- z-index
- stacking contexts
- positioned paint ordering
- transformed containing blocks
- scroll-container interaction
- viewport/root/body propagation behavior

Future positioning work should consume `OutOfFlowLayoutParticipant` and the
existing Y5 positioned containing-block metadata rather than introducing a
parallel DOM- or CSS-derived model.
