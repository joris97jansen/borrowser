# AC4: Deterministic Render Work Plans

Last updated: 2026-06-23
Status: implemented deterministic browser/runtime render work planning for
Milestone AC issue 4

This document defines Borrowser's browser/runtime-owned render work planner.
AC4 derives an explicit, deterministic plan for style, layout, and paint work
before those phases execute.

AC4 does not introduce retained layout caches, retained paint caches, retained
display lists, retained paint scenes, targeted relayout, dirty-region
rendering, compositor layers, GPU concepts, backend partial repaint, or a
browser-owned CSS property-impact table.

Related code:

- `crates/browser/src/rendering/work_plan.rs`
- `crates/browser/src/rendering/invalidation.rs`
- `crates/browser/src/rendering/frame.rs`
- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/page/mod.rs`
- `crates/browser/src/rendering/tests/work_plan.rs`

Related documents:

- `docs/rendering/ac1-retained-render-state-runtime-contract.md`
- `docs/rendering/ac2-retained-render-identities.md`
- `docs/rendering/ac3-explicit-dirty-state-tracking.md`
- `docs/rendering/ac5-retained-style-artifact-reuse.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/ab5-structured-paint-invalidation-model.md`
- `docs/rendering/ab7-deterministic-debug-regression-coverage.md`

## Purpose

AC3 made retained dirty state explicit. AC4 adds the next browser/runtime
boundary:

```text
retained render state
  + retained dirty state
  + pending invalidation work
  -> canonical dirty state for planning
  -> RenderWorkPlan
  -> style/layout/paint execution
```

The plan explains which work the runtime intends to perform before that work
is performed. It is a planning contract, not an execution trace and not proof
that layout or paint artifacts were reused.

## Ownership

Browser/runtime owns:

- render work-plan derivation;
- the canonical dirty-state view used for planning;
- retained style artifact status exposed through typed retained-state queries;
- deterministic work-plan debug output;
- conservative fallback selection when dirty input contains unknown impact.

CSS owns:

- authored CSS parsing;
- selector matching;
- cascade and computed-style semantics;
- any future property-impact classification.

Layout owns:

- box-tree generation;
- formatting behavior;
- geometry;
- layout dependency knowledge.

Paint owns:

- paint semantics;
- stacking-context construction;
- paint ordering;
- paint primitives and paint-side debug surfaces.

Browser/runtime must not inspect CSS declarations, infer CSS property impact,
derive layout geometry, infer paint ordering, or inspect paint primitives while
planning work.

## Vocabulary

AC4 introduces:

- `RenderWorkPlan`
- `PlannedRenderWork`
- `RenderWorkDecision`
- `RenderWorkPlanReason`
- `RenderWorkFallbackReason`
- `RenderWorkPlanInput`
- `RetainedStyleArtifactState`

The existing static invalidation request plan was renamed to
`RenderInvalidationWorkPlan`. It remains part of
`RenderInvalidationRequest` and describes what a runtime entry point requests.
It is not the derived AC4 plan.

## Inputs

The planner consumes typed browser/runtime inputs:

- DOM presence;
- retained dirty state from `RetainedRenderState`;
- pending invalidation work from `PendingRenderWork`;
- retained style artifact state from the existing retained style/cache
  contract.

The planner does not consume `RenderPipelineDebugSnapshot`,
`RetainedRenderStateDebugSnapshot`, or any other debug snapshot. Debug
snapshots are output-only regression surfaces.

AC4 may mention retained style artifacts because retained resolved/computed
style artifacts already exist. It must not invent retained layout-tree,
display-list, paint-scene, or paint-command freshness.

## Canonical Dirty State

Work-plan derivation uses one deterministic dirty-state view:

1. start with retained dirty entries from `RetainedRenderState`;
2. extend with dirty entries derived from `PendingRenderWork`;
3. use `RenderDirtyState`'s existing phase/reason deduplication, conservative
   scope merge, and stable ordering.

This rule lets page-owned retained dirtiness and runtime-owned pending
invalidation contribute to planning without creating two competing sources of
truth. Equivalent retained dirty state and equivalent pending work produce the
same canonical planning input.

## Decisions

The plan reports three phase decisions:

- `restyle`;
- `relayout`;
- `repaint`.

Style planning may report retained style reuse only when retained style
artifacts are fresh and style is not dirty. Layout and paint planning report
whether relayout or repaint is planned from dirty state. They do not report
retained layout or retained paint reuse because those artifacts are not
retained yet.

`ConservativeUnknownImpact` in the canonical dirty state produces an explicit
`conservative-fallback` entry in the plan. Full-document or viewport fallback
is represented as a visible scope, not hidden behind generic wording.

## Relationship To Execution Traces

`RenderWorkPlan` is derived before style/layout/paint execution.

`RenderFrameExecutionTrace` remains the executed-frame trace. It records work
that was requested, materialized from retained artifacts, or required for the
current frame attempt.

The distinction is intentional:

- invalidation request contracts say what a runtime entry point requests;
- retained dirty state says what is dirty and why;
- render work plans say what the runtime plans before execution;
- frame execution traces say what happened during the frame attempt.

Current Borrowser still rebuilds frame-local layout and paint artifacts when a
page frame executes. AC4 does not change that baseline and does not claim
layout or paint artifact reuse.

## Debug Surface

`RenderWorkPlan::to_debug_snapshot()` emits a deterministic snapshot:

```text
version: 1
render-work-plan
entry-points: ...
canonical-dirty-state:
  entries: ...
restyle: decision=... scope=...
relayout: decision=... scope=...
repaint: decision=... scope=...
conservative-fallback: ...
```

The snapshot is an internal regression contract, not a public API.

## Invariants

For equivalent retained state and pending invalidation input:

- work-plan derivation is deterministic;
- debug output is deterministic;
- the planner is read-only and does not clear dirty state;
- viewport dirty state does not imply restyle;
- paint-only runtime dirty state does not imply relayout;
- conservative unknown impact remains visible in the plan;
- retained style reuse is reported only through existing retained style
  artifacts;
- frame-local layout, paint, stacking, traversal, and paint-operation IDs are
  not used as retained planning keys.

## Deliberate Exclusions

AC4 deliberately excludes:

- style artifact lifecycle accounting and conservative retained artifact reuse
  beyond pre-execution planning; AC5 defines that execution-side contract;
- retained layout caches;
- retained paint caches;
- retained display lists or scenes;
- targeted relayout;
- dirty-region rendering;
- compositor layers or GPU concepts;
- backend partial repaint;
- browser-owned CSS property-impact classification;
- dependency graphs from DOM/style/layout nodes to retained paint artifacts.

Future AC issues may add layout or paint artifact reuse only after defining
ownership, retained identifiers, dependency derivation, conservative fallback
behavior, deterministic debug output, and tests. AC5 adds the first
conservative retained style artifact reuse path for resolved/computed style
artifacts only.
