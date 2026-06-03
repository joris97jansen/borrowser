# Y9: Advanced Flow Invariants And Extension Points

Last updated: 2026-06-02
Status: implemented Milestone Y close-out contract for advanced flow invariants
and extension points

This document closes Milestone Y by collecting the implemented advanced-flow
subset, the invariants that must remain true, and the extension points that
future layout milestones must use. It does not introduce new layout behavior.

There is no Y7 rendering contract document in the current Milestone Y chain.
The implemented Y close-out chain is Y1, Y2, Y3, Y4, Y5, Y6, Y8, and this Y9
contract.

Related code:

- `crates/layout/src/flow.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/phase.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/sizing.rs`
- `crates/layout/src/debug.rs`
- `crates/layout/src/box_tree/model.rs`
- `crates/layout/src/box_tree/builder.rs`
- `crates/layout/src/box_tree/formatting.rs`
- `crates/gfx/src/paint/mod.rs`

Related documents:

- `docs/rendering/y1-advanced-flow-layout-architecture-contract.md`
- `docs/rendering/y2-structured-margin-handling.md`
- `docs/rendering/y3-margin-collapsing-supported-subset.md`
- `docs/rendering/y4-overflow-semantics-supported-subset.md`
- `docs/rendering/y5-positioned-containing-block-logic.md`
- `docs/rendering/y6-out-of-flow-layout-participation-groundwork.md`
- `docs/rendering/y8-deterministic-advanced-flow-debug-regressions.md`
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x8-flow-correctness-varied-sizing.md`
- `docs/rendering/x10-sizing-invariants-extension-hooks.md`
- `docs/rendering/z1-flex-layout-architecture-contract.md`

## Purpose

Milestone Y extends Borrowser's layout engine from basic normal-flow sizing and
box-tree projection into the first advanced-flow contract:

- structured margin materialization and placement inputs
- adjacent in-flow block sibling margin collapse
- overflow policy and clip metadata
- positioned containing-block relationships
- out-of-flow participant tracking for absolute and fixed boxes
- deterministic advanced-flow debug output

Y9 records the stable contract that future positioning, stacking, paint,
scrolling, flex, grid, table, and formatting-context work must extend. It is a
documentation close-out issue. It must not be used to imply that deferred CSS or
browser behavior is implemented.

## Ownership

CSS owns parsing, cascade, specified values, computed values, inheritance,
initial/default selection, and computed-value normalization. CSS exposes
canonical values such as margins, `overflow`, and `position`; it does not decide
margin collapse, containing-block lookup, out-of-flow participation, clipping
geometry, or paint behavior.

Layout owns generated-box flow semantics after computed style is available:

- margin materialization through `FlowMargins`
- normal-flow block placement and supported margin-collapse decisions
- overflow interpretation through `OverflowPolicy`
- layout-owned clip metadata through `OverflowClip`
- normal and positioned containing-block metadata
- flow participation classification
- out-of-flow participant tracking
- deterministic advanced-flow debug serialization

Paint consumes layout-provided geometry and clipping. Paint must not inspect raw
CSS declarations, walk DOM ancestry, or reconstruct layout decisions to decide
overflow clipping, containing blocks, margin collapse, out-of-flow
participation, stacking, or positioning.

Browser/runtime orchestration owns invalidation, retained phase artifacts, and
frame lifecycle. It must not infer layout semantics from DOM or CSS when layout
has already produced explicit flow metadata.

## Implemented Supported Subset

The supported margin subset is finite px margins from computed box metrics in
the current horizontal writing-mode subset. Margins are signed logical
flow-placement inputs, not used content sizes. Inline-start and inline-end
margins adjust available inline space without changing the containing-size basis
used by percentage sizing.

Adjacent in-flow block siblings in the same normal-flow block container support
margin collapse. The first in-flow block child keeps its block-start margin, and
the last in-flow block child keeps its block-end margin. Collapse decisions use
the positive/negative `CollapsedMargin` rule and are retained as semantic
layout decisions, not inferred from final coordinates.

The supported overflow CSS surface is the single-keyword `overflow` shorthand:
`visible`, `hidden`, `clip`, `scroll`, and `auto`. Layout materializes a uniform
inline/block `OverflowPolicy`. Clipping policies produce `OverflowClip` metadata
from final layout geometry for supported boxes. Overflow does not inflate used
width or height in the current subset.

Positioned containing-block logic supports `static`, `relative`, `absolute`,
`fixed`, and `sticky` classification. `relative`, `absolute`, `fixed`, and
`sticky` establish positioned containing blocks for descendants. Absolute boxes
resolve to the nearest positioned generated ancestor with fallback to the
initial containing block. Fixed boxes resolve to the initial containing block in
the current subset.

Out-of-flow participation tracks `position: absolute` and `position: fixed`
boxes. Those boxes remain in the generated box tree and projected layout tree,
but they do not contribute to parent normal-flow auto size, block sibling
placement, sibling margin collapse, or inline token contribution in the
supported subset. `LayoutPhaseOutput::out_of_flow_participants()` records their
generated `BoxId`, `OutOfFlowKind`, and resolved `PositionedContainingBlockId`
in deterministic layout-tree preorder.

`LayoutPhaseOutput::to_advanced_flow_debug_snapshot()` exposes the combined
advanced-flow state for regression testing. The snapshot is an internal
debug-contract surface, not a public API and not a paint or runtime discovery
mechanism.

## Margin Invariants

- Margins are layout-owned flow-placement inputs.
- `UsedContentSize` must not absorb margins.
- `FlowMargins` is the shared vocabulary for block placement, inline atomic
  margin handling, intrinsic outer contributions, and debug output.
- Anonymous layout boxes expose zero flow margins.
- Inline negative margin handling must not make the inline engine move
  backwards through its token stream.
- Supported sibling margin collapse is owned by
  `BlockFlowMarginCollapseCursor`.
- Margin collapse decisions must be recorded as semantic decisions when exposed
  through debug output.
- Out-of-flow boxes do not participate in parent sibling margin collapse in the
  current subset.
- Future margin behavior must extend `FlowMargins`, collapse decisions, and
  debug surfaces deliberately rather than reintroducing direct margin arithmetic
  at call sites.

## Overflow Invariants

- Layout consumes canonical computed overflow values and maps them to
  `OverflowPolicy`.
- `OverflowPolicy` is layout vocabulary, not a raw CSS declaration model.
- Overflow policy must not silently resize content to avoid clipping.
- `OverflowClip` is derived from layout-owned policy and final layout geometry.
- Paint consumes `OverflowClip`; paint does not resolve CSS overflow.
- Anonymous generated boxes expose visible overflow in the current subset.
- Scroll-container semantics are represented as policy, but scroll state,
  scrollbars, and scrollable overflow dimensions are deferred.
- Full overflow-created formatting-context behavior must extend generated-box
  and layout metadata rather than becoming a paint-time rule.

## Containing-Block Invariants

- `ContainingBlockId` remains the normal-flow containing block used for current
  sizing and in-flow placement.
- `PositionedContainingBlockId` names the generated-box relationship used by
  positioned descendants.
- Positioned containing-block lookup is layout data. It must not be recomputed
  by paint, hit testing, or browser/runtime orchestration.
- Lookup walks generated ancestors and uses explicit fallback to the initial
  containing block.
- The document/root generated box represents the initial containing block for
  the current positioned lookup subset.
- Anonymous boxes are static for the current positioning subset.
- Transforms, viewport/root/body propagation, and scroll-container interactions
  with containing-block resolution are deferred.

## Out-Of-Flow Invariants

- Out-of-flow boxes retain generated identity, source identity, style, layout
  boxes, and debug visibility.
- Out-of-flow participation is represented by `FlowParticipation` and
  `OutOfFlowKind`.
- Absolute and fixed boxes are excluded from parent normal-flow size
  contribution, sibling block placement, sibling margin collapse, and inline
  token contribution in the current subset.
- Out-of-flow participants must have a resolved `PositionedContainingBlockId`
  before they are recorded by `LayoutPhaseOutput`.
- The participant list is deterministic layout-tree preorder.
- The participant list is a future positioning handoff surface, not a complete
  positioned layout pass.
- Future positioned layout must consume generated `BoxId`, positioned
  containing-block metadata, style insets once supported, static-position data
  once added, and the shared sizing model.

## Debug And Determinism Invariants

- Advanced-flow debug output is deterministic for a fixed DOM, computed style
  tree, viewport, text measurer, and replaced-element metadata.
- Traversal order is layout-tree preorder.
- Box IDs and participant ordering are frame-local and deterministic.
- Debug labels must come from typed layout vocabulary.
- Floating-point output uses fixed precision.
- Margin collapse debug output must describe the retained semantic decision,
  not reconstruct behavior from final coordinates.
- Debug snapshots are internal regression contracts. Changing them should be
  reviewed as a contract change, not incidental formatting churn.

## Extension Points

Positioning work should extend the existing `PositioningScheme`,
`PositionedContainingBlockId`, `FlowParticipation`, and
`OutOfFlowLayoutParticipant` handoff. Final absolute and fixed geometry,
relative offsets, sticky positioning, static-position capture, and inset
resolution must use structured layout inputs and the shared sizing model.

Paint and stacking work should consume layout geometry, overflow clips,
positioned metadata, and future stacking-context metadata. It must not inspect
raw CSS or DOM ancestry to decide positioning, clipping, z-index, or paint
order.

Scrolling work should extend `OverflowPolicy`, `OverflowClip`, and retained
runtime scroll state deliberately. Scrollbar reservation, scroll offset storage,
scrollable overflow dimensions, viewport overflow propagation, and hit-test
clipping are future features.

Formatting-context work should extend generated-box and layout metadata for
overflow-created independent formatting contexts, floats, clearance,
containment, transforms, and fragmentation. These features must define their
own boundaries before they interact with margin collapse.

Flex, grid, and table layout expansion should consume computed style,
generated-box identity, containing-size inputs, available-space inputs, and
shared sizing primitives. These layout systems must not bypass the box tree,
invent parallel containing-block identity, or embed ad hoc CSS interpretation
inside layout algorithms.

## Deferred Features

Milestone Y deliberately does not implement:

- parent block-start with first in-flow child margin collapse
- parent block-end with last in-flow child margin collapse
- empty-block self-collapse
- margin auto distribution
- percentage margins
- full writing-mode-specific margin and overflow mapping
- floats and clearance
- fragmentation
- final absolute or fixed positioning geometry
- relative visual offsets
- sticky positioning
- inset resolution
- static-position capture
- transformed containing blocks
- scrollbars
- scroll state
- scrollable overflow dimensions
- viewport overflow propagation
- root/body overflow propagation
- hit-test clipping from overflow
- stacking contexts
- z-index
- positioned paint ordering
- flex layout
- grid layout
- table layout

Future issues may implement these features only by extending the documented
layout-owned contracts and debug surfaces. They must not treat this Y9 close-out
as evidence that the behavior already exists.

## Close-Out Criteria

Milestone Y is complete while these conditions hold:

- implemented margin, overflow, containing-block, and out-of-flow behavior is
  explicitly documented as a supported subset
- limitations and deferred features are explicit
- subsystem ownership boundaries are clear
- flow metadata remains generated-box and layout-owned
- paint and browser/runtime orchestration consume layout outputs instead of
  rediscovering flow state
- debug surfaces expose semantic layout decisions deterministically
- future contributors have named extension points for positioning, scrolling,
  stacking, formatting-context, flex, grid, and table expansion
