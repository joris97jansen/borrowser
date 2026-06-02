# Y8: Deterministic Advanced Flow Debug Regressions

Last updated: 2026-06-02
Status: implemented deterministic debug output and representative regression
coverage for Milestone Y advanced-flow behavior

Y8 adds an integrated debug surface for the advanced-flow behavior introduced
through Milestone Y. It does not add new layout behavior. Its purpose is to make
the existing layout-owned decisions inspectable and regression-testable without
requiring paint output, screenshots, or coordinate inference.

Related code:

- `crates/layout/src/flow.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/debug.rs`
- `crates/layout/src/phase.rs`
- `crates/layout/src/box_tree/tests/projection.rs`

Related documents:

- `docs/rendering/y1-advanced-flow-layout-architecture-contract.md`
- `docs/rendering/y2-structured-margin-handling.md`
- `docs/rendering/y3-margin-collapsing-supported-subset.md`
- `docs/rendering/y4-overflow-semantics-supported-subset.md`
- `docs/rendering/y5-positioned-containing-block-logic.md`
- `docs/rendering/y6-out-of-flow-layout-participation-groundwork.md`
- `docs/rendering/x9-deterministic-sizing-debug-regressions.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`

## Purpose

Earlier Y issues introduced layout-owned vocabulary and behavior for:

- structured margins
- adjacent in-flow block sibling margin collapse
- overflow policy and clip metadata
- positioned containing-block relationships
- out-of-flow participant tracking

Y8 makes those interactions visible through
`LayoutPhaseOutput::to_advanced_flow_debug_snapshot()`. The snapshot is a
layout-phase debug surface. It is not a public API and it is not a paint or
browser-runtime discovery mechanism.

## Retained Placement Decisions

`LayoutBox` now records:

```text
block_flow_placement: Option<BlockFlowBlockPlacement>
```

This field is set only when the normal-flow block placement pass already asks
`BlockFlowMarginCollapseCursor` for a placement decision. It stores the actual
semantic decision produced during layout, including any
`MarginCollapseDecision`.

The debug surface must not reconstruct margin collapse by comparing final
coordinates. Final geometry can prove where a box ended up, but it cannot
reliably explain which adjoining margins participated or which collapse rule
produced that position.

Out-of-flow boxes do not receive normal-flow block placement metadata in the
current supported subset. They remain in the layout tree and are tracked by the
existing out-of-flow participant registry.

## Snapshot Shape

`LayoutPhaseOutput::to_advanced_flow_debug_snapshot()` prints:

- snapshot version and semantic kind
- viewport width and document rectangle
- layout box count
- deterministic out-of-flow participant list, when present
- preorder layout boxes with:
  - generated box identity and source
  - containing-block and positioned containing-block metadata
  - positioning and flow participation
  - formatting context metadata
  - retained block-flow placement decision
  - overflow policy and clip metadata
  - logical margins
  - border-box and content-box geometry
  - child count

`BlockFlowBlockPlacement::as_debug_label()` prints the retained
`border-block-start` and either `margin-collapse=none` or the
`MarginCollapseDecision` debug label.

## Determinism Rules

The advanced-flow snapshot follows the existing Borrowser debug-surface rules:

- every snapshot starts with `version: 1`
- traversal is layout-tree preorder
- box and participant ordering are frame-local and deterministic
- labels come from typed layout vocabulary
- floating-point values use fixed decimal precision
- output is independent of hash-map iteration
- text labels are escaped through the existing node debug helpers

Changing this output should be reviewed as a debug contract change, not as
incidental formatting churn.

## Ownership

CSS owns parsing, cascade, and computed values for properties such as margins,
`overflow`, and `position`.

Layout owns:

- margin materialization and block-flow placement
- margin-collapse decisions for the supported subset
- overflow policy and clip metadata
- positioned containing-block resolution
- out-of-flow participant tracking
- advanced-flow debug serialization

Paint and browser/runtime orchestration consume layout output. They must not
rediscover margin collapse, overflow behavior, containing blocks, or
out-of-flow participation from DOM ancestry or raw CSS declarations.

## Regression Coverage

Y8 pins a representative exact snapshot covering the Y2-Y6 supported subset:

- an in-flow block with structured margins
- an adjacent sibling margin-collapse decision with positive and negative inputs
- an overflow clipping policy
- a positioned containing-block ancestor
- an out-of-flow absolute participant
- an overflow policy on an out-of-flow box that remains excluded from normal
  flow placement

This complements the narrower numeric tests for individual behavior. The Y8
snapshot asserts inspectability and deterministic formatting, not exhaustive CSS
compatibility.

## Deferred Work

Y8 deliberately does not implement:

- parent/child margin collapse
- empty-block self-collapse
- final absolute or fixed geometry
- relative visual offsets
- sticky positioning behavior
- inset resolution
- scrollbars or scroll state
- viewport overflow propagation
- stacking contexts or z-index
- paint rediscovery of layout relationships
- browser/runtime composed snapshot changes

Future flow work should extend the retained layout-owned decision metadata and
this debug surface deliberately when new behavior becomes real.
