# AB5: Structured Paint Invalidation Model

Last updated: 2026-06-17
Status: implemented structured paint invalidation model for Milestone AB issue 5

This document defines Borrowser's current structured paint invalidation model.
AB5 records why paint is dirty, which conservative paint scope is affected, and
how pending paint invalidations are derived deterministically from runtime
render invalidation requests.

AB5 does not introduce retained display lists, retained paint scenes, compositor
behavior, GPU layer concepts, minimal dirty-rect propagation, backend-specific
partial raster execution, new CSS behavior, new layout behavior, or new paint
ordering behavior.

Related code:
- `crates/browser/src/rendering/types.rs`
- `crates/browser/src/rendering/invalidation.rs`
- `crates/browser/src/rendering/contracts.rs`
- `crates/browser/src/rendering/tests/invalidation.rs`

Related documents:
- `docs/rendering/ab1-stacking-layering-invalidation-architecture.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/rendering/aa9-paint-model-invariants-extension-points.md`
- `docs/rendering/ab4-stacking-context-paint-order.md`
- `docs/rendering/ab6-basic-targeted-repaint-behavior.md`

## Purpose

Before AB5, Borrowser had explicit render invalidation entry points and phase
rerun plans:

```text
RenderInvalidationEntryPoint
  -> RenderInvalidationRequest
  -> RenderWorkPlan
  -> PendingRenderWork
  -> next frame orchestration
```

Those contracts described whether paint reran, but not the paint-specific
reason or repaint scope. AB5 adds that missing layer:

```text
RenderInvalidationRequest
  -> PaintInvalidationRequest
  -> PendingPaintInvalidations
  -> conservative effective paint scope
```

The model is intentionally conservative. It establishes deterministic
invalidation vocabulary and contracts without claiming precision that the engine
does not yet derive.

## Ownership

Browser/runtime owns paint invalidation requests and pending invalidation
collection because it already owns runtime invalidation entry points, queued
render work, and frame scheduling.

CSS owns authored and computed style inputs that can cause paint to become
dirty. CSS does not decide repaint scopes, retained paint artifact lifetime, or
backend repaint execution.

Layout owns geometry, tree order, clip metadata, and layout output that can
cause paint to become dirty. Layout does not queue paint work or decide repaint
execution.

Paint owns paint semantics, paint order, stacking-context ordering, primitive
construction, and immediate paint output. Paint does not retain pending runtime
invalidation state.

GFX/backend executes immediate drawing. It does not decide invalidation
semantics, dirty scopes, retained scene keys, or repaint precision.

## Rust Contract Vocabulary

AB5 introduces these browser/runtime rendering types:

- `PaintInvalidationTrigger`
- `PaintInvalidationReason`
- `PaintInvalidationScope`
- `PaintInvalidationRequest`
- `PendingPaintInvalidations`

The stable contract surfaces are:

- `paint_invalidation_request_contracts()`
- `paint_invalidation_request(...)`
- `RenderInvalidationRequest::paint_invalidation()`
- `PendingRenderWork::paint_invalidations()`
- `PendingPaintInvalidations::effective_scope()`

`PendingPaintInvalidations` is derived from `PendingRenderWork`. It is not an
independent retained paint cache and does not retain frame-local paint or
stacking identifiers.

## Trigger And Reason Model

AB5 maps each paint-rerunning render invalidation entry point to an explicit
paint invalidation request:

| entry point | trigger | reason | scope |
| --- | --- | --- | --- |
| `DocumentReplaced` | `DocumentReplaced` | `ConservativeUnknownImpact` | `Document` |
| `DomStructureChanged` | `DomStructureChanged` | `CascadedFromStyle` | `Document` |
| `DomAttributesChanged` | `DomAttributesChanged` | `CascadedFromStyle` | `Document` |
| `DomTextChanged` | `DomTextChanged` | `CascadedFromLayout` | `Document` |
| `StylesheetSetChanged` | `StylesheetSetChanged` | `CascadedFromStyle` | `Document` |
| `ViewportChanged` | `ViewportChanged` | `CascadedFromLayout` | `Viewport` |
| `ResourceStateChanged` | `ResourceStateChanged` | `DirectPaintDependency` | `Document` |
| `InputStateChanged` | `InputStateChanged` | `RuntimeInputState` | `Viewport` |

The reason describes why paint is dirty, not which lower-level phase happened
to run. For example, `InputStateChanged` can invalidate paint directly without
style or layout invalidation.

## Scope Model

The supported AB5 scopes are:

- `Document`: a conservative full-document paint invalidation.
- `Viewport`: a conservative visible-viewport paint invalidation.

Full repaint is therefore represented explicitly as a conservative scope. It is
not modeled as the absence of invalidation structure.

AB5 deliberately does not expose `PaintSource`, `StackingContext`, or
dirty-region scopes because current stable runtime invalidation does not have
the retained dependency data needed to derive those keys safely. Frame-local
paint and stacking identifiers must not be stored as retained invalidation keys.

## Pending And Merge Behavior

`PendingRenderWork` remains the retained runtime queue. AB5 derives
`PendingPaintInvalidations` from that queue when paint invalidation state is
needed.

The derived collection is deterministic:

- identical paint invalidation requests are deduplicated;
- first-seen order is preserved;
- trigger history is preserved even when a broader effective scope exists;
- `effective_scope()` returns the most conservative scope in the collection.

Scope precedence is:

```text
Document > Viewport
```

This provides a conservative merge result without pretending to compute minimal
dirty rectangles or retained paint dependencies.

## Invariants

For a fixed sequence of render invalidation requests:

- every request that reruns paint has an explicit paint invalidation request;
- paint invalidation mapping is deterministic;
- full-document repaint is represented by `PaintInvalidationScope::Document`;
- viewport repaint is represented by `PaintInvalidationScope::Viewport`;
- pending paint invalidation order is deterministic and deduplicated;
- effective scope calculation is conservative;
- browser/runtime invalidation requests paint work but does not reinterpret
  paint order or stacking semantics;
- paint and GFX/backend do not own pending runtime invalidation state;
- no frame-local paint or stacking identifiers are retained as invalidation
  keys.

## Deliberate Exclusions

AB5 deliberately excludes:

- retained display lists;
- retained paint scenes;
- compositor layers;
- GPU layers or GPU promotion;
- minimal dirty-rect propagation;
- backend-specific partial raster execution;
- paint-source-scoped invalidation;
- stacking-context-scoped invalidation;
- region-level invalidation;
- dependency graphs from DOM/style/layout nodes to paint artifacts;
- new paint ordering behavior;
- new CSS or layout behavior;
- pixel or raster snapshot testing.

Future issues may remove individual exclusions only by defining stable
ownership, deterministic identifiers, dependency derivation, fallback behavior,
and regression surfaces.

AB6 removes the narrow execution-policy exclusion for AB5's supported
`Document` and `Viewport` scopes. It does not remove the exclusions for dirty
regions, retained paint artifacts, paint-source-scoped invalidation, compositor
layers, or GPU behavior.

## Extension Points

Future retained paint or targeted repaint work may extend AB5 by adding:

- stable retained paint artifact identifiers;
- source or stacking-context invalidation only after stable keys exist;
- dirty-region derivation only after real dependency data exists;
- retained scene ownership contracts;
- debug snapshots that prove targeted invalidation is deterministic.

Those extensions must continue deriving from explicit runtime invalidation
entry points or a documented successor contract. They must not bypass
`RenderInvalidationRequest` and `PendingRenderWork` with backend-specific
repaint side effects.
