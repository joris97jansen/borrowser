# AC7: Retained Paint Artifact Reuse And Repaint Planning

Status: implemented conservative retained paint artifact reuse and repaint
planning for Milestone AC issue 7

This document defines Borrowser's first retained paint artifact reuse path.
AC7 connects retained runtime state to paint invalidation, repaint planning,
and conservative paint artifact lifetime without moving paint semantics into
Browser/runtime.

AC7 does not introduce compositor layers, GPU layers, dirty-region rendering,
partial raster invalidation, retained backend draw commands, a full display-list
architecture, broad paint dependency graphs, browser-owned stacking/order
interpretation, or browser-owned CSS property-impact tables.

Related code:

- `crates/gfx/src/paint/mod.rs`
- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/rendering/work_plan.rs`
- `crates/browser/src/rendering/frame.rs`
- `crates/browser/src/rendering/invalidation.rs`
- `crates/browser/src/rendering/lifecycle.rs`
- `crates/gfx/src/viewport.rs`

Related documents:

- `docs/rendering/ac1-retained-render-state-runtime-contract.md`
- `docs/rendering/ac2-retained-render-identities.md`
- `docs/rendering/ac3-explicit-dirty-state-tracking.md`
- `docs/rendering/ac4-deterministic-render-work-plans.md`
- `docs/rendering/ac5-retained-style-artifact-reuse.md`
- `docs/rendering/ac6-retained-layout-artifact-foundation.md`
- `docs/rendering/ab5-structured-paint-invalidation-model.md`
- `docs/rendering/ab6-basic-targeted-repaint-behavior.md`
- `docs/rendering/aa1-paint-model-architecture-ordering-contracts.md`
- `docs/rendering/aa9-paint-model-invariants-extension-points.md`
- `docs/rendering/ab1-stacking-layering-invalidation-architecture.md`
- `docs/rendering/ab2-stacking-context-representation.md`
- `docs/rendering/ab4-stacking-context-paint-order.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`

## Ownership

Browser/runtime owns:

- retained paint artifact lifetime;
- retained paint artifact keys;
- reuse, recompute, discard, and fallback accounting;
- repaint planning and repaint scope selection;
- deterministic retained paint debug output.

Paint owns:

- the semantic paint artifact data type;
- paint artifact construction from layout-to-paint input;
- paint primitives;
- stacking-context construction;
- paint ordering;
- semantic paint layers;
- paint debug meaning.

CSS owns style-impact classification. Browser/runtime consumes CSS-owned facts
such as paint-only versus layout-affecting changes and must not duplicate a CSS
property-impact table.

Layout owns geometry and retained layout artifacts. Layout dirtiness implies
paint dirtiness when geometry can affect paint output.

## Retained Paint Artifact

The only retained paint artifact introduced by AC7 is a paint-owned semantic
artifact derived from the current frame's `PaintPhaseInput`. It may contain
owned paint semantic structures such as the paint tree and stacking-context
tree.

The paint artifact is not:

- an egui/backend draw-command cache;
- a retained GPU resource;
- a compositor layer;
- a dirty-region graph;
- a browser-owned display list;
- a paint dependency graph;
- a set of retained frame-local paint or stacking IDs.

Paint does not know about retained render identity domains, retained render
IDs, layout artifact keys, render epochs, style generations, or paint input
generations. Browser/runtime wraps the paint-owned artifact in a runtime-owned
retained entry keyed by those runtime concepts.

## Retained Paint Key

Browser/runtime keys retained paint entries by:

```text
RetainedPaintArtifactKey {
  identity_domain,
  layout_key,
  paint_style_generation,
  paint_input_generation,
}
```

`identity_domain` prevents reuse across document replacement.

`layout_key` ties paint reuse to the retained layout artifact that supplied
paint geometry and layout-owned metadata.

`paint_style_generation` changes when CSS-owned computed-style impact
classification says a style update is paint-only, layout-affecting, or
unknown.

`paint_input_generation` changes for runtime paint dependencies such as input
state, resource state, and other paint-relevant runtime inputs.

`RenderEpoch` is not a retained paint cache key.

## Reuse And Recompute Rules

Retained paint may be reused only when:

- a retained paint entry exists;
- the retained paint key matches the current key;
- retained paint dirty state is clean;
- Browser/runtime selected a no-op retained paint path for the frame.

Paint is recomputed when:

- no retained paint entry exists;
- the retained paint key mismatches;
- paint is dirty;
- layout is dirty or layout artifact reuse fails;
- paint-relevant style changes occur;
- runtime paint inputs change;
- conservative fallback is selected.

Paint-only style changes may avoid relayout only when CSS-owned impact
classification says relayout is unnecessary. They still dirty paint and force
retained paint recomputation.

Viewport repaint planning uses the existing AB5/AB6 `Document` and `Viewport`
scopes. AC7 does not implement dirty rectangles or partial rasterization.

## Debug Surfaces

Retained render-state debug output reports:

- retained paint key;
- retained paint artifact state;
- last paint artifact action;
- paint artifact reuse/recompute/discard counters.

Render work-plan debug output reports:

- repaint decision;
- repaint scope;
- repaint reasons;
- retained paint artifact state;
- repaint execution strategy;
- conservative fallback when selected.

Frame execution traces may report paint as materialized from retained
artifacts when runtime reused retained paint for a no-op frame.

## Invariants

For equivalent retained runtime state and equivalent invalidation inputs:

- retained paint reuse decisions are deterministic;
- repaint planning is deterministic and debug-visible;
- Browser/runtime may retain and invalidate paint artifacts but must not
  interpret paint semantics;
- Paint owns stacking, ordering, semantic layers, and primitives;
- retained render IDs are not layout `BoxId`, `StackingContextId`, paint
  operation indices, traversal order, or source order;
- layout dirtiness implies paint dirtiness where geometry may affect paint
  output;
- paint-only dirtiness must not trigger relayout when CSS-owned impact
  classification says relayout is unnecessary;
- no-op frames may reuse paint artifacts only when all retained paint key
  inputs still match;
- conservative document or viewport fallback is explicit in debug output.

## Deliberate Exclusions

AC7 deliberately excludes:

- compositor or GPU layers;
- dirty-region rendering;
- partial raster invalidation;
- retained egui/backend draw commands;
- full display-list architecture;
- broad paint dependency graphs;
- browser-owned stacking or paint-order interpretation;
- browser/runtime CSS property-impact tables;
- retained frame-local paint IDs;
- retained `StackingContextId` values;
- retained layout `BoxId` values;
- paint operation indices, traversal order, or source order as retained keys.
