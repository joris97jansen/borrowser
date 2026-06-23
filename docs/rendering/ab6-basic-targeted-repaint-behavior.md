# AB6: Basic Targeted Repaint Behavior

Last updated: 2026-06-17
Status: implemented basic targeted repaint behavior for Milestone AB issue 6

This document defines Borrowser's current repaint execution policy. AB6 turns
AB5's structured paint invalidation scopes into a deterministic runtime repaint
execution decision. The supported execution scopes are deliberately limited to:

- `Document`
- `Viewport`

AB6 does not implement arbitrary dirty rectangles, paint-source-scoped repaint,
stacking-context-scoped repaint, retained display lists, retained paint scenes,
compositor layers, GPU partial raster, or per-node repaint.

Related code:
- `crates/browser/src/rendering/types.rs`
- `crates/browser/src/rendering/invalidation.rs`
- `crates/browser/src/rendering/frame.rs`
- `crates/browser/src/rendering/debug.rs`
- `crates/gfx/src/viewport.rs`
- `crates/browser/src/rendering/tests/invalidation.rs`
- `crates/browser/src/rendering/tests/frame_trace.rs`

Related documents:
- `docs/rendering/ab1-stacking-layering-invalidation-architecture.md`
- `docs/rendering/ab5-structured-paint-invalidation-model.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/rendering/aa8-paint-debug-visual-regression-surface.md`

## Purpose

AB5 records why paint is dirty and which conservative paint invalidation scope
is affected. Before AB6, that scope was not represented as an explicit repaint
execution decision. AB6 adds the missing runtime policy:

```text
PendingRenderWork
  -> PendingPaintInvalidations
  -> PaintInvalidationScope
  -> RepaintExecutionPlan
  -> viewport repaint policy
  -> immediate paint execution
```

This replaces full-document repaint as the only modeled execution behavior. It
does not claim true dirty-region repaint. `Viewport` is a safe narrower
execution scope for cases where AB5 already proves that document-wide repaint
is unnecessary.

## Ownership

Browser/runtime owns repaint scope derivation. It derives
`RepaintExecutionPlan` from `PendingPaintInvalidations::effective_scope()` and
from the existing synthesized `ViewportChanged` frame signal.

Paint owns paint semantics, stacking order, primitive construction, and
immediate paint output. Paint does not inspect invalidation reasons or decide
repaint scope.

GFX/viewport consumes the selected repaint policy. It may apply a viewport
clip for `Viewport` repaint execution, but it must not infer invalidation
causes from DOM, style, layout, paint primitives, or backend draw operations.

## Execution Scope Model

The browser/runtime repaint execution types are:

- `RepaintExecutionScope`
- `RepaintExecutionPlan`
- `RepaintExecutionTrace`

The GFX-facing consumer types are:

- `ViewportRepaintScope`
- `ViewportRepaintPolicy`

The supported mapping is:

| pending paint invalidation scope | synthesized viewport change | repaint execution scope |
| --- | --- | --- |
| `Document` | either | `Document` |
| `Viewport` | either | `Viewport` |
| none | yes | `Viewport` |
| none | no | `Document` |

Mixed invalidations merge through AB5's existing conservative scope
precedence. If any pending paint invalidation requires `Document`, the repaint
execution scope is `Document`.

## Debug Surface

`RenderFrameExecutionTrace` exposes the selected repaint execution scope:

```text
repaint-execution: scope=document
```

or:

```text
repaint-execution: scope=viewport
```

This trace is the deterministic AB6 proof surface. Pixel output, egui command
serialization, GPU state, and backend draw recording are not part of the AB6
contract.

## Invariants

For a fixed sequence of runtime invalidation requests and viewport-change
state:

- repaint execution scope derivation is deterministic;
- browser/runtime owns scope derivation;
- GFX consumes the selected scope without inferring invalidation causes;
- paint does not inspect invalidation reasons;
- `Document` remains the conservative fallback;
- `Viewport` is observably distinct from `Document` through explicit Rust
  types, frame/debug trace output, and tests;
- no frame-local paint, layout, or stacking identifiers are retained as
  invalidation keys.

## Deliberate Exclusions

AB6 deliberately excludes:

- arbitrary dirty rectangles;
- minimal dirty-region propagation;
- paint-source-scoped repaint;
- stacking-context-scoped repaint;
- retained display lists;
- retained paint scenes;
- retained paint artifacts beyond AC7's paint-owned semantic artifact reuse
  contract;
- compositor layers;
- GPU partial raster;
- per-node repaint;
- dependency graphs from DOM/style/layout nodes to paint artifacts;
- pixel or raster snapshot testing.

AC7 adds conservative retained paint-owned semantic artifact reuse and repaint
planning for the existing `Document` and `Viewport` scopes. Future issues may
add narrower repaint behavior only after defining stable ownership,
identifiers, dependency derivation, fallback behavior, and deterministic debug
surfaces.
