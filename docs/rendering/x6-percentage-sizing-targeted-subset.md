# X6: Percentage Sizing For The Targeted Subset

Last updated: 2026-05-07  
Status: implemented percentage sizing for Milestone X issue 6

This document defines Borrowser's supported percentage sizing behavior for
Milestone X. X6 builds on X1-X5 by letting authored sizing percentages flow
from CSS parsing through computed style, layout sizing inputs, and the
normal-flow resolver without ad hoc layout fallbacks.

Related code:
- `crates/css/src/values.rs`
- `crates/css/src/specified/length.rs`
- `crates/css/src/computed/value.rs`
- `crates/css/src/computed/style.rs`
- `crates/layout/src/sizing.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/box_tree/tests/projection.rs`

Related documents:
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x2-structured-size-resolution-model-inputs.md`
- `docs/rendering/x3-width-height-resolution-supported-subset.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/x5-min-max-sizing-constraints.md`
- `docs/rendering/x7-shrink-to-fit-containing-size-dependent-sizing.md`
- `docs/rendering/x8-flow-correctness-varied-sizing.md`
- `docs/rendering/x9-deterministic-sizing-debug-regressions.md`
- `docs/css/s3-parsed-specified-value-representations.md`
- `docs/css/s4-computed-value-representations-normalization.md`

## Supported Scope

The live CSS-supported subset is:

- `width: auto | <length> | <percentage>`
- `height: auto | <length> | <percentage>`
- `min-width: auto | <length> | <percentage>`
- `max-width: none | <length> | <percentage>`

Percentages are accepted only for the sizing properties above. Margin, padding,
font size, color, and display keep their existing property-specific value
contracts.

CSS `min-height` and `max-height` properties remain deferred because they are
not yet exposed by the CSS property model. The structured sizing resolver can
still represent block-axis percentage constraints through `StyleSizeInputs` for
future extension.

## CSS Value Contract

The CSS property system now represents the sizing value families as
`<length-percentage>` plus keyword branches:

- specified values preserve authored lengths, percentages, `auto`, and `none`
- computed values normalize lengths to CSS px
- computed values normalize percentages to finite fractions
- computed values do not resolve percentages to px

Layout owns percentage resolution because the required basis depends on the
containing block and formatting context.

## Resolution Basis

Percentage sizing resolves against `ConstraintSpace::containing_size`, not
against narrowed `AvailableSpace`.

For the current horizontal normal-flow subset:

- inline-axis percentages resolve against the containing block content-box
  inline size
- block-axis percentages resolve against the containing block content-box block
  size only when that block size is definite
- an indefinite containing-size basis produces
  `SizeResolutionReason::DeferredIndefinitePercentage`

The resolver must not treat an indefinite percentage basis as zero available
space. For inline-axis unsupported resolver cases, the current deterministic
fallback may be zero. For normal-flow block-axis percentage height with an
indefinite containing block, the resolver falls back to the auto content block
size while recording `DeferredIndefinitePercentage`.

## Integration With Constraints And Intrinsic Sizing

Percentage preferred sizes participate in the same post-preferred constraint
pipeline as lengths and intrinsic sizes:

1. Resolve the preferred content-box size from length, percentage, auto, or
   intrinsic behavior.
2. Apply style max constraint.
3. Apply the atomic-inline available content-space clamp where applicable.
4. Apply style min constraint last.
5. Expand the final content size to border-box geometry by adding padding.

Percentage min/max constraints resolve against the same containing-size basis
as percentage preferred sizes. If the percentage constraint basis is
indefinite, that individual constraint is ignored for the current pass rather
than synthesized as zero.

Intrinsic sizing remains content-owned. `width: auto` atomic inline boxes still
choose their intrinsic/shrink-to-fit preferred size first; percentage min/max
constraints may then constrain that result.

## Live Layout Behavior

Normal-flow layout now passes authored percentages through the existing
`SizeResolutionInput` handoff:

- block-level percentage widths resolve against the parent content-box width
- nested percentage widths use the parent used content width, not the page
  viewport when the parent is narrower
- atomic inline percentage widths resolve against their containing content-box
  width before the existing available-space clamp step
- percentage width constraints apply before padding expansion
- percentage heights in ordinary document flow resolve only when the containing
  block has a definite block size; otherwise they fall back to auto content
  height while preserving deferred metadata

## Determinism And Tests

X6 adds CSS tests for:

- specified percentage parsing for supported sizing properties
- computed preservation of `<length-percentage>` values
- updated property metadata and golden snapshots

X6 adds resolver-level tests for:

- percentage style input materialization
- percentage width resolving against containing size instead of available space
- indefinite percentage width deferral
- percentage max-width constraining an explicit width
- percentage height resolving with a definite block basis
- percentage height falling back to auto content height with an indefinite block
  basis

X6 adds layout-level tests for:

- nested percentage widths resolving against parent content width
- percentage min/max-width constraints in live layout
- atomic inline percentage width resolution

## Deferred Work

X6 intentionally does not implement:

- CSS `min-height` and `max-height`
- percentage margin or padding
- percentage resolution for replaced-element sizing paths still using the
  transitional replaced-size helper
- resolved percentage block-size behavior beyond the auto-content fallback used
  for ordinary document flow with an indefinite containing block
- calc/mixed arithmetic such as `calc(50% - 10px)`
- borders, `box-sizing`, and margin auto distribution
- flex, grid, table, float, positioned, orthogonal writing-mode, or
  fragmentation-specific percentage behavior

Later milestones must extend the typed CSS value and sizing input contracts
rather than resolving percentages locally in layout traversal.

## Exit Contract

X6 is complete while these remain true:

- supported sizing percentages are parsed and preserved through computed style
- layout-facing style inputs represent percentage preferred/min/max sizes
- resolver percentage resolution uses containing-size bases explicitly
- indefinite percentage bases produce deterministic deferred metadata
- percentage constraints integrate with X5 min/max ordering
- representative CSS, resolver, and live layout regression tests pass
