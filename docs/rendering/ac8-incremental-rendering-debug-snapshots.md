# AC8: Incremental Rendering Debug Snapshots

Status: implemented deterministic incremental rendering debug snapshots for
Milestone AC issue 8

This document defines Borrowser's AC8 debug and regression contract for
retained and incremental rendering observability.

AC8 does not introduce new rendering optimizations, dirty-region rendering,
compositor layers, GPU concepts, retained display lists, retained paint
scenes, new artifact reuse semantics, or new CSS/Layout/Paint dependency
classification. It hardens existing debug surfaces so retained rendering
decisions can be reviewed from deterministic text.

Related code:

- `crates/browser/src/rendering/lifecycle.rs`
- `crates/browser/src/rendering/work_plan.rs`
- `crates/browser/src/rendering/debug.rs`
- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/page/debug.rs`
- `crates/browser/src/rendering/tests/runtime_state.rs`
- `crates/browser/src/rendering/tests/work_plan.rs`
- `crates/browser/src/rendering/tests/frame_trace.rs`

Related documents:

- `docs/rendering/ac1-retained-render-state-runtime-contract.md`
- `docs/rendering/ac2-retained-render-identities.md`
- `docs/rendering/ac3-explicit-dirty-state-tracking.md`
- `docs/rendering/ac4-deterministic-render-work-plans.md`
- `docs/rendering/ac5-retained-style-artifact-reuse.md`
- `docs/rendering/ac6-retained-layout-artifact-foundation.md`
- `docs/rendering/ac7-retained-paint-artifact-reuse-repaint-planning.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/ab7-deterministic-debug-regression-coverage.md`

## Purpose

Milestone AC made retained runtime state, retained identities, dirty state,
work planning, and conservative style/layout/paint artifact reuse explicit.
AC8 makes those decisions inspectable as a stable regression surface.

The debug contract answers:

- what retained runtime state exists;
- which style, layout, and paint dirty entries are active;
- which retained generations and keys explain artifact freshness;
- which work the runtime plans before execution;
- which conservative fallback reasons were selected;
- which style, layout, and paint artifact lifecycle actions actually happened;
- which identities are retained and which remain frame-local by policy.

## Ownership

Browser/runtime owns:

- retained render-state debug output;
- dirty-state debug output;
- render work-plan debug output;
- retained artifact keys, generations, lifecycle counters, and last actions;
- frame execution traces for runtime orchestration decisions.

CSS owns CSS parsing, selector matching, cascade, computed values, and
computed-style impact facts. Browser/runtime may report CSS-owned impact facts
after CSS exposes them, but it must not duplicate CSS property semantics.

Layout owns geometry, formatting behavior, layout metadata, and retained layout
artifact materialization. Browser/runtime reports layout artifact keys,
actions, and fallback results; it must not infer layout geometry from debug
output.

Paint owns paint artifact semantics, stacking contexts, paint ordering,
semantic layers, primitives, and paint-owned debug surfaces. Browser/runtime
reports retained paint artifact lifetime and repaint planning; it must not
reinterpret paint primitives or backend output.

## Snapshot Surfaces

`PageState::retained_render_state_debug_snapshot()` returns
`RetainedRenderStateDebugSnapshot`. Its stable string form reports:

- render epoch;
- retained/rebuilt artifact policy;
- dirty entries, dirty booleans, and style invalidation state;
- retained DOM/style/layout/paint generations;
- retained style, layout, and paint artifact keys where present;
- artifact state, last action, reuse count, recompute count, and discard count;
- retained render identity domain and retained DOM-backed render identities;
- explicit non-retention policy for frame-local layout, paint, stacking, and
  traversal/source-order identities.

`RenderWorkPlan::to_debug_snapshot()` reports pre-execution intent:

- pending invalidation entry points;
- canonical dirty state after retained and pending dirty inputs are merged;
- restyle, relayout, and repaint decisions;
- requested dirty scopes;
- relayout and repaint execution strategies;
- conservative fallback reasons.

`RenderFrameExecutionTrace::to_debug_snapshot()` reports executed frame
orchestration:

- triggered entry points;
- which phases were requested;
- which phases were materialized from retained artifacts;
- repaint execution scope;
- semantic phase order.

Work plans are not execution traces. A work plan may say reuse is intended, but
actual reuse/recompute/discard summaries come only from retained artifact
lifecycle state recorded during style, layout, and paint execution paths.

## Generations And Epochs

`RenderEpoch` reports that browser/runtime retained render state changed. It is
not a frame count, cache-hit proof, layout pass count, paint pass count, or
stable layout/paint identity.

The retained generation block reports browser/runtime generation counters used
to explain retained artifact validity:

- `dom-generation`
- `style-input-generation`
- `stylesheet-generation`
- `layout-input-generation`
- `layout-style-generation`
- `paint-style-generation`
- `paint-input-generation`
- `text-measurement-generation`
- `replaced-metadata-generation`

These fields make reuse, recompute, stale state, discard, and fallback
decisions reviewable even when a retained key is absent.

## Determinism Rules

For equivalent retained state and equivalent invalidation inputs:

- debug output is byte-stable;
- dirty entries use deterministic ordering;
- retained identities use deterministic retained ID allocation and DOM anchor
  reporting;
- artifact summaries use typed lifecycle counters and actions;
- fallback reasons use typed enum values;
- no hash-map iteration order or memory address is exposed.

Retained render-state snapshots must not expose frame-local layout `BoxId`
values, paint operation IDs, stacking context IDs, traversal IDs,
source-order IDs, backend resource handles, memory addresses, or any other
non-retained identity as a stable retained identity.

## Invariants

- Debug output observes runtime state and execution decisions; it never drives
  rendering behavior.
- Browser/runtime remains the owner of retained state, dirty state, planning,
  artifact lifetime, keys, counters, and debug output.
- CSS, Layout, and Paint ownership boundaries remain intact.
- Conservative fallback reasons are explicit in work-plan snapshots.
- Artifact reuse summaries are based on actual lifecycle and execution
  outcomes, not only on planned work.
- Frame-local identities remain explicitly non-retained in browser/runtime
  retained-state snapshots.

## Deliberate Exclusions

AC8 deliberately excludes:

- dirty-region rendering;
- compositor layers or GPU concepts;
- retained display lists or retained paint scenes;
- backend command serialization;
- new artifact reuse semantics;
- new CSS property support;
- browser-owned CSS property-impact tables;
- layout or paint dependency graphs;
- true minimal/subtree relayout execution;
- performance or allocation guardrails.
