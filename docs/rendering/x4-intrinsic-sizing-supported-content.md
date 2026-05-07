# X4: Intrinsic Sizing For Supported Content

Last updated: 2026-05-06  
Status: implemented intrinsic sizing for Milestone X issue 4

This document defines the first concrete intrinsic sizing behavior built on the
X1-X3 sizing contracts. X4 gives Borrowser deterministic intrinsic inline-size
contributions for the supported normal-flow subset and wires those
contributions into auto atomic inline sizing.

Related code:
- `crates/layout/src/sizing.rs`
- `crates/layout/src/inline/intrinsic.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/box_tree/tests/projection.rs`

Related documents:
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x2-structured-size-resolution-model-inputs.md`
- `docs/rendering/x3-width-height-resolution-supported-subset.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`

## Purpose

Before X4, the layout pass always supplied `IntrinsicSizes::zero()` to
`SizeResolutionInput`. That made X1/X2 structurally correct, but auto-sized
atomic inline boxes still behaved as if content did not exist.

X4 introduces a deterministic intrinsic contribution pass for supported content
types and uses those contributions during used inline-size resolution.

## Supported Intrinsic Contributions

The current intrinsic pass supports:

- text runs in the current HTML-like whitespace-collapsing subset
- inline containers that flatten text and nested inline containers into one
  inline contribution stream
- atomic inline boxes, including inline-block boxes and replaced inline boxes
- block containers by taking the maximum of direct block-flow child
  contributions and own inline-formatting-context contributions
- replaced inline metadata currently available to layout, including images,
  text inputs, textareas, checkbox/radio controls, and buttons

Text min-content is the widest unbreakable word contribution. Text max-content
is the width of the collapsed text sequence with preserved single spaces
between emitted words. Atomic inline boxes contribute as unbreakable items to
their parent's inline intrinsic sizes.

`IntrinsicSizes` remains a content-contribution model. A box's own padding and
non-negative margins are added only when that box contributes as an atomic item
to an ancestor's inline formatting context. Its own used content size is still
resolved as a content box by `crates/layout/src/sizing.rs`.

Replaced control fallbacks may include deterministic UA-like internal control
chrome, but they must not include author CSS padding. Author padding belongs to
the normal content-box-to-border-box conversion and to outer contribution
calculation, so it is applied exactly once.

## Resolver Integration

`inline::refine_layout_with_inline` now materializes intrinsic contributions
for non-anonymous boxes before building `SizeResolutionInput`.

For block-level and document normal-flow boxes, auto inline size continues to
stretch to the available inline space as defined by X3.

For atomic inline boxes with `width: auto`, the inline resolver now uses the
box's intrinsic contributions:

- if available inline content space is definite, the preferred size is the
  supported shrink-to-fit result: clamp the available content width between
  min-content and max-content
- if available inline content space is indefinite, the preferred size is the
  intrinsic preferred/max-content size
- if no intrinsic contribution exists, the auto content-based preferred size is
  zero for the current subset

Explicit `width` continues to override intrinsic preferred width. `min-width`
and `max-width` still apply after preferred-size selection and before padding
expands the border-box geometry.

## Determinism And Tests

X4 adds resolver-level tests for:

- auto atomic inline width using intrinsic max-content when space is larger
- auto atomic inline shrink-to-fit when available space falls between
  min-content and max-content
- auto atomic inline width using intrinsic preferred size when available space
  is indefinite
- explicit width overriding intrinsic contributions

X4 adds layout-level regression tests for:

- auto inline-block width coming from text intrinsic width
- padding expanding intrinsic content width to border-box geometry
- replaced/control CSS padding being applied once in intrinsic contribution
  paths
- auto inline-block shrink-to-fit under constrained available width
- `max-width` applying after intrinsic preferred-size selection
- explicit inline-block width overriding intrinsic content width

## Deferred Work

X4 intentionally does not implement:

- CSS percentage parsing or percentage intrinsic behavior
- intrinsic keywords such as `min-content`, `max-content`, or `fit-content`
- full CSS text layout and Unicode line-breaking rules
- hyphenation, soft wrap opportunities beyond the current whitespace subset,
  or `white-space` modes
- full CSS 2.1 shrink-to-fit equations for every formatting context
- border and `box-sizing` participation
- min-height and max-height
- replaced-element sizing migration entirely into the shared resolver
- floats, positioning, flex, grid, table, orthogonal writing modes, or
  fragmentation sizing

Later milestones must extend this intrinsic pass deliberately instead of
reintroducing ad hoc content-size guesses in layout traversal code.

## Exit Contract

X4 is complete while these remain true:

- supported content types produce deterministic intrinsic inline-size
  contributions
- `SizeResolutionInput` receives non-zero intrinsic sizes where supported
- auto atomic inline width resolution uses intrinsic contributions
- explicit width and min/max-width remain authoritative in the right order
- representative resolver and layout regression tests pass
