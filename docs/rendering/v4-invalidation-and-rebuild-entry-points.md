# V4: Invalidation And Rebuild Entry Points

Last updated: 2026-08-17
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
  -> sealed RenderInvalidationRequest (inspectable owner and phase work)
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

Borrowser exposes the normative entry-point table through the public read-only
contract/debug surfaces:

- `browser::rendering::render_invalidation_request_contracts()`
- `browser::rendering::render_invalidation_request(...)`

Production Browser code constructs intrinsic requests through the
crate-internal `render_intrinsic_invalidation_request(...)` path. That helper
is not an external Browser API and its closed source domain cannot express CSS
authorization.

The static contract records intrinsic rendering dependencies only. CSS-owned
Style work is composed after classification and therefore cannot be fabricated
by this table:

| entry point | requested by | style | layout | paint | frame orchestration |
| --- | --- | --- | --- | --- | --- |
| `DocumentReplaced` | browser runtime | none | direct | cascaded from layout | direct |
| `DomStructureChanged` | browser runtime | none | direct | cascaded from layout | direct |
| `DomAttributesChanged` | browser runtime | none | none | none | none |
| `DomTextChanged` | browser runtime | none | direct | cascaded from layout | direct |
| `DomPublicationStyleInvalidated` | CSS engine | direct only after capability consumption | cascaded from style | cascaded from layout | cascaded from style |
| `DomMutationUnclassified` | browser runtime | none | direct conservative document fallback | cascaded from layout | direct |
| `StylesheetSetChanged` | browser runtime | none | none | none | none |
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

AF4e asks CSS to classify one complete neutral DOM publication. `Some(plan)` is
applied once and produces an invariant-carrying
`AppliedCssStyleInvalidation`; only the factory consuming that capability may
turn the otherwise empty `DomPublicationStyleInvalidated` base contract into
direct Style work. `None` adds no Style work and does not advance style-input
generation. Intrinsic mutation requests remain separate. Thus
`DomTextChanged` retains direct Layout input without acquiring or being named
as the cause of CSS's aggregate Style authorization.

`RenderInvalidationEntryPoint` may name an external/runtime source or a
validated engine-owned source entering the rendering pipeline.
`DomPublicationStyleInvalidated` is the latter: it is requested by
`CssEngine`, means CSS has already classified and applied one publication
plan, and deliberately does not name the responsible mutation dimension.
Viewport, resource-state, input-state, and intrinsic DOM entry points cannot
be composed into CSS-authorized Style work.

Render invalidation requests and their work plans are sealed runtime values.
Callers inspect phase work through read-only accessors; they do not construct
arbitrary phase combinations. The general `render_invalidation_request(...)`
lookup remains a read-only contract/debug surface. Production Browser
intrinsic construction uses the closed `IntrinsicRenderInvalidationSource`
domain and `render_intrinsic_invalidation_request(...)`; that source domain
cannot express either CSS-authorized entry point. The typed CSS Style
composition factory owns the only production path that adds CSS-authorized
Style work.

## Page-Owned Invalidation Entry Points

Page-owned retained state is invalidated through explicit `PageState` methods:

### DOM Replacement And Mutation

- `PageState::replace_dom(...) -> DomPublicationRenderInvalidation`
- `PageState::mark_dom_changed(...) -> DomPublicationRenderInvalidation`

These are the normative style/layout invalidation entry points for:

- document replacement
- DOM structure mutation
- DOM attribute mutation
- DOM text mutation

`DomMutationFacts` preserves all simultaneous neutral mutation dimensions.
Page publication classifies that aggregate once in CSS and derives independent
intrinsic requests from the same facts; no dominant trigger or precedence
ordering exists.

`DomPublicationRenderInvalidation` stores the intrinsic collection separately
from one optional CSS Style request. This makes the zero-or-one publication
authorization invariant structural rather than something recovered by
scanning general entry-point values.

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

- `Tab::ui_content(...)` converts `ResourceManager::pump(...)` changes through
  `render_intrinsic_invalidation_request(ResourceStateChanged)`
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
that into `render_intrinsic_invalidation_request(InputStateChanged)` through
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
