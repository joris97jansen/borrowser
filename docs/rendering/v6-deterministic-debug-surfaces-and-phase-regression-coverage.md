# V6: Deterministic Debug Surfaces And Phase Regression Coverage

Last updated: 2026-05-01  
Status: implemented deterministic phase-boundary debug surfaces for Milestone V issue 6

This document is the source-of-truth contract for Milestone V6. It defines the
stable debug output and regression fixtures that pin Borrowser's rendering
pipeline boundaries, phase handoffs, and orchestration decisions.

Related code:
- `crates/css/src/computed/style.rs`
- `crates/css/src/computed/style_tree.rs`
- `crates/layout/src/lib.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/browser/src/rendering.rs`

Related documents:
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
- `docs/architecture/ARCHITECTURE.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`

## Purpose

V1 defined ownership. V2 defined typed phase outputs. V3 defined retained
versus rebuilt ownership. V4 defined invalidation entry points. V5 routed the
runtime through the explicit orchestration path.

V6 makes those contracts inspectable and regression-testable without relying on
pixel snapshots or backend-specific paint output. The goal is deterministic
boundary visibility, not broader rendering capability.

## Shipped Debug Surfaces

Borrowser now exposes stable phase-boundary serialization through:

- `ComputedStyle::to_boundary_debug_label()`
- `StylePhaseOutput::to_debug_snapshot()`
- `LayoutPhaseInput::to_debug_snapshot()`
- `LayoutPhaseOutput::to_debug_snapshot()`
- `PaintPhaseInput::to_debug_snapshot()`
- `RenderFrameExecutionTrace::to_debug_snapshot()`
- `RenderPhaseBoundaryDebugSnapshot::to_debug_snapshot()`
- `browser::rendering::render_phase_boundary_debug_snapshot(...)`

These surfaces are the normative V6 debug contract for:

- style outputs
- layout inputs and outputs
- paint inputs
- orchestration decisions for a frame attempt

The browser-level composed surface is
`render_phase_boundary_debug_snapshot(...)`. It captures one deterministic
phase-boundary flow without depending on egui paint command emission:

```text
StylePhaseOutput
  -> LayoutPhaseInput
  -> LayoutPhaseOutput
  -> PaintPhaseInput
  -> RenderFrameExecutionTrace
```

## Deterministic Serialization Rules

The shipped debug output follows these rules:

- every snapshot begins with `version: 1`
- every snapshot declares its semantic object kind on the next line
- tree-shaped outputs use preorder traversal
- node/box indentation is structural, not incidental
- text and comment content are escaped through Rust's deterministic
  `escape_default()` representation
- floating-point layout values are rounded to fixed decimal precision
- orchestration traces serialize entry points and trigger reasons in
  first-seen deterministic order
- phase order is serialized semantically as `style -> layout -> paint`
- snapshots describe semantic handoff state, not backend-specific paint
  command buffers

This means the debug surfaces may evolve only through explicit contract
changes. Formatting drift is a regression.

## Representative Regression Fixtures

The repository now pins representative boundary flows through exact snapshot
fixtures:

- `render_phase_boundary_debug_snapshot_is_stable_for_simple_text_flow()`
- `render_phase_boundary_debug_snapshot_is_stable_for_replaced_element_flow()`

Those exact fixtures intentionally use parser-backed DOMs that currently expose
`Id(0)` identities in this baseline path. They remain shape/style/layout and
orchestration fixtures, not the only identity-preservation coverage.

These fixtures intentionally cover two different rendering baselines:

### Simple Text Flow

The simple text fixture exercises:

- document replacement
- stylesheet-set invalidation
- fresh style recomputation
- text-bearing layout boxes
- paint input built from ordinary inline text content

This proves the serialized style, layout, paint, and orchestration surfaces
stay aligned for a basic document flow.

### Replaced Element Flow

The replaced-element fixture exercises:

- resource-driven invalidation
- input-driven paint invalidation
- in-frame viewport change handling
- replaced-element intrinsic sizing metadata
- style materialization from retained artifacts without a fresh style request

This proves the boundary snapshots can distinguish:

- semantic layout/paint reruns requested by invalidation
- style output rebuilt from retained computed artifacts
- viewport/runtime orchestration triggers that did not originate in the CSS
  engine

## Additional Structural Contract Coverage

V6 builds on the earlier structural tests rather than replacing them:

- `style_to_layout_handoff_uses_explicit_phase_output_models()`
- `layout_to_paint_handoff_wraps_layout_phase_output_without_reinterpretation()`
- `frame_execution_trace_distinguishes_requested_work_from_frame_prerequisites()`
- `frame_execution_trace_adds_viewport_change_as_direct_runtime_trigger()`
- `render_phase_boundary_debug_snapshot_preserves_non_zero_dom_identity_across_handoffs()`

Taken together, these tests pin:

- the existence of the typed handoff objects
- the exact object forwarding semantics between phases
- the deterministic orchestration classification for current-frame execution
- the representative serialized fixtures that later layout/paint work must keep
  compatible or update deliberately

## Current Scope

V6 intentionally captures semantic phase boundaries, not final raster output.

The current debug fixtures do not attempt to serialize:

- egui paint command buffers
- compositor/layer state
- retained layout caches
- retained paint/display-list caches
- region-based paint invalidation
- platform font backend internals

Those belong to later milestones once Borrowser introduces those retained or
backend-specific artifacts explicitly.
