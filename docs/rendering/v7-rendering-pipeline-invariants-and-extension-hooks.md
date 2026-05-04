# V7: Rendering Pipeline Invariants And Extension Hooks

Last updated: 2026-05-02  
Status: implemented Milestone V close-out contract

This document is the source-of-truth close-out contract for Milestone V. It
records the rendering pipeline invariants, ownership model, retained-versus-
rebuilt assumptions, invalidation boundaries, and explicit future extension
hooks that later milestones must extend deliberately.

Related code:
- `crates/browser/src/rendering.rs`
- `crates/browser/src/page.rs`
- `crates/browser/src/view.rs`
- `crates/css/src/computed/style_tree.rs`
- `crates/layout/src/lib.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/gfx/src/viewport.rs`

Related documents:
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w2-structured-box-tree-data-structures.md`
- `docs/rendering/w3-display-to-box-generation-behavior.md`
- `docs/rendering/w4-anonymous-box-generation-supported-subset.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/w8-box-generation-formatting-debug-surfaces.md`
- `docs/architecture/ARCHITECTURE.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`

## Purpose

Milestone V is complete only if future contributors can answer these questions
without reverse-engineering call ordering:

- what browser/runtime owns
- what engine crates own
- what is retained across updates
- what is rebuilt per frame or on demand
- where invalidation enters the system
- which structured outputs later phases must consume
- where later milestones are allowed to extend the pipeline

V1 through V6 introduced those pieces incrementally. V7 closes the milestone by
making the full rendering architecture explicit as one maintained contract.

## Shipped Code Surfaces

The implemented rendering architecture is pinned by these normative code
surfaces:

- `browser::rendering::render_phase_contracts()`
  Phase ownership, inputs, outputs, retained outputs, and rebuild triggers.
- `browser::rendering::render_artifact_ownership_contracts()`
  Retained-versus-rebuilt artifact lifetimes and retention owners.
- `browser::rendering::render_invalidation_request_contracts()`
  Runtime invalidation entry points and their phase rerun plans.
- `browser::rendering::render_extension_hook_contracts()`
  Deferred rendering hooks that later milestones may extend without
  reinterpreting the current architecture.
- `PageState::render_pipeline_debug_snapshot()`
  Runtime-visible retained-style and downstream rebuilt-state policy.
- `browser::rendering::render_phase_boundary_debug_snapshot(...)`
  Deterministic style/layout/paint/orchestration boundary snapshot.

These are the stable V7 maintenance surfaces. Future rendering work should
extend them or add adjacent explicit contract objects rather than bypassing
them.

## Non-Negotiable Invariants

### Ownership Invariants

- Browser runtime owns DOM lifetime, stylesheet attachment order, retained
  style lifecycle state, invalidation requests, queued render work, and frame
  scheduling/orchestration state.
- `crates/css` owns style semantics, including parsing handoff consumption,
  selector matching, cascade, computed-style assembly, and styled-tree
  construction.
- `layout` owns box generation, box-tree structure, formatting-context
  assignment, containing-block relationships, and geometry semantics.
- `gfx::paint` owns translating geometry plus runtime paint state into the
  current frame's visual output only.
- Later phases must not reach backward and re-own earlier-phase semantics.

### Retained / Rebuilt Invariants

- Retained today:
  `Dom`, `StylesheetSet`, `ResolvedDocumentStyle`,
  `ComputedDocumentStyle`, `ResourceState`, and `InputState`.
- Rebuilt today:
  `StyledTree`, `LayoutTree`, and `PaintCommands`.
- Borrow-backed `StyledNode` and frame-local `LayoutBox` trees must not be
  retained self-referentially inside `PageState`.
- A future retained layout or paint cache must become a new explicit owner
  contract, not an accidental consequence of keeping old frame-local objects
  alive longer.

### Invalidation And Scheduling Invariants

- Rendering work enters only through explicit
  `RenderInvalidationEntryPoint -> RenderInvalidationRequest -> PendingRenderWork`
  boundaries.
- `PendingRenderWork` is consumed on the next orchestrated frame attempt, not
  only after successful paint.
- `ViewportChanged` remains an explicit in-frame trigger synthesized by runtime
  orchestration.
- Text mutation currently invalidates layout without invalidating computed style
  unless stylesheet reconciliation or future selector/property features make
  text content style-relevant.

### Phase Handoff Invariants

The semantic render handoff order is:

```text
StylePhaseOutput
  -> LayoutPhaseInput
  -> LayoutPhaseOutput
  -> PaintPhaseInput
  -> RenderFrameExecutionTrace
```

The rules for that chain are:

- `StylePhaseOutput` is the only semantic style-to-layout handoff.
- `LayoutPhaseInput` forwards the chosen styled root and explicit layout
  environment inputs without reinterpretation.
- `LayoutPhaseOutput` is the authoritative geometry handoff to paint and input
  routing.
- `PaintPhaseInput` is semantic layout-to-paint input; `PaintArgs` remains
  backend/runtime execution context only.
- Debug surfaces must remain semantic and deterministic rather than backend or
  pixel specific.

## Explicit Future Extension Hooks

Milestone V7 exposes these deferred hooks through
`browser::rendering::render_extension_hook_contracts()`:

| hook | integration owner | extends | contract rule |
| --- | --- | --- | --- |
| `BoxTreeFormalization` | `LayoutEngine` | `Layout`, `Paint`; `StyledTree`, `LayoutTree` | may continue expanding the box-tree representation, but must preserve typed style-to-layout and layout-to-paint handoffs |
| `ConstraintSizingAndIntrinsicLayout` | `LayoutEngine` | `Style`, `Layout`; styled-tree, viewport, text, replaced metadata, layout tree | may add sizing/constraint algorithms, but layout must continue consuming computed values plus explicit environment inputs |
| `PaintPrimitiveAndDisplayListExpansion` | `PaintEngine` | `Paint`, `FrameOrchestration`; layout/resource/input/paint artifacts | may expand paint output into richer scene/display-list forms without re-owning layout semantics |
| `IncrementalInvalidationAndDependencyTracking` | `BrowserRuntime` | all phases; retained/computed/styled/layout/paint artifacts | may refine rerun planning only through explicit invalidation entry points and work plans |
| `RetainedLayoutState` | `BrowserRuntime` | `Layout`, `Paint`, `FrameOrchestration`; viewport/text/replaced/layout artifacts | may introduce retained layout caches only as explicit runtime-owned retained state, not as borrowed `LayoutBox` retention |
| `RetainedPaintSceneState` | `BrowserRuntime` | `Paint`, `FrameOrchestration`; layout/resource/input/paint artifacts | may introduce retained display-list/scene caches only with explicit invalidation and ownership boundaries |
| `RuntimeFrameSchedulingIncrementality` | `BrowserRuntime` | `Layout`, `Paint`, `FrameOrchestration`; viewport/resource/input/layout/paint artifacts | may add selective scheduling or async scene work, but must preserve queued invalidation contracts and deterministic traces |

These hooks are deliberately named and scoped so later milestones can grow the
pipeline without silently changing what the current tables mean.

## Deferred Beyond Milestone V

Milestone V intentionally does not ship:

- a formal retained box-tree cache
- block/inline sizing completeness beyond the current baseline
- advanced paint primitives or stacking-context/layer architecture
- retained display lists or scene graphs
- targeted dependency-driven invalidation
- resource dependency graphs
- compositor scheduling
- async frame production

Those are now explicit extension hooks, not missing architecture.

## Alignment With Tests

The repository now pins the Milestone V architecture through a layered test
surface:

- `render_phase_contracts_pin_expected_phase_boundaries()`
- `render_artifact_ownership_contracts_cover_each_artifact_once()`
- `phase_contract_outputs_align_with_artifact_lifetimes()`
- `render_invalidation_request_contracts_cover_each_entry_point_once()`
- `direct_invalidation_phase_sources_align_with_phase_rebuild_triggers()`
- `frame_execution_trace_distinguishes_requested_work_from_frame_prerequisites()`
- `style_to_layout_handoff_uses_explicit_phase_output_models()`
- `layout_to_paint_handoff_wraps_layout_phase_output_without_reinterpretation()`
- `render_phase_boundary_debug_snapshot_is_stable_for_simple_text_flow()`
- `render_phase_boundary_debug_snapshot_is_stable_for_replaced_element_flow()`
- `render_phase_boundary_debug_snapshot_preserves_non_zero_dom_identity_across_handoffs()`
- `render_extension_hook_contracts_cover_expected_future_work_once()`
- `render_extension_hook_contracts_anchor_deferred_work_to_current_pipeline()`

This is the V7 alignment rule: architectural prose, stable contract tables,
and regression tests must continue describing the same pipeline.

## Milestone Completion

Milestone V is complete while these conditions hold:

- runtime rendering uses the explicit orchestration path
- phase boundaries stay typed and deterministic
- retained-versus-rebuilt ownership stays explicit
- invalidation enters only through named entry points and work plans
- debug surfaces remain semantic and deterministic
- later rendering work integrates through named extension hooks instead of
  bypassing the contract tables

Future contributors should treat this document and the associated code tables as
the rendering equivalent of a parser or CSS contract: extend deliberately,
never by accidental eager rebuild behavior.
