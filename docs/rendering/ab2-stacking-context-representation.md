# AB2: Stacking Context Representation

Last updated: 2026-06-15
Status: implemented explicit root stacking-context representation for Milestone AB issue 2

This document defines Borrowser's first concrete stacking-context
representation. AB2 makes stacking contexts explicit, deterministic, and
testable inside the paint pipeline without changing visual paint order.

AB2 does not implement `z-index`, opacity, transforms, filters, blend modes,
isolation, containment, compositor layers, GPU layers, retained display lists,
retained paint scenes, targeted repaint, dirty regions, or paint invalidation
behavior.

Related code:
- `crates/gfx/src/paint/stacking.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/contracts.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/layout/src/layout_box.rs`

Related documents:
- `docs/rendering/ab1-stacking-layering-invalidation-architecture.md`
- `docs/rendering/aa7-deterministic-paint-ordering-layering-rules.md`
- `docs/rendering/aa9-paint-model-invariants-extension-points.md`
- `docs/rendering/aa6-overflow-clipping-paint-behavior.md`
- `docs/rendering/y9-advanced-flow-invariants-extension-points.md`

## Scope

AB2 introduces a paint-owned, frame-local `StackingContextTree` attached to
`PaintInput`.

The supported subset is intentionally root-context-only:

- every `PaintInput` has one root stacking context;
- the root context has deterministic `StackingContextId::ROOT`;
- every currently paintable layout box belongs to the root context;
- membership follows layout-owned traversal order;
- the existing AA paint order remains unchanged.

AB2 does not create placeholder child contexts. Future issues must add child
contexts only when they define the computed style and layout facts that
establish those contexts and the ordering rules that use them.

## Ownership

Paint/GFX owns stacking-context semantics:

- `StackingContextId`;
- `StackingContextTree`;
- `StackingContextNode`;
- `StackablePaintItem`;
- stacking-context debug snapshots.

Layout provides stable inputs only:

- generated box identity;
- deterministic layout tree order;
- final geometry;
- existing positioning metadata;
- existing overflow clip metadata.

Layout does not store stacking-context ownership, sort paint primitives, create
compositor layers, or decide paint invalidation boundaries.

CSS owns future authored and computed style inputs that may establish stacking
contexts, but AB2 does not add CSS properties or computed values.

Browser/runtime owns frame orchestration and invalidation requests. It does not
reinterpret stacking membership or decide paint order.

## Root Stacking Context

`StackingContextId::ROOT` is always `0` for a frame-local
`StackingContextTree`.

The root context source is the root layout box represented as
`StackingContextSource::RootDocument(PaintSource)`. It has no parent and no
child contexts in AB2.

The root context is a semantic paint representation. It is not a compositor
layer, retained scene node, display list, backend command buffer, texture, GPU
resource, or invalidation boundary.

## Stackable Paint Items

`StackablePaintItem` records that a paint source belongs to a stacking context.
For AB2, all items belong to `StackingContextId::ROOT`.

Items are collected from layout boxes before immediate backend drawing. They
are not discovered by scanning emitted backend operations.

Non-rendering element subtrees follow the existing paint behavior and are not
added as stackable paint items.

## Overflow Clips

Overflow clips remain scoped paint execution contexts derived from
layout-owned `OverflowClip` metadata.

An overflow clip does not establish:

- a stacking context;
- a semantic paint layer;
- a compositor layer;
- a retained clip node;
- a scroll container;
- a paint invalidation boundary.

## Debug Surface

`PaintInput::to_stacking_context_debug_snapshot()` serializes the
`StackingContextTree` in deterministic form.

The snapshot is backend-independent. It must not include egui command details,
texture identifiers, GPU handles, compositor state, pixel output, or runtime
draw ordering.

The snapshot proves:

- the root context exists;
- root identity, parentage, and source are deterministic;
- box-to-context membership is deterministic;
- overflow clips do not create contexts.

## Invariants

For a fixed DOM, computed style tree, layout output, viewport, text measurer,
resource state, and input state:

- stacking-context representation is deterministic;
- the root context always exists;
- `StackingContextId::ROOT` is the root context ID;
- box membership preserves layout-owned traversal order;
- every stackable item has a valid owning context;
- visual paint order remains the AA supported order;
- overflow clips remain clip scopes, not stacking contexts;
- stacking representation is built before immediate backend drawing;
- backend, compositor, GPU, retained-scene, and invalidation concepts do not
  leak into the representation.

## Deliberate Exclusions

AB2 deliberately excludes:

- `z-index` parsing, computed values, sorting, or behavior;
- full CSS painting order;
- opacity, transforms, filters, blend modes, isolation, and containment
  stacking triggers;
- semantic child stacking contexts;
- semantic paint layers beyond the root context representation;
- compositor layers;
- GPU layers or promotion logic;
- retained display lists;
- retained paint scenes;
- targeted repaint execution;
- dirty-region computation;
- paint invalidation boundaries;
- pixel or raster snapshot testing;
- new visual behavior.

Future issues must remove these exclusions one at a time with explicit
contracts, deterministic debug surfaces, and tests.
