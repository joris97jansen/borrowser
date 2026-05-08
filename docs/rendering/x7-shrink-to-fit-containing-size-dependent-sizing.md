# X7: Shrink-To-Fit And Containing-Size-Dependent Sizing

Last updated: 2026-05-07  
Status: implemented shrink-to-fit and containing-size-dependent sizing for Milestone X issue 7

This document defines the supported shrink-to-fit and available-space-dependent
behavior built on the X1-X6 sizing contracts.

Related code:
- `crates/layout/src/sizing.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/inline/intrinsic.rs`
- `crates/layout/src/box_tree/tests/projection.rs`

Related documents:
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x2-structured-size-resolution-model-inputs.md`
- `docs/rendering/x3-width-height-resolution-supported-subset.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/x5-min-max-sizing-constraints.md`
- `docs/rendering/x6-percentage-sizing-targeted-subset.md`
- `docs/rendering/x8-flow-correctness-varied-sizing.md`
- `docs/rendering/x9-deterministic-sizing-debug-regressions.md`
- `docs/rendering/x10-sizing-invariants-extension-hooks.md`

## Supported Scope

X7 applies live shrink-to-fit behavior to atomic inline boxes with `width: auto`
in the current normal-flow subset. This includes inline-block boxes whose
intrinsic inline contribution is produced by supported text, inline, block
child, and replaced/control content paths.

Block-level auto width remains stretch-to-available. Future shrink-to-fit cases
for floats, absolutely positioned boxes, tables, flex, grid, and fragmentation
remain deferred until those formatting modes exist.

## Size Bases

X7 keeps the X2 distinction between containing size and available space:

- `ContainingSize` is the percentage basis for CSS sizing values.
- `AvailableSpace` is the formatting context space offered to a box.
- shrink-to-fit uses available content inline size after the box's own padding
  has been subtracted.

`ConstraintSpace::available_size_after_edges` centralizes that subtraction. It
preserves indefinite available space instead of synthesizing zero, and it does
not change the containing-size basis used by percentages.

## Shrink-To-Fit Contract

The supported shrink-to-fit formula is:

```text
min(max(min-content, available), preferred-ceiling)
```

`preferred-ceiling` is the intrinsic preferred inline size clamped between
min-content and max-content, or max-content when no separate preferred
intrinsic size is available.

The typed resolver primitive is:

```text
ShrinkToFitInput
  -> resolve_shrink_to_fit_inline_size
  -> ShrinkToFitResult
```

`ShrinkToFitResult` records the deterministic branch that produced the value:

- empty intrinsic contribution
- indefinite available space
- min-content floor
- available-space result
- preferred/max-content ceiling

For definite available inline space, the resolver uses the formula above with
an explicit preferred ceiling. The ceiling is the intrinsic preferred inline
size clamped between min-content and max-content, or max-content when no
preferred intrinsic size is available. For indefinite available inline space,
the resolver uses the same preferred ceiling.

## Integration With Percentages And Constraints

Atomic inline `width: auto` first resolves its shrink-to-fit preferred
content-box size from intrinsic contributions and available content space.
Then the existing X5/X6 constraint pipeline applies:

1. style max constraint, including percentage `max-width` resolved against
   `ContainingSize`
2. atomic-inline available-space clamp only for non-auto preferred widths
3. style min constraint, including percentage `min-width` resolved against
   `ContainingSize`
4. padding expansion from final content-box size to border-box geometry

This means `width: auto` shrink-to-fit is not double-clamped by available
space, and percentage constraints never resolve against narrowed available
space.

## Determinism And Tests

X7 adds resolver-level coverage for:

- `ConstraintSpace::available_size_after_edges`
- shrink-to-fit min-content, available-space, and preferred/max-content
  ceiling branches
- preferred inline size acting as the definite shrink-to-fit ceiling
- indefinite available space using intrinsic preferred size
- empty intrinsic contributions
- atomic inline auto width using the min-content floor
- percentage max-width using containing size instead of available space
- percentage max-width constraining after shrink-to-fit preferred-size
  selection

X7 adds layout-level coverage for:

- auto inline-block shrink-to-fit hitting the min-content floor when available
  inline space is narrower than the longest unbreakable contribution

## Deferred Work

X7 intentionally does not implement:

- authored intrinsic sizing keywords such as `min-content`, `max-content`, or
  `fit-content`
- full CSS text shaping, Unicode line-breaking, hyphenation, or `white-space`
  modes
- shrink-to-fit for floats, absolutely positioned boxes, tables, flex, grid, or
  fragmentation
- border and `box-sizing` participation
- margin auto distribution
- migration of every replaced-element sizing path into the shared resolver
- aspect-ratio transfer after post-preferred constraints

Later milestones must extend the typed shrink-to-fit inputs and decisions
instead of reintroducing local width clamps in layout traversal.

## Exit Contract

X7 is complete while these remain true:

- shrink-to-fit is represented by explicit input/result contract types
- the supported formula is centralized and regression-tested
- definite and indefinite available sizes remain distinct
- percentages continue to resolve against `ContainingSize`
- min/max constraints apply after shrink-to-fit preferred-size selection
- representative resolver and live layout regressions pass
