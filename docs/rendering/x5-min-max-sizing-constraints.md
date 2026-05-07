# X5: Min/Max Sizing Constraints

Last updated: 2026-05-07  
Status: implemented min/max constraints for Milestone X issue 5

This document defines the supported min/max sizing behavior for Borrowser's
Milestone X sizing model. X5 builds on X1-X4 by making post-preferred
constraint application explicit, centralized, and regression-tested across
explicit sizes, intrinsic sizes, auto sizes, and atomic-inline available-space
clamping.

Related code:
- `crates/layout/src/sizing.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/box_tree/tests/projection.rs`
- `crates/css/src/computed/style.rs`

Related documents:
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x2-structured-size-resolution-model-inputs.md`
- `docs/rendering/x3-width-height-resolution-supported-subset.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/x6-percentage-sizing-targeted-subset.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`

## Supported Scope

The live CSS-supported subset is:

- `min-width: auto | <length>`
- `max-width: none | <length>`

X6 extends those live CSS values to also accept percentages:

- `min-width: auto | <length> | <percentage>`
- `max-width: none | <length> | <percentage>`

The structured sizing model also supports logical block-axis min/max inputs
through `AxisStyleSizeInput` and `StyleSizeInputs`. Those resolver-level block
constraints are tested, but CSS `min-height` and `max-height` properties remain
deferred until the CSS property model exposes them.

A percentage min/max value only resolves when the relevant containing-size basis
is definite.

## Constraint Ordering

Min/max constraints are post-preferred-size adjustments. They do not replace
the preferred size reason. `UsedAxisSize` must preserve:

- preferred value
- preferred `SizeResolutionReason`
- final value
- applied post-preferred adjustment

For one axis, the supported ordering is:

1. Resolve the preferred content-box size from explicit style, auto behavior,
   percentage basis, or intrinsic contribution.
2. Apply style max constraint.
3. Apply atomic-inline definite available-space clamp where applicable.
4. Apply style min constraint last.
5. Expand the final content size to border-box geometry by adding padding.

The min constraint wins if constraints cross. This includes both crossed
`min-width`/`max-width` and the case where `min-width` exceeds the available
inline clamp for an atomic inline box.

## Integration With Intrinsic Sizing

Intrinsic contributions produce preferred content sizes. Min/max constraints
apply after that preferred size is chosen:

- `width: auto` atomic inline boxes use X4 intrinsic/shrink-to-fit preferred
  width first
- `min-width` can grow the intrinsic/shrink-to-fit result
- `max-width` can shrink the intrinsic preferred result
- explicit `width` still overrides intrinsic preferred width before min/max
  constraints apply

This keeps intrinsic sizing and constraint application separate. Intrinsic
calculation must not bake in author min/max behavior.

## Determinism And Tests

X5 adds resolver-level tests for:

- style min and max metadata
- available-space clamps remaining distinct from style max constraints
- style max, available-space clamp, then style min ordering
- crossed min/max constraints resolving with min winning
- min-width after intrinsic shrink-to-fit
- max-width after intrinsic preferred width
- block-axis constraints through structured sizing inputs

X5 adds layout-level regression tests for:

- crossed `min-width`/`max-width` with padding
- `min-width` winning over atomic inline available-space clamp
- `min-width` after intrinsic auto inline-block width
- existing max-width, min-width, explicit width, and intrinsic tests continuing
  to pass through the integrated layout path

## Deferred Work

X5 intentionally does not implement:

- CSS `min-height` and `max-height` properties
- `min-content`, `max-content`, `fit-content`, or other intrinsic sizing
  keywords as authored CSS values
- aspect-ratio transfer after min/max-constrained inline sizes for replaced
  elements whose opposite axis is auto
- `box-sizing`
- borders
- margin auto distribution
- full CSS 2.1 block width equation support
- flex, grid, table, float, positioned, orthogonal writing-mode, or
  fragmentation-specific min/max behavior

Later milestones must extend the structured sizing inputs and resolver
constraints deliberately rather than adding local min/max corrections in layout
traversal.

## Exit Contract

X5 is complete while these remain true:

- min/max constraint application is centralized in the sizing model
- live CSS `min-width` and `max-width` affect supported normal-flow width
  resolution
- structured block-axis min/max constraints are honored by the resolver
- min wins over max and available-space clamps when constraints cross
- constraints apply after explicit, auto, percentage, or intrinsic preferred
  size selection and before padding expansion
- representative resolver and layout regression tests pass
