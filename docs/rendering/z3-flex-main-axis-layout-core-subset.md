# Z3: Flex Main-Axis Layout Core Subset

Last updated: 2026-06-04
Status: implemented main-axis contract for Milestone Z issue 3

This document defines Borrowser's first flex layout algorithm behavior. Z3
implements a layout-owned, deterministic main-axis pass for a deliberately
small flex subset. It does not implement the full Flexbox specification.

Related code:

- `crates/layout/src/flex.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/sizing.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/debug.rs`
- `crates/layout/src/box_tree/model.rs`
- `crates/layout/src/box_tree/formatting.rs`

Related documents:

- `docs/rendering/z1-flex-layout-architecture-contract.md`
- `docs/rendering/z2-flex-box-tree-structure.md`
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/x5-min-max-sizing-constraints.md`
- `docs/rendering/x7-shrink-to-fit-containing-size-dependent-sizing.md`

## Supported Scope

Z3 supports:

- `display: flex` as the already-supported block-level flex container
- single-line `flex-direction: row`
- generated direct in-flow flex items only
- deterministic generated child order
- `flex-grow: 0`
- `flex-shrink: 1`
- `flex-basis: auto`
- start packing
- positive free-space detection without growth in production defaults
- negative free-space shrink distribution using the default shrink factor
- final flex item main-axis geometry written to `LayoutBox`
- deterministic debug metadata for container and item main-axis decisions

The current CSS property model does not represent authored `flex-grow`,
`flex-shrink`, `flex-basis`, `flex-direction`, or `justify-content`. Z3 does
not fake support for those properties. The layout algorithm uses typed
internal inputs so later CSS support can feed authored values through the same
layout-owned path.

## Main-Axis Ownership

Flex main-axis layout is owned by layout. It consumes generated layout boxes
and flex participation metadata produced by box-tree generation. It must not
walk DOM children, inspect raw CSS declarations, or infer flex behavior in
paint or browser/runtime orchestration.

Eligible flex items are direct generated children whose
`FlexFormattingParticipation` is `FlexItem` and whose flow participation is
in-flow. Out-of-flow children remain generated boxes and remain visible in
debug surfaces, but they do not contribute to flex item collection, basis
resolution, free-space calculation, or item placement.

## Basis And Distribution

For Z3, `flex-basis: auto` resolves through the existing sizing and intrinsic
sizing machinery. Flex item main-axis basis resolution uses a dedicated sizing
mode so auto flex items are content-based instead of block-stretched.

Min/max constraints remain centralized in the sizing subsystem. Flex layout may
choose a distributed target size, but constraint application is delegated back
to the existing sizing resolver before final geometry is assigned.

The supported free-space behavior is:

- no items: no-op with deterministic metadata
- exact fit: items keep their basis sizes
- positive free space with zero grow factors: start packing; free space remains
  at the end of the line
- positive free space with internal non-zero grow inputs: proportional growth
- negative free space with shrink inputs: proportional shrink using scaled
  shrink factors
- zero shrink denominator: deterministic no-shrink fallback

Production style adaptation currently supplies grow `0`, shrink `1`, and auto
basis for every flex item.

## Geometry Contract

For the row-only horizontal subset, the flex main axis maps to the physical
inline axis. The flex container participates in its parent normal flow as a
block-level box. Inside the container, direct flex items are not block-stacked
or collected by the container's inline formatting context.

The flex pass assigns each item a final border-box x coordinate and width. A
minimal cross-axis bookkeeping step top-aligns items at the container content
top and computes the container auto block size from the maximum laid-out item
height. This is required to produce coherent geometry, but it is not
cross-axis alignment support.

Paint consumes the final `LayoutBox` rectangles exactly as it does for other
layout modes. Paint must not inspect flex metadata to infer behavior.

## Debug Contract

Flex debug metadata is an internal regression contract. It records typed
layout decisions rather than reconstructing them from final rectangles:

- row main axis
- available main size
- total outer base size
- free space
- distribution decision
- item base size
- item target size
- item main offset
- grow and shrink factors used by the algorithm

Floating-point values use stable CSS-px formatting consistent with existing
layout debug surfaces.

## Deliberate Exclusions

Z3 does not implement:

- authored flex-related CSS properties
- `flex-direction: column`
- reverse directions
- wrapping or multi-line flex
- `justify-content`
- cross-axis alignment
- `align-items` or `align-self`
- baseline alignment
- `gap`
- `order`
- inline flex containers
- browser-perfect min-content/max-content flexbox nuance
- flex-specific paint behavior
- runtime-specific flex behavior

Unsupported features must remain explicit follow-up work. They must not be
approximated through block layout patches or paint-time inference.

## Exit Contract

Z3 is complete while these remain true:

- flex containers use a layout-owned row main-axis algorithm
- flex item collection consumes generated box-tree participation metadata
- out-of-flow children do not participate in distribution
- flex items are laid out horizontally instead of block-stacked
- basis and constraint handling route through the sizing/intrinsic machinery
- debug metadata exposes deterministic main-axis decisions
- representative algorithm and layout regression tests pass
