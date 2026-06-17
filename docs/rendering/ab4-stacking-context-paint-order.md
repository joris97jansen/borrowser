# AB4: Stacking-Context Paint Order Execution

Last updated: 2026-06-17
Status: implemented shared stacking-context paint-order execution for Milestone AB issue 4

This document defines how Borrowser resolves paint order from the explicit
stacking-context model introduced by AB2 and refined by AB3.

AB4 does not introduce new stacking-context triggers, new CSS property support,
full CSS painting order, opacity, transforms, filters, blend modes, isolation,
containment, compositor layers, GPU layers, retained display lists, retained
paint scenes, targeted repaint, dirty regions, or paint invalidation behavior.

Related code:
- `crates/gfx/src/paint/stacking.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/debug.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/gfx/src/paint/context.rs`

Related documents:
- `docs/rendering/ab1-stacking-layering-invalidation-architecture.md`
- `docs/rendering/ab2-stacking-context-representation.md`
- `docs/rendering/ab3-z-order-layering-semantics.md`
- `docs/rendering/aa7-deterministic-paint-ordering-layering-rules.md`
- `docs/rendering/aa8-paint-debug-visual-regression-surface.md`
- `docs/rendering/aa9-paint-model-invariants-extension-points.md`
- `docs/rendering/aa6-overflow-clipping-paint-behavior.md`

## Purpose

AB2 made stacking contexts explicit. AB3 added a narrow behavioral subset for
positioned generated boxes with computed integer `z-index`. AB4 makes that
explicit `StackingContextTree` the shared source of cross-context paint order.

The current AB4 ordering flow is:

```text
LayoutPhaseOutput
  -> PaintInput
  -> PaintTree
  -> StackingContextTree
  -> StackingOrderSlot sequence
  -> order debug snapshot
  -> operation debug snapshot
  -> immediate backend painting
```

The resolved slot sequence is paint-owned semantic data. It is not a retained
display list, scene graph, compositor layer tree, GPU resource model, backend
command stream, or invalidation boundary.

## Ownership

CSS owns authored and computed values such as `z-index`.

Layout owns generated box identity, geometry, deterministic tree order,
positioning metadata, inline fragments, replaced metadata, and overflow clip
rectangles.

Paint/GFX owns stacking-context paint ordering. Paint consumes CSS and layout
facts through existing phase outputs, builds `StackingContextTree`, resolves
`StackingOrderSlot` values, and emits debug snapshots and immediate output from
that resolved order.

Browser/runtime may request paint work, but it must not decide which box paints
above another.

## Shared Ordering API

`StackingContextTree::ordered_slots(context_id)` returns the canonical AB4
paint order for one context:

1. negative integer `z-index` child contexts;
2. the context source subtree;
3. zero integer `z-index` child contexts;
4. positive integer `z-index` child contexts.

Child contexts within each z-index bucket are ordered by:

1. semantic layer;
2. integer `z-index` value;
3. stable layout preorder.

The slot vocabulary is:

- `StackingOrderSlot::ChildContext(StackingContextId)`;
- `StackingOrderSlot::ContextSource(PaintSource)`.

Consumers must use this slot sequence for cross-context order. They must not
rebuild the negative/source/zero/positive order separately, sort already
flattened paint primitives, or fall back to simple layout traversal once the
stacking-context tree is available.

## Source Subtree Traversal

Inside a `ContextSource` slot, paint preserves the supported AA per-box order:

1. box background;
2. box border;
3. list marker;
4. overflow clip for contents and descendants;
5. inline formatting content;
6. in-context child subtrees in layout child order;
7. box outline.

If a child layout box starts a different stacking context, that child root is
not emitted through its parent's normal source-subtree traversal. It is emitted
only through its explicit `ChildContext` slot.

`StackingContextTree::source_starts_external_context(owner_context, source)`
is the shared predicate for this skip behavior.

## Debug And Immediate Alignment

These consumers share the same `StackingOrderSlot` order:

- `PaintInput::to_order_debug_snapshot()`;
- `PaintInput::to_operation_debug_snapshot()`;
- `paint_page()` immediate backend painting.

This keeps semantic order snapshots, structural operation snapshots, and visible
immediate output aligned without sorting final flattened primitives.

## Overflow Clips

Overflow clips remain layout-owned clip scopes, not stacking contexts, semantic
layers, compositor layers, retained clip nodes, or invalidation boundaries.

When a child stacking context is emitted through a `ChildContext` slot, ancestor
overflow clips still apply to that child context's immediate output and
operation debug snapshot. A box's own overflow clip keeps the AA6 scope: it
clips contents and descendants, not the box's own background, border, list
marker, or outline.

## Invariants

For a fixed DOM, computed style tree, layout output, viewport, text measurer,
resource state, and input state:

- `StackingContextTree::ordered_slots` is deterministic;
- all cross-context paint order is resolved before primitive emission or
  immediate backend drawing;
- child stacking contexts are atomic relative to sibling parent-context content;
- child context roots are skipped from parent source-subtree traversal;
- same-layer and same-`z-index` ties resolve by stable layout preorder;
- static boxes with integer `z-index` remain in source-subtree order;
- positioned boxes with `z-index: auto` remain in source-subtree order;
- order snapshots, operation snapshots, and immediate painting use the same
  resolved stacking order;
- overflow clips remain clips and still apply across child context emission;
- browser/runtime does not infer or reinterpret paint order.

## Deliberate Exclusions

AB4 deliberately excludes:

- full CSS painting order;
- full CSS stacking-context trigger set;
- `z-index` behavior outside AB3's positioned integer subset;
- opacity, transforms, filters, blend modes, isolation, and containment;
- compositor layers;
- GPU layers or promotion logic;
- retained display lists;
- retained paint scenes;
- targeted repaint execution;
- dirty-region computation;
- paint invalidation boundaries;
- pixel or raster snapshot testing.

