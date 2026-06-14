# AA7: Deterministic Paint Ordering And Layering Rules

Last updated: 2026-06-12
Status: implemented deterministic ordering contract for Milestone AA issue 7

This document formalizes the paint ordering rules for Borrowser's current AA
supported paint subset. It strengthens the AA1 paint ordering contract and AA2
paint primitive model without introducing full CSS painting order, stacking
contexts, `z-index`, opacity, transforms, compositing, retained display lists,
GPU layers, or pixel snapshot infrastructure.

Related code:
- `crates/gfx/src/paint/contracts.rs`
- `crates/gfx/src/paint/debug.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/gfx/src/paint/inline.rs`
- `crates/gfx/src/paint/replaced.rs`
- `crates/gfx/src/paint/images.rs`
- `crates/gfx/src/paint/text_control.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/inline/types.rs`

Related documents:
- `docs/rendering/aa1-paint-model-architecture-ordering-contracts.md`
- `docs/rendering/aa2-paint-primitives-input-model.md`
- `docs/rendering/aa3-border-rendering-box-decoration.md`
- `docs/rendering/aa4-outline-rendering-box-decoration.md`
- `docs/rendering/aa5-text-decoration-rendering-subset.md`
- `docs/rendering/aa6-overflow-clipping-paint-behavior.md`
- `docs/rendering/aa8-paint-debug-visual-regression-surface.md`
- `docs/rendering/ab1-stacking-layering-invalidation-architecture.md`
- `docs/rendering/y4-overflow-semantics-supported-subset.md`

## Supported Ordering Contract

Paint order is a consequence of layout-owned traversal order plus paint-owned
per-box sequencing rules. Paint must not sort primitives after construction to
manufacture determinism.

For each paintable layout box, the supported AA order is:

1. box background
2. box border
3. list marker
4. overflow clip for contents and descendants
5. inline formatting content
6. child subtrees in layout child order
7. box outline

The explicit contract representation is `PaintOrderPhase` in
`crates/gfx/src/paint/contracts.rs`. It is a contract, debug, and enforcement
concept. It is not a display-list sort key, retained layer id, stacking context,
or compositor layer.

## Inline And Replaced Content

Inline formatting content is ordered by the layout-owned inline fragment
sequence. For the current supported subset:

- text fragments paint as text primitives;
- decorated text fragments paint text first, then text decoration;
- inline boxes paint at their inline fragment position;
- replaced fragments, images, and text controls paint as current supported
  inline paint items at their inline fragment position.

AA7 does not define finer-grained global ordering for replaced element internals
than the current immediate fragment paint path already exposes.

## Overflow Clips

Overflow clipping is a scoped paint execution context, not a stacking context,
layer, compositor concept, or retained clip node.

A box's own overflow clip applies to:

- inline content emitted for that box;
- child subtrees in layout child order;
- descendant painting reached through those children, including descendant
  outlines.

A box's own overflow clip does not apply to:

- the box's own background;
- the box's own border;
- the box's own list marker;
- the box's own outline.

Ancestor clips remain active for descendant painting. Paint consumes
layout-owned `OverflowClip` metadata; it must not inspect raw CSS overflow
values or decide whether a box clips.

## Semantic And Immediate Alignment

The semantic paint model and immediate backend painting must remain aligned:

- `PaintInput::to_order_debug_snapshot()` walks the semantic `PaintTree` in
  construction order;
- immediate painting follows the same per-box sequence in `paint_layout_box`;
- regression tests cover both semantic order and immediate output order.

The order debug snapshot is a stable regression surface. It is not a public API,
retained display list, scene graph, command buffer, or compositor artifact.
AA8 extends this with a paint-operation debug snapshot that uses the same
paint-owned ordering rules while remaining structural and backend-independent.

## Determinism Invariants

For a fixed DOM, computed style tree, layout output, viewport, text measurer,
resource state, and input state:

- layout tree traversal order is deterministic;
- sibling order comes from layout-owned child order;
- inline item order comes from layout-owned inline fragment order;
- paint-owned sequencing emits pre-content primitives before contents and
  child subtrees;
- outlines are emitted after child subtrees for the supported AA subset;
- overflow clips scope contents and descendants without moving parent-owned
  background, border, list marker, or outline into the box's own clip;
- semantic paint order snapshots are deterministic across construction runs;
- immediate backend rectangle paint order is deterministic for representative
  supported scenarios.

## Deliberate Exclusions

AA7 deliberately does not implement:

- full CSS painting order;
- stacking contexts;
- `z-index`;
- opacity layering;
- transforms;
- compositing;
- GPU layer trees;
- retained display lists;
- retained paint scenes;
- scrollbars or scroll offset painting;
- pixel or visual snapshot infrastructure;
- border-radius or background clipping refinements;
- broad replaced-control internal ordering beyond current supported inline
  paint item behavior.

Those features must extend the AB1 stacking, layering, and invalidation
architecture before changing traversal or layering behavior.
