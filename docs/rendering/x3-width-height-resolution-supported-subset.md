# X3: Width And Height Resolution For The Supported Subset

Last updated: 2026-05-06  
Status: implemented width and height resolution for Milestone X issue 3

This document defines the first concrete used-size resolver built on the X1/X2
sizing contracts. X3 strengthens Borrowser's supported normal-flow width and
height behavior without introducing flex, positioning, border sizing,
percentages in CSS parsing, or full shrink-to-fit algorithms.

Related code:
- `crates/layout/src/sizing.rs`
- `crates/layout/src/inline/intrinsic.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/geometry.rs`
- `crates/layout/src/box_tree/tests/projection.rs`

Related documents:
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x2-structured-size-resolution-model-inputs.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`

## Purpose

Before X3, normal-flow layout mixed sizing decisions directly into recursive
geometry refinement. Width was mostly passed through as an available border-box
number, explicit `width` behaved like border-box width, and explicit `height`
was not applied to ordinary normal-flow boxes.

X3 introduces resolver functions that consume `SizeResolutionInput` and produce
deterministic content-box used sizes plus border-box geometry sizes. The
inline-aware normal-flow refinement pass now uses those functions as the
authoritative source for box `rect.width` and `rect.height`.

## Supported Resolution Model

The supported model is still intentionally narrow:

- horizontal writing-mode only
- no borders
- no `box-sizing`
- no `min-height` or `max-height` CSS properties yet
- no parsed CSS percentages yet
- normal-flow block, anonymous block, document/root, inline-level, and atomic
  inline sizing roles

Within that subset, the contract is:

- `width` and `height` are content-box sizes.
- Padding expands the final border-box geometry.
- Auto block-level width stretches to the available inline space offered by the
  containing formatting context, with content size reduced by own padding.
- Explicit width/height are preferred content sizes.
- `min-width` and `max-width` apply to content width before padding expands the
  border box.
- Auto height is content-derived from inline lines and block children.
- Explicit height overrides content-derived auto height and is then expanded by
  vertical padding.
- Atomic inline boxes keep the current supported shrink-to-fit-equivalent
  behavior by clamping content width to definite available inline content
  space.
- Indefinite available inline size does not synthesize a zero available-space
  clamp. Unsupported auto-stretch cases are deferred explicitly.
- Anonymous generated boxes use zero box metrics for sizing, even though they
  retain an anchor style for bridge compatibility.

## Resolver Entry Points

`crates/layout/src/sizing.rs` provides:

- `NormalFlowSizingMode`
- `ResolvedAxisSize`
- `resolve_normal_flow_inline_size`
- `resolve_normal_flow_block_size`

`ResolvedAxisSize` exposes:

- `content`: the `UsedAxisSize` content-box result, including preferred reason
  and applied post-preferred adjustment metadata
- `border`: the border-box axis size used by `LayoutBox::rect`

This preserves the X1 distinction between preferred-size selection,
post-preferred min/max or available-space adjustment, and final geometry
conversion.

## Integration With Normal Flow

`inline::refine_layout_with_inline` remains the authoritative normal-flow
geometry pass. It now:

- builds a `SizeResolutionInput` for each `LayoutBox`
- resolves inline size before laying out descendants
- computes child and line content heights
- resolves block size after content height is known
- writes the resolved border-box sizes to `LayoutBox::rect`

The initial `LayoutBox` projection still creates placeholder geometry. Final
used width and height are owned by the refinement pass.

## Determinism And Tests

X3 adds resolver-level tests for:

- auto width filling the available border box after padding
- explicit width as content-box size
- min/max constraints applying to content width before padding
- atomic inline available-space clamping being distinct from style `max-width`
- indefinite available inline size not clamping atomic inline width to zero
- explicit height as content-box size

X3 also adds layout-level regression tests for:

- explicit width plus padding
- auto width plus padding
- max-width plus padding
- min-width plus padding
- explicit height plus padding
- nested auto width derived from the parent content box
- atomic inline width clamping to available content space
- anonymous generated boxes not inheriting anchor padding for sizing

These tests exercise both the pure sizing model and the integrated normal-flow
geometry path.

## Deferred Work

X3 intentionally does not implement:

- CSS percentage parsing
- percentage width/height behavior in live layout
- min-height or max-height
- borders or `box-sizing`
- margin auto distribution
- full CSS 2.1 block width equation support
- broader intrinsic shrink-to-fit beyond the X4 supported atomic-inline subset
- replaced-element sizing migration into the shared resolver
- floats, positioning, flex, grid, table, or fragmentation sizing

Those features must extend the X1/X2 sizing contracts deliberately.

## Exit Contract

X3 is complete while these remain true:

- supported normal-flow width resolution uses the structured sizing model
- supported normal-flow height resolution uses the structured sizing model
- CSS width/height behave as content-box sizes in the supported subset
- min/max-width constraints apply before padding expansion
- normal-flow layout derives descendant available width from parent content
  width
- representative resolver and layout regression tests pass
