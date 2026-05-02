# V4: Invalidation And Rebuild Entry Points

Last updated: 2026-05-02  
Status: implemented invalidation-entry-point contract for Milestone V issue 4

This document is the source-of-truth contract for Milestone V4. It defines the
entry points through which style, layout, and paint work are invalidated or
rerun, who may request that work, and how runtime-triggered invalidation moves
through Borrowser.

Related code:
- `crates/browser/src/rendering.rs`
- `crates/browser/src/page.rs`
- `crates/browser/src/tab/state.rs`
- `crates/browser/src/tab/html.rs`
- `crates/browser/src/tab/css.rs`
- `crates/browser/src/tab/discovery.rs`
- `crates/browser/src/tab/image.rs`
- `crates/browser/src/tab/ui.rs`
- `crates/browser/src/view.rs`

Related documents:
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/architecture/ARCHITECTURE.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`

## Purpose

V1 defined the rendering ownership boundaries. V2 defined the typed handoff
structures. V3 defined retained versus rebuilt ownership. V4 now defines how
runtime events request rendering work so later dirty-bit, incremental layout,
or paint invalidation work can extend a named orchestration model instead of
attaching ad hoc redraw behavior to unrelated code paths.

The goal is not deep optimization yet. The goal is explicit invalidation flow.

## Normative Runtime Flow

The shipped invalidation path is now:

```text
runtime event or viewport/runtime state change
  -> explicit RenderInvalidationEntryPoint
  -> RenderInvalidationRequest { requested_by, work }
  -> PageState retained-state mutation when page-owned state is affected
  -> Tab::request_render_work(...)
  -> PendingRenderWork
  -> repaint request
  -> browser::view::content(...)
  -> browser::rendering::prepare_page_frame(...)
  -> browser::rendering::execute_prepared_page_frame(...)
  -> style/layout/paint rerun according to dirty state and typed handoffs
```

This is the contract:

- runtime code chooses an invalidation entry point
- page code mutates retained state only through explicit invalidation methods
- tab/runtime code requests repaint through the resulting `RenderInvalidationRequest`
- view/viewport rerun the pipeline on the next frame using the retained state
  and typed phase outputs already defined in V1-V3

## Entry-Point Contract Table

Borrowser exposes the normative entry-point table through:

- `browser::rendering::render_invalidation_request_contracts()`
- `browser::rendering::render_invalidation_request(...)`

The shipped entry points are:

| entry point | requested by | style | layout | paint | frame orchestration |
| --- | --- | --- | --- | --- | --- |
| `DocumentReplaced` | browser runtime | direct | cascaded from style | cascaded from layout | cascaded from style |
| `DomStructureChanged` | browser runtime | direct | cascaded from style | cascaded from layout | cascaded from style |
| `DomAttributesChanged` | browser runtime | direct | cascaded from style | cascaded from layout | cascaded from style |
| `DomTextChanged` | browser runtime | none | direct | cascaded from layout | direct |
| `StylesheetSetChanged` | browser runtime | direct | cascaded from style | cascaded from layout | cascaded from style |
| `ViewportChanged` | browser view | none | direct | cascaded from layout | direct |
| `ResourceStateChanged` | browser runtime | none | direct | direct | direct |
| `InputStateChanged` | browser view | none | none | direct | direct |

Interpretation:

- "direct" means the entry point is listed as a direct rebuild trigger for
  that phase
- "cascaded from style" means the phase reruns because style outputs change
- "cascaded from layout" means the phase reruns because layout outputs change
- "frame orchestration" is the runtime request to execute the viewport frame
  path again

## Page-Owned Invalidation Entry Points

Page-owned retained state is invalidated through explicit `PageState` methods:

### DOM Replacement And Mutation

- `PageState::replace_dom(...) -> RenderInvalidationRequest`
- `PageState::mark_dom_changed(...) -> RenderInvalidationRequest`

These are the normative style/layout invalidation entry points for:

- document replacement
- DOM structure mutation
- DOM attribute mutation
- DOM text mutation

`RestyleTrigger` still classifies DOM mutations, but V4 now turns that
classification into an explicit render invalidation request.

### Stylesheet-Set Changes

- `PageState::reconcile_document_stylesheets() -> PageStylesheetReconcile`
- `PageState::apply_css_block(...) -> Option<RenderInvalidationRequest>`
- `PageState::mark_css_done(...) -> Option<RenderInvalidationRequest>`
- `PageState::mark_css_failed(...) -> Option<RenderInvalidationRequest>`
- `PageState::mark_css_aborted(...) -> Option<RenderInvalidationRequest>`

These are the normative style invalidation entry points for stylesheet slot
discovery and external stylesheet load-state changes.

The important boundary is:

- `PageState` owns retained style invalidation and cache dirtiness
- the runtime does not mutate style-dirty/layout-dirty state directly
- the runtime consumes the returned invalidation request and schedules a frame

## Runtime-Orchestrated Entry Points

Not all invalidation comes from page-owned retained state.

### Resource State

Resource invalidation is runtime-owned:

- `Tab::ui_content(...)` converts `ResourceManager::pump(...)` changes into
  `render_invalidation_request(ResourceStateChanged)`
- `Tab::on_image_network_error(...)` requests the same resource-state
  invalidation explicitly

This keeps image/resource changes out of `PageState` while still making the
layout/paint rerun contract explicit.

`ResourceStateChanged` is intentionally conservative in V4. Decoded image
metadata can affect replaced-element intrinsic sizing, so the current contract
directly reruns both layout and paint. Future resource dependency tracking may
split this into layout-affecting and paint-only resource invalidations, but
that distinction is intentionally out of scope for this milestone.

### Input State

`InputStateChanged` is part of the shipped runtime path. Viewport/input routing
now returns explicit follow-up render intent, and the browser runtime converts
that into `render_invalidation_request(InputStateChanged)` through
`Tab::request_render_work(...)`.

The contract is:

- input changes do not rerun style
- input changes do not rerun layout in the current baseline
- input changes rerun paint and frame orchestration

Later incremental input or caret/selection invalidation work must preserve
that boundary unless layout truly becomes input-dependent for a specific case.

### Viewport State

`ViewportChanged` is likewise explicit in the contract table:

- viewport changes do not rerun style
- viewport changes rerun layout directly
- paint reruns from the new layout output
- the runtime requests a new frame

This contract exists now even though V4 does not introduce a more advanced
viewport/layout scheduler yet.

## Runtime Request Boundary

`Tab::request_render_work(...)` is now the browser-side runtime boundary for
requesting pipeline work from an invalidation contract.

This matters because redraw is no longer just:

```text
"something changed, so call poke_redraw()"
```

For rendering invalidation paths, it is now:

```text
"this named invalidation entry point requests these reruns, so queue the work
and request the next frame through the explicit render-work boundary"
```

`poke_redraw()` still exists for non-pipeline UI/status refreshes such as
loading text or navigation-bar state. V4 does not force every UI repaint
through the rendering invalidation model. It only formalizes the page-rendering
pipeline invalidation paths.

## Determinism And Tests

The repository now pins invalidation behavior through:

- `render_invalidation_request_contracts_pin_runtime_entry_points()`
- `render_invalidation_request_contracts_cover_each_entry_point_once()`
- `direct_invalidation_phase_sources_align_with_phase_rebuild_triggers()`
- `document_replacement_returns_explicit_full_pipeline_work_request()`
- `dom_text_mutation_returns_explicit_layout_and_paint_work_request()`
- `stylesheet_reconcile_returns_explicit_style_invalidation_request()`

These tests validate:

- every shipped runtime entry point has exactly one invalidation contract
- direct invalidation requests align with the phase rebuild-trigger tables
- DOM and stylesheet entry points now return explicit runtime render-work
  requests rather than only mutating dirty flags

## Non-Goals

V4 does not introduce:

- targeted dirty-bit graphs
- retained layout invalidation regions
- retained paint invalidation regions
- display-list diffing
- compositing invalidation
- async render scheduling
- a new frame scheduler beyond the current repaint request path

Those remain later work. V4 only establishes the invalidation and rebuild
entry-point contract that future optimization must extend.
