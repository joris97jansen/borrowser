# AB3: Z-Order And Layering Semantics

Last updated: 2026-06-16
Status: implemented narrow z-order and layering subset for Milestone AB issue 3

This document defines Borrowser's first behavioral z-order and layering
subset. AB3 refines the AB2 stacking-context representation by allowing
paint-owned child stacking contexts for positioned generated boxes with
computed integer `z-index` values.

AB3 does not implement full CSS painting order, full positioned layout
geometry, opacity, transforms, filters, blend modes, isolation, containment,
compositor layers, GPU layers, retained display lists, retained paint scenes,
targeted repaint, dirty regions, or paint invalidation behavior.

Related code:
- `crates/css/src/values.rs`
- `crates/css/src/specified/z_index.rs`
- `crates/css/src/computed/style.rs`
- `crates/gfx/src/paint/stacking.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/debug.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/gfx/src/paint/contracts.rs`

Related documents:
- `docs/rendering/ab1-stacking-layering-invalidation-architecture.md`
- `docs/rendering/ab2-stacking-context-representation.md`
- `docs/rendering/aa7-deterministic-paint-ordering-layering-rules.md`
- `docs/rendering/aa9-paint-model-invariants-extension-points.md`
- `docs/rendering/aa6-overflow-clipping-paint-behavior.md`
- `docs/rendering/y5-positioned-containing-block-logic.md`

## Supported Subset

CSS supports authored and computed:

- `z-index: auto`;
- `z-index: <integer>`.

Paint gives `z-index` behavioral effect only for generated layout boxes whose
layout-owned positioning scheme is not `static`.

The AB3 stacking-context trigger is:

- positioned generated box;
- computed `z-index` is an integer.

`z-index: auto` does not create a child stacking context in AB3. A
non-positioned generated box with integer `z-index` computes the value, but it
does not gain z-order behavior in AB3.

## Ownership

CSS owns:

- parsing `z-index`;
- rejecting unsupported `z-index` values;
- cascade, initial value, and computed `ZIndex`;
- exposing `ComputedStyle::z_index()`.

Layout owns:

- generated box identity;
- deterministic layout tree order;
- final geometry;
- positioning metadata;
- overflow clip metadata;
- computed-style handoff.

Layout does not sort paint order, decide stacking-context membership, or
reinterpret `z-index`.

Paint owns:

- child stacking-context construction;
- semantic layer assignment;
- z-order resolution;
- deterministic tie-breaking;
- resolved semantic debug snapshots;
- immediate paint traversal order.

Browser/runtime does not reinterpret stacking, layering, or z-order.

## Layer Ordering

Inside one stacking context, AB3 uses paint-owned semantic layer buckets:

1. negative integer `z-index` child contexts;
2. normal-flow stackable paint subtree for the context source;
3. zero integer `z-index` child contexts;
4. positive integer `z-index` child contexts.

Negative, zero, and positive child contexts are ordered by:

1. semantic layer;
2. integer `z-index` value;
3. stable layout preorder.

This tie-breaking is deterministic and must not depend on `HashMap` or other
unordered iteration.

Child stacking contexts are atomic relative to sibling paint items in the
parent context. When a child context is emitted, paint emits the child context
root's own primitives and its non-context descendants, while skipping nested
child context roots until those nested contexts are emitted by their owning
context.

## Overflow Clips

Overflow clips remain scoped paint execution contexts, not stacking contexts,
semantic layers, compositor layers, or invalidation boundaries.

When a child stacking context is emitted through z-order traversal, ancestor
layout-owned overflow clips still apply to that child context's immediate paint
output and operation debug snapshot. Paint derives those clips from layout
metadata; it does not inspect raw CSS overflow declarations.

## Debug Surfaces

`PaintInput::to_stacking_context_debug_snapshot()` serializes:

- context source;
- parent context;
- child context count;
- stackable item count;
- semantic layer;
- optional integer `z-index`;
- stable layout preorder.

`PaintInput::to_order_debug_snapshot()` and
`PaintInput::to_operation_debug_snapshot()` consume the resolved stacking
traversal. Immediate painting uses the same resolved traversal. Backend draw
commands are not sorted after emission.

## Invariants

For a fixed DOM, computed style tree, layout output, viewport, text measurer,
resource state, and input state:

- stacking-context construction is deterministic;
- the root context remains `StackingContextId::ROOT`;
- AB3 child contexts are created only for positioned integer `z-index` boxes;
- `z-index: auto` does not create a child context;
- integer `z-index` on static boxes has no paint-order effect;
- child contexts are atomic relative to sibling parent-context content;
- same-layer and same-`z-index` ties resolve by layout preorder;
- overflow clips remain clips and still apply to descendant child contexts;
- operation snapshots and immediate painting use the same semantic order;
- backend, compositor, GPU, retained-scene, and invalidation concepts do not
  leak into the representation.

## Deliberate Exclusions

AB3 deliberately excludes:

- full CSS painting order;
- full CSS stacking-context trigger set;
- `z-index` behavior for non-positioned boxes;
- relative offsets, inset positioning, and final absolute/fixed/sticky
  geometry;
- opacity, transforms, filters, blend modes, isolation, and containment;
- compositor layers;
- GPU layers or promotion logic;
- retained display lists;
- retained paint scenes;
- targeted repaint execution;
- dirty-region computation;
- paint invalidation boundaries;
- pixel or raster snapshot testing.
