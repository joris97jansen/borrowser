# AC9: Incremental Rendering Performance Guardrails

Status: implemented deterministic retained rendering performance/resource
guardrails for Milestone AC issue 9

This document defines Borrowser's first performance and resource guardrail
contract for retained incremental rendering.

AC9 does not introduce renderer micro-optimizations, dirty-region rendering,
compositor layers, GPU concepts, retained backend draw commands, retained paint
scenes, new rendering semantics, or browser-owned CSS property-impact tables.
It proves that the retained runtime model has measurable, bounded behavior in
representative scenarios.

Related code:

- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/rendering/lifecycle.rs`
- `crates/browser/src/rendering/work_plan.rs`
- `crates/browser/src/rendering/frame.rs`
- `crates/browser/src/rendering/debug.rs`
- `crates/browser/src/rendering/tests/perf_guards.rs`

Related documents:

- `docs/rendering/ac1-retained-render-state-runtime-contract.md`
- `docs/rendering/ac2-retained-render-identities.md`
- `docs/rendering/ac3-explicit-dirty-state-tracking.md`
- `docs/rendering/ac4-deterministic-render-work-plans.md`
- `docs/rendering/ac5-retained-style-artifact-reuse.md`
- `docs/rendering/ac6-retained-layout-artifact-foundation.md`
- `docs/rendering/ac7-retained-paint-artifact-reuse-repaint-planning.md`
- `docs/rendering/ac8-incremental-rendering-debug-snapshots.md`

## Purpose

AC9 turns the retained rendering debug and lifecycle state from AC1 through AC8
into regression guardrails. The guardrails answer whether representative
updates:

- reuse retained artifacts on no-op frames;
- avoid style recomputation for viewport-only and text-only updates;
- avoid relayout for CSS-owned paint-only style changes when classification
  supports that;
- recompute layout and paint for layout-affecting updates;
- clean retained dirty state after full frame execution and recording;
- avoid unbounded retained-state growth across repeated updates.

These guardrails are deliberately counter-based. They avoid CI assertions based
on wall-clock timing because normal hardware, scheduler, and profile
differences make timing thresholds brittle.

## Measurement Surface

AC9 uses existing deterministic browser/runtime debug surfaces:

- `RetainedStyleArtifactStats`
- `RetainedLayoutArtifactStats`
- `RetainedPaintArtifactStats`
- retained dirty-state entry counts
- retained identity counts
- retained artifact states and last actions
- `RenderFrameExecutionTrace` for actual reuse/materialization decisions

The tests compare baseline and post-update snapshots. They prefer coarse
properties such as "style recomputation did not increase" or "layout
recomputation is bounded by the number of viewport updates" over exact
operation counts unless the earlier retained rendering contracts already make
the behavior explicit.

## Covered Scenarios

The AC9 guard tests cover:

- initial render baseline;
- no-op repeated render;
- repeated viewport resize;
- text/content update;
- paint-only style update using CSS-owned impact classification;
- layout-affecting style update;
- stylesheet/global style update;
- representative deterministic in-repo page update.

The representative page fixture is generated in the test module. It is not a
live website, does not require network access, and is intended to exercise
repeated app-like markup without becoming a benchmark suite for one page.

## Resource Guardrails

AC9 treats retained-state growth as the CI-safe resource guardrail:

- retained dirty entries must be cleaned after a fully prepared, executed, and
  recorded frame;
- retained identity counts must not grow for non-structural repeated updates;
- retained style/layout/paint caches must remain fresh after recorded frames;
- recompute counters must stay bounded by the number and kind of updates.

Heap-byte allocation measurement is not part of AC9's default CI proof. The
workspace has opt-in allocation guards in other crates, but AC9 does not add a
new global allocator hook for browser rendering. Future work may add isolated
browser allocation guards if a clean crate-local pattern exists.

## Ownership

Browser/runtime owns:

- retained artifact lifetime counters;
- retained dirty-state cleanup checks;
- retained identity/resource growth checks;
- deterministic guard tests and documentation.

CSS owns:

- property parsing and computed values;
- selector/cascade behavior;
- computed-style impact classification used to distinguish paint-only and
  layout-affecting updates.

Layout owns:

- layout geometry;
- retained layout artifact materialization and keys;
- fallback behavior when retained layout cannot be reused.

Paint owns:

- semantic paint artifact construction;
- paint primitives, stacking, ordering, and paint-owned debug meaning.

## Invariants

- Guardrails observe rendering behavior; they do not drive rendering behavior.
- No-op frames must avoid retained style/layout/paint recomputation when
  retained artifact keys still match.
- Viewport-only updates must not restyle by default.
- Text-only updates must not restyle in the currently supported CSS model.
- Paint-only style updates must not relayout when CSS-owned impact
  classification safely narrows the impact.
- Layout-affecting style and stylesheet updates must visibly recompute the
  necessary downstream work.
- Full frame execution and retained result recording must leave retained dirty
  state clean.
- Repeated non-structural updates must not grow retained identity state.

## Deliberate Exclusions

AC9 deliberately excludes:

- wall-clock CI benchmark thresholds;
- production-grade optimization goals;
- heap-byte allocation assertions for browser rendering;
- dirty-region rendering;
- true minimal/subtree relayout execution;
- compositor or GPU layers;
- retained backend draw commands;
- retained display lists or paint scenes;
- browser-owned CSS property-impact classification;
- live-page or network-backed benchmark fixtures.
