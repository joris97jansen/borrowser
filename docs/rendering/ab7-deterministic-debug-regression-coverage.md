# AB7: Deterministic Debug Output And Regression Coverage

Last updated: 2026-06-18
Status: implemented deterministic debug output and regression coverage for
Milestone AB issue 7

This document defines Borrowser's AB7 debug and regression contract for the
stacking, layering, paint invalidation, and repaint execution foundations
introduced by AB2 through AB6.

AB7 does not introduce new rendering semantics. It does not add new CSS
behavior, new `z-index` behavior, compositor layers, GPU concepts, retained
display lists, retained paint scenes, dirty rectangles, per-node repaint, or
backend command serialization.

Related code:
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/debug.rs`
- `crates/gfx/src/paint/stacking.rs`
- `crates/browser/src/rendering/debug.rs`
- `crates/browser/src/rendering/invalidation.rs`
- `crates/browser/src/rendering/frame.rs`
- `crates/browser/src/rendering/tests/invalidation.rs`
- `crates/browser/src/rendering/tests/frame_trace.rs`

Related documents:
- `docs/rendering/ab1-stacking-layering-invalidation-architecture.md`
- `docs/rendering/ab2-stacking-context-representation.md`
- `docs/rendering/ab3-z-order-layering-semantics.md`
- `docs/rendering/ab4-stacking-context-paint-order.md`
- `docs/rendering/ab5-structured-paint-invalidation-model.md`
- `docs/rendering/ab6-basic-targeted-repaint-behavior.md`
- `docs/rendering/aa8-paint-debug-visual-regression-surface.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`

## Purpose

AB2 through AB4 made stacking contexts, semantic z-order layers, and shared
paint-order execution explicit. AB5 and AB6 made paint invalidation requests
and repaint execution scopes explicit. AB7 makes those decisions inspectable
through deterministic debug surfaces and representative regression tests.

The AB7 proof surfaces answer:

- which paint-owned stacking contexts exist;
- which semantic layer each context belongs to;
- which canonical ordered slots paint will consume;
- which runtime invalidation requests dirtied paint;
- which conservative effective invalidation scope was selected;
- which repaint execution plan the runtime derived.

These surfaces are internal regression contracts. They are not public APIs.

## Ownership

Paint/GFX owns stacking, layering, paint order, paint primitive construction,
and paint-side debug serialization. Paint-side AB7 output consumes
`StackingContextTree::ordered_slots(...)`, the same canonical slot path used by
paint order snapshots, operation snapshots, and immediate painting.

Browser/runtime owns invalidation decisions, pending render work, paint
invalidation derivation, repaint planning, frame orchestration, and
runtime-side debug serialization. Runtime-side AB7 output consumes
`PendingRenderWork::paint_invalidations()`,
`PendingPaintInvalidations::effective_scope()`, and
`RepaintExecutionPlan` construction.

CSS owns authored and computed style inputs. Layout owns layout tree order,
geometry, positioning metadata, and generated box identity. AB7 does not move
either ownership boundary.

## Paint-Side Debug Surface

`PaintInput::to_layering_debug_snapshot()` serializes the AB7 layering surface:

```text
version: 1
paint-layering-snapshot
layout-root-id: ...
root-context: ...
context id=... parent=... source=... layer=... z-index=... tree-order=... children=... items=...
  items:
    item[0]: source=... layer=... z-index=... tree-order=...
  ordered-slots:
    slot[0]: child-context id=... source=... layer=... z-index=... tree-order=...
    slot[1]: context-source source=...
```

The snapshot records semantic paint layers, not compositor layers. Child
contexts are emitted by walking `StackingContextTree::ordered_slots(...)`.
Debug serialization must not reconstruct negative/source/zero/positive order
independently, sort already-flattened primitives, or scan backend draw output.

AB7 also keeps these existing paint surfaces aligned:

- `PaintInput::to_stacking_context_debug_snapshot()`
- `PaintInput::to_order_debug_snapshot()`
- `PaintInput::to_operation_debug_snapshot()`
- immediate `paint_page(...)` execution

## Runtime Debug Surface

`browser::rendering::paint_invalidation_debug_snapshot(...)` serializes the
AB7 runtime invalidation surface:

```text
version: 1
paint-invalidation-snapshot
pending-render-work: ...
paint-invalidations: ...
  request[0]: entry-point=... trigger=... reason=... scope=...
effective-scope: ...
repaint-execution-plan: scope=...
```

The snapshot is derived from pending render work and the existing AB5/AB6
runtime APIs. It must not duplicate the paint invalidation mapping table,
derive scope from layout or paint primitives, or infer repaint behavior from
backend output.

`RenderFrameExecutionTrace::to_debug_snapshot()` remains the frame-level
orchestration surface. AB7 pins representative repaint execution output in
exact regression tests.

## Regression Coverage

AB7 regression tests cover:

- overlapping positioned boxes with deterministic negative, normal, zero, and
  positive layer order;
- child stacking-context atomicity;
- semantic layering, paint-order, and operation snapshot alignment;
- paint invalidation request mapping;
- pending paint invalidation deduplication and first-seen order;
- conservative effective invalidation scope selection;
- repaint execution plan and frame trace debug output.

These tests are semantic. They do not depend on pixels, egui commands, GPU
handles, texture identifiers, platform font output, or live website behavior.

## Invariants

For fixed DOM, computed style tree, layout output, viewport, text measurer,
resource state, input state, and pending render work:

- AB7 debug output is deterministic;
- paint-side layering snapshots consume paint-owned stacking order;
- runtime-side invalidation snapshots consume runtime-owned invalidation and
  repaint planning APIs;
- exact snapshot formatting is a regression contract;
- child stacking contexts remain atomic relative to sibling parent-context
  content;
- operation snapshots and semantic order snapshots remain aligned with the
  canonical stacking slot path;
- full-document repaint remains represented as an explicit `Document` scope;
- viewport repaint remains represented as an explicit `Viewport` scope;
- no frame-local paint, layout, or stacking identifiers are retained as
  runtime invalidation keys.

## Deliberate Exclusions

AB7 deliberately excludes:

- compositor layers;
- GPU concepts or GPU promotion;
- retained display lists;
- retained paint scenes;
- dirty rectangles;
- paint-source-scoped invalidation;
- stacking-context-scoped invalidation;
- per-node repaint;
- new CSS property support or new `z-index` behavior;
- backend command serialization;
- pixel or raster screenshots;
- live-site visual validation as proof of correctness.

Future issues may add narrower repaint or retained paint behavior only after
defining stable ownership, identifiers, dependency derivation, fallback
behavior, deterministic debug output, and regression coverage.
