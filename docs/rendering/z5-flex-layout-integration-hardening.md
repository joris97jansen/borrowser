# Z5: Flex Layout Integration Hardening

Last updated: 2026-06-07
Status: implemented integration contract for Milestone Z issue 5

This document records the integration invariants for Borrowser's current flex
layout subset. Z5 does not add new flexbox features. It hardens the existing
Z2-Z4 support against the W box-tree model, the X sizing model, and the Y
advanced-flow groundwork.

Related code:

- `crates/layout/src/box_tree/model.rs`
- `crates/layout/src/box_tree/builder.rs`
- `crates/layout/src/document.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/sizing.rs`
- `crates/layout/src/flex.rs`
- `crates/layout/src/flow.rs`
- `crates/layout/src/phase.rs`
- `crates/layout/src/debug.rs`
- `crates/layout/src/box_tree/tests/projection.rs`

Related documents:

- `docs/rendering/z1-flex-layout-architecture-contract.md`
- `docs/rendering/z2-flex-box-tree-structure.md`
- `docs/rendering/z3-flex-main-axis-layout-core-subset.md`
- `docs/rendering/z4-flex-cross-axis-layout-core-subset.md`
- `docs/rendering/z6-flex-unsupported-feature-handling.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/x5-min-max-sizing-constraints.md`
- `docs/rendering/x8-flow-correctness-varied-sizing.md`
- `docs/rendering/x10-sizing-invariants-extension-hooks.md`
- `docs/rendering/y1-advanced-flow-layout-architecture-contract.md`
- `docs/rendering/y5-positioned-containing-block-logic.md`
- `docs/rendering/y6-out-of-flow-layout-participation-groundwork.md`
- `docs/rendering/y9-advanced-flow-invariants-extension-points.md`

## Purpose

Z2 introduced layout-owned flex container and flex item structure. Z3 added
the row-only main-axis algorithm. Z4 added the row-only cross-axis algorithm.
Z5 verifies that this subset is integrated into the existing layout engine
rather than operating as a special-case geometry pass.

The supported Z5 integration shape is:

```text
StyledNode + generated BoxTree metadata
  -> LayoutBox projection with flex, flow, containing-block, and positioning metadata
  -> normal parent flow routes flex containers as block-level participants
  -> flex item collection consumes generated direct in-flow children only
  -> flex basis and distributed sizes use sizing/intrinsic/constraint inputs
  -> out-of-flow children remain generated and tracked outside flex distribution
  -> deterministic layout/debug output exposes the retained decisions
```

Paint and browser/runtime orchestration remain downstream consumers. They must
not infer flex behavior from DOM shape, CSS declarations, or final rectangles.

## Box-Tree Integration

A flex container is a generated principal layout box with
`DisplayBoxBehavior::FlexContainer` and `FormattingContextKind::Flex`.

For the current subset, a flex container participates in its parent formatting
context as a block-level normal-flow box. Its own auto block size is produced
by flex cross-axis layout and is then consumed by the parent block-flow
placement machinery like any other in-flow block-level child.

Flex item identity remains generated layout identity:

- direct generated in-flow children of a flex container are flex items
- nested flex containers may also be flex items of an outer flex container
- nested flex containers establish their own independent flex formatting
  context for their direct generated in-flow children
- flex item participation is not derived from DOM child lists or raw CSS
  declarations during geometry

## Sizing Integration

Flex item main-axis basis resolution must consume the X sizing model.
`flex-basis: auto` in the current production subset routes through the
`NormalFlowSizingMode::FlexItemMainAxis` sizing path. Auto flex items use
intrinsic content contributions rather than block-level stretch behavior.

Distributed flex item sizes must continue to use the centralized sizing
constraint helpers:

- `resolve_flex_distributed_inline_size` for row main-axis distribution
- `resolve_flex_distributed_block_size` for row cross-axis stretch

Min/max constraints, padding expansion, used content size, and
`SizeResolutionReason::FlexDistributed` metadata belong to the sizing
subsystem. Flex layout may choose a target size, but it must not locally
reimplement min/max clamping or content-box versus border-box conversion.

Percentages must preserve the X distinction between `ContainingSize` and
`AvailableSpace`. Flex item margins may narrow available space for auto and
shrink-to-fit behavior, but they must not change the containing-size basis used
for percentage width resolution.

## Flow And Positioning Integration

Flex containers and flex items preserve the Y flow vocabulary.

In-flow flex items participate in flex distribution. Out-of-flow generated
children do not participate in flex main-axis sizing, cross-axis sizing,
placement, or parent auto-size contribution. They remain in the generated box
tree and projected layout tree with their source identity, style, flow
participation, and positioned containing-block metadata.

`LayoutPhaseOutput::out_of_flow_participants()` remains the handoff for future
positioned layout. A positioned child inside a flex container must be recorded
through generated `BoxId` and `PositionedContainingBlockId` metadata rather
than rediscovered from DOM ancestry.

Z5 does not complete absolute or fixed positioning geometry. It only preserves
the existing positioned-containing-block and out-of-flow groundwork while flex
layout runs.

## Debug And Regression Contract

Z5 integration behavior is covered by deterministic layout regression tests.
Those tests protect:

- flex containers participating correctly in parent block flow
- flex item percentages resolving against containing content size
- flex-distributed sizing preserving content-box constraints and padding
- nested flex containers being both flex items and flex formatting-context
  roots
- out-of-flow descendants inside flex containers keeping positioned metadata
  without flex item layout metadata

Debug surfaces remain internal regression contracts. They expose typed layout
decisions already retained on `LayoutBox`; they are not public APIs and must
not become a substitute for layout-owned data flow.

## Deliberate Exclusions

Z5 does not implement:

- authored flex CSS properties
- `flex-direction: column`
- reverse flex directions
- wrapping or multi-line flex
- `gap`
- `order`
- `justify-content`
- authored `align-items` or `align-self`
- inline flex containers
- full absolute or fixed positioning geometry
- relative visual offsets
- flex-specific paint behavior
- browser/runtime flex behavior

Unsupported flex features must remain explicit follow-up work. They must not
be approximated through block layout patches, paint-time inference, browser
runtime behavior, or raw DOM/CSS shortcuts. Z6 records the boundary-specific
unsupported feature policy for those exclusions.

## Exit Contract

Z5 is complete while these remain true:

- flex containers are integrated as generated block-level participants in
  parent normal flow for the current subset
- flex item collection consumes generated direct in-flow children
- nested flex preserves both outer flex item participation and inner flex
  formatting-context ownership
- flex sizing routes through intrinsic sizing, containing-size inputs,
  available-space inputs, and centralized constraints
- out-of-flow children remain generated and tracked without participating in
  flex distribution
- non-flex layout assumptions and regression coverage remain intact
- paint and browser/runtime orchestration do not infer flex layout behavior
