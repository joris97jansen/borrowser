# AA6: Overflow Clipping Paint Behavior

Last updated: 2026-06-12
Status: implemented paint clipping behavior for Milestone AA issue 6

This document records Borrowser's supported paint behavior for layout-owned
overflow clips. AA6 does not introduce new CSS overflow parsing, new layout
overflow semantics, scrolling, retained clip nodes, or compositor behavior.
It makes the paint-time clip scope explicit and deterministic.

Related code:
- `crates/gfx/src/paint/mod.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/contracts.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/flow.rs`

Related documents:
- `docs/rendering/aa1-paint-model-architecture-ordering-contracts.md`
- `docs/rendering/aa2-paint-primitives-input-model.md`
- `docs/rendering/aa3-border-rendering-box-decoration.md`
- `docs/rendering/aa4-outline-rendering-box-decoration.md`
- `docs/rendering/aa5-text-decoration-rendering-subset.md`
- `docs/rendering/y4-overflow-semantics-supported-subset.md`

## Ownership

CSS owns parsing, cascade, and computed-value normalization for the supported
`overflow` shorthand.

Layout owns overflow semantics. It maps computed overflow values to
`OverflowPolicy`, derives `OverflowClip` from final layout geometry, and exposes
that metadata through `LayoutBox::overflow_clip()`.

Paint consumes `LayoutBox::overflow_clip()` and emits a semantic
`PaintPrimitive::Clip` with `PaintClipScope::ContentsAndDescendants`. Paint must
not inspect raw CSS overflow values, decide whether a box clips, or synthesize
scrolling geometry.

GFX executes the immediate backend clip by using `egui::Painter::with_clip_rect`.
That clipped painter is an execution detail for the current frame, not retained
paint state.

Browser/runtime orchestration must not emulate or repair clipping behavior.

## Clip Scope

For a layout box with an `OverflowClip`, the box's own clip applies only to:

- inline content emitted for that box;
- child layout subtrees in layout child order;
- all descendant painting reached through those children, including descendant
  outlines.

The box's own clip does not apply to:

- the box's own background;
- the box's own border;
- the box's own list marker in the current supported order;
- the box's own outline.

Ancestor clips still apply to descendant painting. This means a descendant
outline is clipped by an ancestor overflow clip even though a box's own outline
is not clipped by that same box's own overflow clip.

## Paint Order

AA6 preserves the supported per-box paint order:

1. background
2. border
3. list marker
4. overflow clip for contents and descendants
5. inline formatting primitives
6. child paint nodes in layout child order
7. outline

The semantic paint input represents the clip as a `Clip` primitive before
inline content and child paint nodes. Immediate painting enters the backend clip
only while painting contents and descendants, then exits before painting the
box's own outline.

## Invariants

For fixed DOM, computed style, layout output, viewport, text measurer, resource
state, and input state:

- clip primitive construction is deterministic;
- clip rectangles come from layout-owned `OverflowClip` metadata;
- backend clipping uses the same layout-owned rectangle translated by the frame
  origin;
- paint never reinterprets raw CSS `overflow`;
- own background, border, and outline remain outside the box's own overflow
  clip;
- ancestor clips remain active for descendant painting.

## Deliberate Exclusions

AA6 does not implement:

- scrollbars;
- scroll offsets;
- scrolling interaction;
- viewport or body overflow propagation;
- border-radius clipping;
- background-clip refinements;
- stacking contexts or `z-index`;
- compositing;
- GPU or display-list architecture;
- retained clip nodes;
- pixel snapshot infrastructure.
