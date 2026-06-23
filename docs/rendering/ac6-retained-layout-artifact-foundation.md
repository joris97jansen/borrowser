# AC6: Retained Layout Artifact Foundation

Status: implemented retained layout artifact foundation for Milestone AC issue 6

This document defines Borrowser's first retained layout artifact reuse path and
explicit targeted relayout boundaries.

AC6 does not implement true minimal subtree relayout. It represents requested
relayout scope separately from the execution strategy, and it uses visible
document relayout fallbacks when Layout cannot yet execute a narrower relayout
safely.

Related code:

- `crates/css/src/computed/impact.rs`
- `crates/layout/src/retained.rs`
- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/rendering/work_plan.rs`
- `crates/browser/src/rendering/lifecycle.rs`
- `crates/gfx/src/viewport.rs`

Related documents:

- `docs/rendering/ac1-retained-render-state-runtime-contract.md`
- `docs/rendering/ac2-retained-render-identities.md`
- `docs/rendering/ac3-explicit-dirty-state-tracking.md`
- `docs/rendering/ac4-deterministic-render-work-plans.md`
- `docs/rendering/ac5-retained-style-artifact-reuse.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`

## Ownership

Browser/runtime owns:

- retained layout artifact lifetime in page retained state;
- retained layout cache keys;
- layout dirty state and layout reuse planning;
- reuse/recompute/discard counters;
- deterministic retained layout debug output.

Layout owns:

- box-tree construction;
- formatting behavior;
- geometry;
- layout order and metadata;
- retained layout artifact construction and materialization.

CSS owns style-impact classification. Browser/runtime consumes the CSS-owned
classification result and must not maintain its own CSS property-impact table.

Paint consumes the current frame's `LayoutPhaseOutput`. Paint does not own the
retained layout artifact and does not infer layout reuse.

## Retained Layout Artifact

The retained layout artifact is `layout::RetainedLayoutArtifact`. It is owned
data. It does not store:

- `LayoutBox<'_>`;
- `StyledNode<'_>`;
- `&ComputedStyle`;
- frame-local `BoxId` as a retained identity.

The artifact stores copied layout-owned geometry and metadata plus DOM-source
provenance anchors needed to materialize a current-frame `LayoutPhaseOutput`.
Materialization reattaches current `StyledNode` and `ComputedStyle` references
from the current style tree. If anchors cannot be found, materialization fails
and the runtime falls back to document relayout.

Artifact-local layout box ordinals inside the artifact are private
materialization structure owned by Layout. They may be copied from frame-local
layout box order and converted back into frame-local layout IDs while
materializing the current frame's `LayoutPhaseOutput`, but they are not stable
retained identities, not AC2 retained render identities, and not a competing
retained layout identity domain.

## Cache Key

Retained layout artifacts are keyed by:

```text
RetainedLayoutKey {
  identity_domain,
  layout_input_generation,
  layout_style_generation,
  viewport_width,
  text_measurement_generation,
  replaced_metadata_generation,
}
```

`identity_domain` comes from the AC2 retained render identity domain and
prevents reuse across full document replacement.

`layout_input_generation` changes for DOM structure and text changes that can
affect layout.

`layout_style_generation` changes only when CSS-owned computed-style impact
classification says style changed in a layout-affecting or unknown way.

`viewport_width` is a quantized viewport-width key. Viewport width changes
invalidate layout without implying restyle by default.

`text_measurement_generation` and `replaced_metadata_generation` reserve
explicit layout-input boundaries for text measurement and replaced content
metadata. Resource changes conservatively advance replaced metadata.

`RenderEpoch` is deliberately not part of the layout cache key.

## Reuse Rules

Retained layout may be reused only when:

- a retained layout artifact exists;
- the retained layout key matches the current layout key;
- retained layout dirty state is clean;
- layout materialization succeeds against the current style tree.

No-op frames can reuse retained style and retained layout artifacts.

Paint-only style changes can recompute retained style artifacts while
preserving retained layout. In that case the style phase rebuilds the
borrow-backed `StyledNode` view, retained layout materialization attaches the
current style references, and paint sees current paint-only style values.

Unknown style impact, layout-affecting style changes, text changes, DOM
structure changes, resource changes, and viewport changes mark layout dirty or
change the layout key, so retained layout is not reused.

## Relayout Scope And Execution

`RenderWorkPlan` records both:

- requested relayout scope, through the layout dirty scope;
- relayout execution strategy, through `RelayoutExecution`.

AC6 can represent targeted scopes such as `Viewport`, AC2 retained render node,
retained artifact, or retained subtree scopes. It does not execute true
targeted subtree relayout yet.

When requested scope is narrower than document but Layout cannot execute that
scope safely, the plan reports:

```text
relayout-execution: strategy=conservative-document-fallback
```

with the requested scope and fallback reason. This is a real document relayout
fallback, not a claim of minimal relayout.

## Debug Output

Retained render-state debug output includes:

- retained layout key seed;
- retained layout key;
- retained layout artifact state;
- last layout artifact action;
- reuse/recompute/discard counters.

Render work-plan debug output includes:

- relayout decision;
- requested relayout scope;
- relayout execution strategy;
- conservative fallback reason when a narrower scope executes as document
  relayout.

Frame execution traces may report layout as `materialized-from-retained-artifacts`
when retained layout was actually reused for the frame.

## Invariants

For a fixed DOM, retained style artifacts, viewport width, text measurer,
replaced metadata, and layout dirty state:

- retained layout key construction is deterministic;
- retained layout reuse is deterministic;
- layout materialization either succeeds with current-frame references or
  fails visibly;
- Browser/runtime owns retained lifetime and counters;
- Layout owns geometry, formatting, and materialization semantics;
- CSS owns style-impact classification;
- frame-local `BoxId` values are not retained identities;
- artifact-local layout box ordinals are materialization structure only;
- DOM IDs are provenance anchors only;
- true minimal/subtree relayout remains unsupported.

## Deliberate Exclusions

AC6 does not introduce:

- true minimal/subtree relayout execution;
- layout dependency graphs;
- dirty-region rendering;
- retained display lists or paint scenes;
- compositor layers or GPU resources;
- browser-owned CSS property-impact tables;
- retained paint artifact reuse; AC7 adds that later as a separate
  browser/runtime-retained, paint-owned semantic artifact contract.
