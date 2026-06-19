# AC1: Runtime-Owned Retained Render State Contract

Last updated: 2026-06-18
Status: implemented foundational retained render state contract for Milestone AC issue 1

This document defines Borrowser's browser/runtime-owned retained render state
foundation for incremental rendering work.

AC1 does not optimize rendering. It does not introduce retained layout caches,
retained paint caches, targeted relayout, dirty-region rendering, compositor
layers, GPU layers, or broad stable retained identities. It makes the runtime
state that may survive across render updates explicit, typed, inspectable, and
ready for later dirty-state tracking, work planning, and conservative artifact
reuse.

Related code:
- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/page/debug.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/rendering/lifecycle.rs`
- `crates/browser/src/rendering/debug.rs`
- `crates/browser/src/rendering/invalidation.rs`
- `crates/browser/src/rendering/frame.rs`
- `crates/browser/src/rendering/tests/runtime_state.rs`

Related documents:
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/rendering/ab5-structured-paint-invalidation-model.md`
- `docs/rendering/ab6-basic-targeted-repaint-behavior.md`
- `docs/rendering/ab8-stacking-compositing-invalidation-closeout.md`
- `docs/architecture/ARCHITECTURE.md`

## Purpose

Milestone V made Borrowser's eager rendering pipeline explicit. Milestone AB
added paint invalidation and repaint execution foundations without introducing
retained paint scenes or compositor concepts. AC1 starts the next layer:
runtime-owned retained render state across updates.

The foundational retained render state is:

```text
PageState
  -> RetainedRenderState
  -> RenderEpoch
  -> retained style artifacts and runtime invalidation metadata
  -> deterministic retained render-state debug snapshot
```

This state is a browser/runtime lifetime boundary. It is not a transfer of CSS,
layout, or paint semantics into browser/runtime code.

## Ownership Boundaries

Browser/runtime owns:

- retained render state lifetime;
- render epoch advancement;
- document stylesheet attachment state;
- retained style artifact lifecycle in `PageState`;
- runtime invalidation coordination;
- minimal dirty-state placeholders and summaries;
- deterministic retained-state debug summaries.

CSS owns:

- authored CSS parsing;
- selector matching;
- cascade semantics;
- computed value semantics;
- `ResolvedDocumentStyle`, `ComputedDocumentStyle`, and `StyledNode`
  construction.

Layout owns:

- box-tree construction;
- formatting behavior;
- geometry;
- layout order and layout metadata;
- layout debug surfaces.

Paint owns:

- paint semantics;
- stacking-context construction;
- paint ordering;
- semantic paint layers;
- paint primitives;
- paint debug surfaces.

Browser/runtime must not reinterpret CSS, layout, paint, stacking, or paint
ordering semantics while summarizing retained render state.

## Retained Render Epoch

`RenderEpoch` is a typed browser/runtime generation for retained render state.

Initial value:

- `RenderEpoch::initial()` is `0`.
- `PageState::new()` starts at epoch `0`.
- `PageState::start_nav(...)` resets retained render state to epoch `0`.

Advancement:

- DOM invalidation advances the epoch because browser/runtime retained render
  state changed.
- Stylesheet-set invalidation advances the epoch because retained stylesheet
  and style lifecycle state changed.
- Consuming pending style invalidation for recomputation advances the epoch
  before fallible style work continues, even if recomputation later fails.
- Successful retained style artifact recomputation advances the epoch because
  retained resolved/computed style artifacts changed when no pending
  invalidation had already advanced the epoch for that recompute attempt.
- A no-op render/update that only rematerializes frame-local or borrow-backed
  views from already-retained artifacts preserves the epoch.

Meaning:

- The epoch means only: browser/runtime retained render-state generation
  changed.

The epoch deliberately does not mean:

- frame count;
- layout pass count;
- paint pass count;
- cache hit or miss proof;
- artifact reuse proof;
- stable DOM identity;
- stable layout identity;
- stable paint identity;
- stable stacking identity;
- compositor identity.

## Retained And Non-Retained State

AC1 explicitly allows browser/runtime retained state to include:

- render epochs;
- document stylesheet lifecycle state;
- retained resolved/computed style artifacts already covered by V3;
- style and layout dirty placeholders already present in `PageState`;
- style invalidation summaries;
- deterministic debug summaries of artifact lifetime policy.

AC1 explicitly does not retain:

- `StyledNode` trees;
- `LayoutBox` trees;
- `BoxId` values as retained identities;
- traversal or source-order IDs as retained identities;
- `StackingContextId` values as retained identities;
- paint primitive IDs as retained identities;
- display lists;
- retained paint scenes;
- compositor layers;
- GPU resources.

Frame-local layout, paint, traversal, source-order, and stacking identifiers
may appear in their owning subsystem's frame-local debug surfaces. They must
not be presented by browser/runtime retained state as stable retained
identities.

## Debug Surface

`PageState::retained_render_state_debug_snapshot()` returns a
`RetainedRenderStateDebugSnapshot`.

The stable string form begins with:

```text
version: 1
retained-render-state
```

It reports:

- render epoch;
- DOM presence;
- retained/rebuilt artifact lifetime states;
- minimal dirty-state placeholders;
- style invalidation summary;
- explicit `none-frame-local` identity policy for layout, paint, stacking, and
  traversal identities.

The retained render-state debug surface is deterministic. It is an internal
regression contract, not a public API.

## Invariants

For a fixed sequence of browser/runtime invalidations and render updates:

- retained render-state construction is deterministic;
- initial retained state uses epoch `0`;
- navigation reset returns retained state to epoch `0`;
- no-op updates without new invalidation preserve the retained render epoch;
- retained style artifacts are reported only according to the existing V3
  retained-style contract;
- frame-local style, layout, paint, traversal, and stacking artifacts are not
  represented as retained identities;
- browser/runtime owns retained state lifetime and invalidation coordination;
- CSS owns CSS semantics;
- layout owns layout semantics and layout structures;
- paint owns paint semantics, stacking, ordering, layers, and primitives;
- debug summaries use stable field ordering and backend-independent labels.

## Deliberate Exclusions

AC1 deliberately excludes:

- real style cache work beyond the existing retained style cache contract;
- retained layout caches;
- retained paint caches;
- targeted relayout;
- deterministic render work planning;
- full dirty-state tracking;
- dirty-region rendering;
- display-list reuse;
- retained paint scenes;
- compositor or GPU layer models;
- stable retained layout, paint, stacking, traversal, or paint primitive
  identities;
- performance or allocation guardrails for incremental behavior.

Future AC issues may add those concepts only by defining explicit ownership,
stable identifiers where retention is needed, dependency derivation,
conservative fallback behavior, deterministic debug output, and representative
tests.

## Future Extension Points

Future Milestone AC work should extend this foundation through adjacent,
explicit contracts:

- retained dirty-state scopes and reasons;
- deterministic render work plans;
- retained render identities that are distinct from frame-local IDs;
- conservative style artifact reuse summaries;
- conservative layout artifact reuse only after layout ownership and
  invalidation rules are documented;
- conservative paint artifact reuse only after paint ownership and
  invalidation rules are documented;
- measurable incremental behavior guardrails.

Those extensions must preserve the AC1 rule that browser/runtime owns retained
state lifetime, while CSS, layout, and paint continue to own their own
semantics.
