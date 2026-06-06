# Z4: Flex Cross-Axis Layout Core Subset

Last updated: 2026-06-06
Status: implemented cross-axis contract for Milestone Z issue 4

This document defines Borrowser's first flex cross-axis layout behavior. Z4
extends the Z3 row-only, single-line flex subset with layout-owned physical
block-axis sizing and placement. It does not implement the full Flexbox
specification.

Related code:

- `crates/layout/src/flex.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/sizing.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/debug.rs`

Related documents:

- `docs/rendering/z1-flex-layout-architecture-contract.md`
- `docs/rendering/z2-flex-box-tree-structure.md`
- `docs/rendering/z3-flex-main-axis-layout-core-subset.md`
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x3-width-height-resolution-supported-subset.md`
- `docs/rendering/x5-min-max-sizing-constraints.md`
- `docs/rendering/x10-sizing-invariants-extension-hooks.md`

## Supported Scope

Z4 supports:

- single-line `display: flex` containers from the Z2 generated box-tree model
- row-direction behavior only
- physical block axis as the cross axis
- generated direct in-flow flex items only
- layout-owned hypothetical item cross sizes
- auto flex container cross size from the maximum item outer cross size
- explicit flex container height as the available cross size
- deterministic item cross-axis offsets
- default stretch behavior for auto-height flex items
- preservation of explicit item heights
- typed debug metadata for container and item cross-axis decisions

The current CSS property model does not represent authored `align-items` or
`align-self`. Z4 does not add parser, cascade, or computed-style support for
those properties. The layout algorithm uses typed internal alignment inputs so
future CSS support can feed authored values through the same layout-owned path.

## Cross-Axis Ownership

Flex cross-axis layout is owned by layout. It consumes generated layout boxes,
flex participation metadata, sizing inputs, and item geometry produced by the
main-axis phase. It must not walk DOM children, inspect raw CSS declarations,
or infer flex behavior in paint or browser/runtime orchestration.

Paint consumes final `LayoutBox` rectangles. Paint must not inspect flex
metadata to decide cross-axis sizing, stretching, offsets, or alignment.

Browser/runtime orchestration owns invalidation and frame lifecycle only. It
must not infer flex cross-axis behavior from DOM or CSS.

## Sizing And Placement

For the supported row subset, the flex cross axis maps to the physical block
axis.

Each eligible flex item first receives a hypothetical cross size by running the
normal layout sizing path with its resolved main-axis width. The flex line's
auto cross size is the maximum outer hypothetical cross size, including
block-start and block-end margins.

For an auto-height flex container, the container auto content block size comes
from that line auto cross size. For an explicit-height flex container, the
resolved content block size becomes the available cross size for item alignment
and stretch.

Default stretch applies only to flex items whose cross size is auto. Explicit
item heights remain explicit. Stretch target sizes route through the sizing
subsystem with a flex-distributed reason before final geometry is assigned.

Cross-axis offsets are produced by typed flex layout data. Final `rect.y` and
`rect.height` values are applied from that data during layout refinement, not
from paint or late rectangle patching.

## Debug Contract

Flex cross-axis debug metadata is an internal regression contract. It records
typed layout decisions rather than reconstructing them from final rectangles:

- block cross axis
- optional available cross size
- line cross size
- auto cross size
- used cross size
- item hypothetical cross size
- item target cross size
- item cross offset
- item block-axis margins
- alignment input
- stretch eligibility

Floating-point values use stable CSS-px formatting consistent with existing
layout debug surfaces.

## Deliberate Exclusions

Z4 does not implement:

- authored `align-items` or `align-self`
- `flex-direction: column`
- reverse directions
- wrapping or multi-line flex
- `gap`
- `order`
- `align-content`
- baseline alignment
- inline flex containers
- browser-perfect min-content/max-content flexbox nuance
- flex-specific paint behavior
- runtime-specific flex behavior

Unsupported features must remain explicit follow-up work. They must not be
approximated through block layout patches, paint-time inference, or CSS parser
shortcuts.

## Exit Contract

Z4 is complete while these remain true:

- cross-axis layout consumes generated flex item metadata
- out-of-flow children do not participate in cross-axis sizing or placement
- auto container cross size is derived from explicit flex cross-axis data
- explicit container height becomes the available cross size for the supported
  row subset
- auto-height flex items can stretch through typed layout data
- explicit item heights are preserved
- stretch sizing routes through the sizing subsystem
- debug metadata exposes deterministic cross-axis decisions
- paint and browser/runtime orchestration do not infer flex behavior
