# V5: Explicit Runtime Render Orchestration Path

Last updated: 2026-05-01  
Status: implemented explicit runtime orchestration cutover for Milestone V issue 5

This document is the source-of-truth contract for Milestone V5. It defines how
the browser runtime now routes rendering work through explicit orchestration
state and typed phase contracts instead of relying on ambiguous eager rebuild
behavior as the de facto architecture.

Related code:
- `crates/browser/src/rendering.rs`
- `crates/browser/src/view.rs`
- `crates/browser/src/tab/state.rs`
- `crates/browser/src/tab/ui.rs`
- `crates/browser/src/tab/html.rs`
- `crates/browser/src/tab/nav.rs`
- `crates/gfx/src/viewport.rs`
- `crates/gfx/src/input/route/mod.rs`

Related documents:
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/architecture/ARCHITECTURE.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`

## Purpose

V1 defined ownership boundaries. V2 defined typed phase outputs. V3 defined
retained versus rebuilt ownership. V4 defined invalidation entry points and
work plans. V5 is the cutover where the runtime actually uses those contracts
to drive frame rendering.

The goal is not selective execution yet. The goal is to make orchestration
intentional, deterministic, and owned.

## Normative Runtime Flow

The shipped render path is now:

```text
runtime event
  -> RenderInvalidationRequest
  -> Tab::request_render_work(...)
  -> PendingRenderWork retained on Tab
  -> next Tab::ui_content(...)
  -> browser::view::content(...)
  -> browser::rendering::prepare_page_frame(...)
  -> browser::rendering::execute_prepared_page_frame(...)
  -> gfx::viewport::execute_viewport_frame(...)
  -> layout + paint + input routing
  -> RenderFrameExecutionTrace
  -> optional follow-up InputStateChanged request
```

This replaces the old implicit model where the next UI frame simply happened to
call style/layout/paint helpers in a workable order.

## Runtime-Owned Orchestration State

`Tab` is now the retained runtime owner of queued frame work:

```rust
struct Tab {
    ...
    pending_render_work: PendingRenderWork,
    last_render_trace: Option<RenderFrameExecutionTrace>,
}
```

The contract is:

- `request_render_work(...)` records invalidation requests in
  `PendingRenderWork`
- identical requests are deduplicated while preserving first-seen order
- repaint wakeups still happen through `poke_redraw()`
- the next frame consumes `PendingRenderWork` explicitly instead of dropping
  the invalidation contract after the wakeup
- queued render work is consumed when the runtime attempts the next
  orchestrated page frame, not only after a successful paint
- navigation start and explicit render-orchestration resets clear stale queued
  work and the previous frame trace before a new document can render

This means the current contract is:

- if no DOM is available, there is no page frame to execute and no stale
  queued render work should survive from the previous document
- if style preparation fails, the attempted frame consumes the queued work and
  the runtime renders the visible style-failure state instead of retrying the
  same invalidation indefinitely

## Prepare / Execute Split

V5 introduces an explicit two-step frame path:

### 1. `prepare_page_frame(...)`

`browser::rendering::prepare_page_frame(...)` owns the runtime style-phase
boundary for the current frame. It:

- inspects the retained style-dirty state before execution
- builds or reuses retained resolved/computed style artifacts
- rebuilds the borrow-backed `StylePhaseOutput`
- computes page background color from the style output
- packages the frame inputs into `PreparedPageFrame`

This keeps style preparation explicit while still allowing the panel fill color
to be derived before the viewport executes.

### 2. `execute_prepared_page_frame(...)`

`browser::rendering::execute_prepared_page_frame(...)` owns the runtime frame
execution cutover. It:

- hands `PreparedPageFrame` into `gfx::viewport::execute_viewport_frame(...)`
- consumes queued `PendingRenderWork`
- synthesizes in-frame `ViewportChanged` triggers when viewport metrics differ
  from the previous frame
- records `RenderFrameExecutionTrace`
- converts follow-up input repaint needs into
  `render_invalidation_request(InputStateChanged)`

This is the explicit orchestration path for visible page rendering.

## Explicit Frame Execution Trace

V5 adds a deterministic frame trace surface:

- `PendingRenderWork`
- `RenderPhaseExecutionKind`
- `RenderPhaseExecutionTrace`
- `RenderFrameExecutionTrace`

`RenderPhaseExecutionKind` distinguishes:

- `Requested`: this phase ran because queued invalidation work or an in-frame
  viewport change requested it directly or cascaded into it
- `MaterializedFromRetainedArtifacts`: style rebuilt only the borrow-backed
  `StyledTree` from retained computed style without a fresh style invalidation
- `RequiredForCurrentFrame`: the phase ran because current Borrowser still
  needs frame-local artifacts to paint the frame, even when no direct
  invalidation requested the phase

This is the key V5 clarification. Layout and paint may still execute for the
current frame even when there is no retained layout or paint cache, but that
behavior is now reported as an explicit prerequisite rather than left as
ambiguous eager rebuilding.

## Viewport And Input Cutover

`gfx::viewport::page_viewport(...)` has been replaced by the explicit
`gfx::viewport::execute_viewport_frame(...)` path.

That executor now returns:

```rust
pub struct ViewportFrameOutput {
    pub action: Option<PageAction>,
    pub viewport_changed: bool,
    pub requested_followup_render: bool,
}
```

`route_frame_input(...)` no longer calls `ui.ctx().request_repaint()` as a
hidden repaint side effect. It returns an explicit `FrameInputResult`, and the
browser runtime translates that into a follow-up `InputStateChanged`
invalidation request through `Tab::request_render_work(...)`.

This is an important ownership change:

- input routing may request future rendering work
- browser runtime owns the scheduling request
- egui is no longer the hidden owner of page-render follow-up invalidation

## Current Behavioral Baseline

V5 intentionally preserves the current visible rendering baseline:

- retained resolved/computed style artifacts still work as before
- `StyledTree`, layout output, fragment maps, and paint output remain rebuilt
- the viewport still performs one frame-local layout and paint pass per frame
  when the page is rendered
- no retained layout cache or paint cache is introduced here

The difference is structural ownership, not feature breadth.

## Determinism And Tests

The repository now pins the orchestration cutover through:

- `pending_render_work_deduplicates_and_preserves_request_order()`
- `frame_execution_trace_distinguishes_requested_work_from_frame_prerequisites()`
- `frame_execution_trace_adds_viewport_change_as_direct_runtime_trigger()`
- `ui_content_consumes_pending_render_work_through_explicit_orchestration_path()`
- `starting_new_navigation_clears_pending_render_work_and_last_trace()`

These tests validate:

- runtime invalidation requests are retained until frame execution
- frame traces distinguish requested reruns from current-frame prerequisites
- in-frame viewport changes become explicit runtime triggers
- `Tab::ui_content(...)` consumes queued render work through the new
  orchestration path instead of bypassing it
- navigation/reset boundaries clear stale queued work and prior frame traces

## Non-Goals

V5 does not introduce:

- retained layout caching
- retained paint/display-list caching
- selective phase skipping for every frame
- resource dependency graphs
- region-based paint invalidation
- compositor/layer orchestration
- async frame scheduling beyond the current repaint wakeup model

Those remain later milestones. V5 only replaces the implicit eager rebuild flow
with an explicit runtime-owned orchestration path.
