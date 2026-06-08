# Z7: Deterministic Flex Debug Regressions

Last updated: 2026-06-08
Status: implemented debug and regression contract for Milestone Z issue 7

This document records Borrowser's deterministic flex layout debug surfaces and
regression coverage for the current Milestone Z production subset. Z7 does not
add flexbox behavior. It pins the layout decisions implemented by Z1-Z6 so flex
layout can be inspected and regressed without relying on visual output.

Related code:

- `crates/layout/src/flex.rs`
- `crates/layout/src/debug.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/box_tree/tests/projection.rs`

Related documents:

- `docs/rendering/z1-flex-layout-architecture-contract.md`
- `docs/rendering/z2-flex-box-tree-structure.md`
- `docs/rendering/z3-flex-main-axis-layout-core-subset.md`
- `docs/rendering/z4-flex-cross-axis-layout-core-subset.md`
- `docs/rendering/z5-flex-layout-integration-hardening.md`
- `docs/rendering/z6-flex-unsupported-feature-handling.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/w8-box-generation-formatting-debug-surfaces.md`
- `docs/rendering/x9-deterministic-sizing-debug-regressions.md`

## Supported Scope

Z7 covers debug output and regression fixtures for the existing supported flex
subset:

- block-level `display: flex` containers
- single-line row main-axis layout
- generated direct in-flow flex items
- internal default `flex-grow: 0`, `flex-shrink: 1`, and auto basis
- negative free-space shrink and positive free-space start packing
- block cross-axis sizing
- explicit container cross size
- default stretch for auto-height flex items
- preservation of explicit item heights
- exclusion of out-of-flow children from flex item layout

Z7 does not add authored flex CSS support or any new flex layout modes.

## Debug Surfaces

Flex algorithm tests use:

- `FlexMainAxisLayout::to_debug_snapshot()`
- `FlexCrossAxisLayout::to_debug_snapshot()`

Those snapshots record algorithm-level inputs and decisions in stable item
order. They are exact-testable and contain no DOM, paint, runtime, or backend
state.

Integrated layout tests use:

- `LayoutPhaseOutput::to_flex_debug_snapshot()`

The layout flex snapshot walks the final layout tree in deterministic generated
box order and emits only:

- flex containers
- their retained main-axis and cross-axis container metadata
- their direct retained flex items
- each flex item's retained main-axis and cross-axis metadata

The snapshot is intentionally smaller than the full layout phase snapshot. It
is a focused regression surface for flex decisions, not a broad rendering
golden.

## Retained Metadata Contract

Flex debug output must serialize retained layout metadata from `LayoutBox`:

- `flex_container_main_axis`
- `flex_item_main_axis`
- `flex_container_cross_axis`
- `flex_item_cross_axis`
- `FlexFormattingParticipation::FlexItem`

Debug code must not reconstruct flex decisions from final rectangles. Rectangles
may prove geometry in separate tests, but they are not the source of truth for
debugging distribution, basis, target size, offsets, alignment inputs, or
stretch eligibility.

## Determinism

The flex debug contract requires:

- fixed traversal order for a fixed generated layout tree
- stable item indexes from direct generated child order
- stable `BoxId` labels from deterministic box generation
- stable node/source labels using existing layout debug helpers
- stable CSS-px formatting with two decimal places
- exact snapshot assertions for representative main-axis, cross-axis, and
  integrated layout fixtures

Changing the debug format is allowed only as an intentional contract change
with updated tests and docs.

## Regression Coverage

Z7 regression coverage pins:

- main-axis negative shrink decisions
- item base size, target size, offset, margins, grow factor, and shrink factor
- cross-axis available size, line size, auto size, used size
- item hypothetical size, target size, offset, margins, alignment input, and
  stretch eligibility
- survival of flex metadata through the real layout pipeline

The integrated fixture uses supported `display: flex` behavior only. It does
not depend on unsupported authored flex declarations.

## Ownership Boundaries

CSS continues to own parsing, cascade, and computed style. Z7 does not add
property support for unsupported flex declarations.

Layout owns flex item collection, flex layout algorithms, retained flex
metadata, and flex debug output.

Paint consumes final layout geometry. It must not inspect DOM, CSS, or flex
debug metadata to infer flex behavior.

Browser/runtime orchestration owns invalidation and frame lifecycle only. It
must not emulate or repair flex behavior.

## Deliberate Exclusions

Z7 does not implement:

- `inline-flex`
- authored `flex-direction`
- authored `flex-wrap`
- authored `flex-flow`
- authored `justify-content`
- authored `align-items`, `align-self`, or `align-content`
- `gap`, `row-gap`, or `column-gap`
- `order`
- authored `flex`, `flex-grow`, `flex-shrink`, or `flex-basis`
- column, reverse, wrapping, multi-line, baseline, gap, or order-sorting layout
- paint-time or browser/runtime flex emulation
- visual or manual testing as primary proof

Unsupported features remain explicit follow-up work.

## Invariants

- Flex debug output is deterministic for fixed inputs.
- Debug snapshots describe retained flex decisions, not inferred geometry.
- Flex item order follows generated layout child order.
- Out-of-flow descendants do not become flex items.
- Main-axis and cross-axis decisions remain separately inspectable.
- Debug snapshots remain internal regression contracts, not public APIs.
