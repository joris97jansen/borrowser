# V1: Rendering Architecture, Ownership Boundaries, And Phase Contracts

Last updated: 2026-05-02  
Status: implemented architecture contract for Milestone V issue 1

This document is the source-of-truth contract for Milestone V1. It defines
Borrowser's rendering architecture before retained layout, paint
incrementality, and deeper rendering features are expanded.

Related code:
- `crates/browser/src/rendering.rs`
- `crates/browser/src/page.rs`
- `crates/browser/src/view.rs`
- `crates/gfx/src/viewport.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/layout/src/lib.rs`
- `crates/css/src/computed/style_tree.rs`
- `crates/gfx/src/lib.rs`

Related documents:
- `docs/architecture/ARCHITECTURE.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w2-structured-box-tree-data-structures.md`
- `docs/rendering/w3-display-to-box-generation-behavior.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`

## Purpose

Borrowser already renders real page content, but the rendering path has been
described more by call ordering than by explicit subsystem contracts. Milestone
V makes the rendering architecture normative before later work adds retained
layout state, paint invalidation, compositing, or broader feature coverage.

The contract established here is:

```text
DOM + document-order stylesheets
  -> retained resolved/computed style artifacts
  -> rebuilt borrow-backed StyledNode tree
  -> frame-local BoxTree generation
  -> frame-local LayoutBox geometry projection
  -> immediate paint commands
  -> egui/wgpu submission
```

The key result is that Borrowser now has an explicit retained-vs-rebuilt model:

- page state retains DOM, stylesheet slots, and resolved/computed style caches
- style-tree views are rebuilt from retained style artifacts
- layout trees are rebuilt frame-locally from the styled tree
- paint output is immediate frame output, not a retained display list

This is a deliberate baseline, not an accident of eager rebuild code.

## Normative Pipeline

The current rendering pipeline is:

```text
runtime events
  -> browser::Tab event routing
  -> browser::rendering::PendingRenderWork
  -> browser::PageState DOM + stylesheet-set ownership
  -> browser::rendering::prepare_page_frame(...)
  -> css::build_style_tree_from_computed_styles(...)
  -> css::StylePhaseOutput
  -> browser::rendering::execute_prepared_page_frame(...)
  -> gfx::viewport::execute_viewport_frame(...)
  -> layout::layout_document(LayoutPhaseInput::from_style_output(...))
  -> gfx::paint::paint_page(PaintPhaseInput::new(...), PaintArgs { ... })
  -> gfx::Renderer egui/wgpu submission
```

The runtime owns scheduling and invalidation. Engine crates own rendering
semantics inside their phases. No later phase may reach backward and re-own an
earlier phase's job.

## Ownership Boundaries

### Browser Runtime And Page State

The browser runtime owns:

- DOM lifetime for the active document
- document-order stylesheet attachment and slot lifetime
- page-local generations and dirty state
- translating patch-layer identities into DOM node IDs before restyle hints
  reach `PageState`
- style invalidation entry points
- deciding when downstream layout/paint work must be considered invalid
- view-level orchestration between style resolution and the viewport

The browser runtime must not:

- parse authored CSS in layout or paint code
- compute selector matching, cascade winners, or computed values
- retain self-referential `StyledNode` or `LayoutBox` trees inside `PageState`
- treat eager layout rebuilds as proof that layout ownership is implicit

### CSS Engine

The CSS engine owns:

- structured stylesheet parsing
- selector parsing and matching
- cascade and computed-style assembly
- `ResolvedDocumentStyle`, `ComputedDocumentStyle`, and `StyledNode`
  construction

The CSS engine must not:

- own viewport, input, or paint scheduling
- fetch resources
- decide layout geometry or paint order

### Layout Engine

The layout engine owns:

- `BoxTree` generation
- display-to-box generation decisions
- `LayoutBox` geometry projection
- block and inline geometry
- text measurement consumption
- replaced-element intrinsic size consumption

Milestones W1 and W2 refine this boundary: the box tree is a distinct
layout-owned model derived from `StyledNode` and computed style, not the DOM
tree and not style data itself.

The layout engine must not:

- parse CSS text
- inspect stylesheet ordering or cascade provenance
- own image loading or input routing
- emit backend paint commands directly

### Paint System

The paint system owns:

- translating layout geometry plus runtime paint state into draw commands
- backgrounds, inline text fragments, replaced-element painting, and selection
  visuals for the current supported feature set

The paint system must not:

- compute layout geometry
- perform selector/cascade work
- introduce retained scene ownership as an implicit side effect

### Viewport And Renderer Runtime

`browser::view` plus `gfx::viewport` own viewport orchestration:

- requesting styled content from page state
- supplying viewport width, resources, and input state to layout/paint
- routing hit-testing and input behavior against the frame-local layout tree
- forwarding immediate paint output into the `gfx::Renderer` backend path

`gfx::Renderer` owns egui/wgpu integration and GPU submission only. It is not
the page layout engine and does not own CSS, layout, or page paint semantics.

## Phase Contracts

### Style Phase

Coordinator: `browser::PageState`  
Semantic owner: `crates/css`

Consumes:

- active DOM
- document-order stylesheet set

Produces:

- `ResolvedDocumentStyle`
- `ComputedDocumentStyle`
- borrow-backed `StyledNode`

Retention contract:

- `ResolvedDocumentStyle` is retained in `PageStyleCache`
- `ComputedDocumentStyle` is retained in `PageStyleCache`
- `StyledNode` is rebuilt on demand from the current DOM plus retained computed
  styles

Style-phase invariants:

- `StyledNode` is a derived view, not retained page-owned state
- downstream layout/paint code consumes `ComputedStyle` or `StyledNode`, not
  authored CSS text
- `Node::style` is not the normative style handoff

### Layout Phase

Coordinator: `browser::rendering::execute_prepared_page_frame(...)`
via `gfx::viewport::execute_viewport_frame(...)`  
Semantic owner: `crates/layout`

Consumes:

- `StyledNode`
- viewport width/available size
- text measurement
- replaced-element intrinsic metadata

Produces:

- `BoxTree`-backed `LayoutBox` geometry tree

Retention contract:

- no retained layout cache exists yet
- the generated `BoxTree` and `LayoutBox` geometry tree are rebuilt
  frame-locally for the viewport render pass

Layout-phase invariants:

- layout reads computed values only
- layout does not parse CSS or inspect style provenance
- layout output is the only geometry input to paint

### Paint Phase

Coordinator: `browser::rendering::execute_prepared_page_frame(...)`
via `gfx::viewport::execute_viewport_frame(...)`  
Semantic owner: `gfx::paint`

Consumes:

- `LayoutBox` geometry tree
- image/resource state
- input/focus/selection state

Produces:

- immediate paint commands issued through egui

Retention contract:

- no retained display list or paint cache exists yet
- paint output is immediate frame output, not a retained scene object

Paint-phase invariants:

- paint uses layout geometry as authoritative
- paint may not change layout geometry or restyle content
- paint state is runtime-controlled and deterministic from current inputs

### Frame Orchestration Phase

This is an orchestration phase, not a semantic rendering phase on the same
level as style, layout, or paint. It coordinates one viewport frame using the
outputs and invariants owned by those phases.

Coordinator: `browser::view::content(...)`  
Execution owner: `gfx::viewport`

Consumes:

- `StyledNode`
- viewport metrics
- resources
- input state

Produces:

- one frame-local layout/paint execution for the current viewport
- input routing results such as navigation actions

Viewport invariants:

- browser view requests style-tree construction from `PageState`; it does not
  reimplement style invalidation
- viewport code may orchestrate layout and paint, but it does not become the
  owner of CSS or layout semantics
- hit-testing and input routing operate against the same frame-local layout
  tree used for paint

## Retained Versus Rebuilt State

### Retained Page-Owned State

Retained state is owned by `PageState` and survives across frames:

- active DOM
- head metadata
- visible text cache
- form-control index/input state ownership boundary
- explicit `RetainedRenderState` sub-owner containing:
  `DocumentStyleSet`, `PageStyleGenerations`, `PageStyleCache { resolved, computed }`,
  and dirty/invalidation metadata

### Rebuilt Derived State

Rebuilt state is intentionally not retained across frames today:

- borrow-backed `StyledNode` view
- generated `BoxTree`
- `LayoutBox` geometry tree
- fragment-rect maps used during paint/input routing
- immediate paint commands

This boundary is now explicit. Clean frames may reuse retained style artifacts,
but they still rebuild the derived styled-tree view and the current frame-local
layout/paint artifacts.

### Deferred Retention Work

Later Milestone V issues may introduce:

- retained layout cache
- explicit layout invalidation scopes beyond the current page-level baseline
- retained paint/display-list artifacts
- partial layout or paint recomputation

None of those exist yet, and no current API should imply that they do.

## Invalidation And Rebuild Entry Points

The browser runtime owns the invalidation entry points. Current entry points
are:

- document replacement and navigation reset
- DOM structural mutations
- DOM attribute mutations
- DOM text mutations
- stylesheet reconciliation changes from DOM updates
- external stylesheet install/fail/abort state changes
- viewport width changes
- resource state changes that affect replaced-element sizing or paint
- input/focus/selection state changes

Current downstream effects:

| entry point | owner | style effect | layout effect | paint effect |
| --- | --- | --- | --- | --- |
| document replacement | browser runtime | full invalidation | dirty | dirty |
| DOM structure mutation | browser runtime | full invalidation | dirty | dirty |
| DOM attribute mutation | browser runtime | suffix invalidation when proven safe, else full | dirty | dirty |
| DOM text mutation | browser runtime | no computed-style invalidation by itself in the current supported selector/property model | dirty | dirty |
| stylesheet set/state change | browser runtime | full invalidation | dirty | dirty |
| viewport change | viewport runtime | none | rebuild frame-local layout | repaint |
| resource/input state change | viewport/runtime | none | maybe rebuild depending on phase input | repaint |

`style_dirty` and `layout_dirty` are invalidation-state signals, not proof that
Borrowser already has a retained layout artifact to reuse.

The DOM text-mutation rule is intentionally scoped. It remains valid only while
text content is not style-relevant in the supported selector/property model.
Today that excludes cases such as `:empty`, `:has(...)`, text-sensitive
generated content, and similar features that would require broader style
invalidation. `<style>` text changes are already handled separately through
stylesheet reconciliation.

## Determinism And Invariants

The rendering pipeline must remain deterministic under these invariants:

- document-order stylesheet slots determine author stylesheet order
- clean style inputs may reuse retained computed-style artifacts
- `StyledNode` rebuilds must validate against the current DOM rather than pair
  styles by shape alone
- layout consumes only typed computed values and explicit viewport/runtime
  inputs
- paint consumes only layout geometry and explicit runtime paint state
- later phases must not silently backfill missing earlier-phase semantics
- fallback behavior for supported CSS stays inside the CSS engine

## Debug And Test Surfaces

Milestone V1 introduces deterministic rendering contract surfaces:

- `browser::rendering::render_phase_contracts()`
  Records the normative phase ownership, inputs, outputs, retained outputs, and
  rebuild triggers.
- `browser::rendering::render_invalidation_request_contracts()`
  Records the normative runtime entry points that request style/layout/paint
  reruns and how those requests cascade through frame orchestration.
- `PageState::render_pipeline_debug_snapshot()`
  Reports whether retained style artifacts are absent, fresh, or stale, and
  reports the current rebuilt-state policy for styled-tree, layout, and paint
  outputs. For frame-local layout and immediate paint output, this is a policy
  surface, not proof that `PageState` is retaining those artifacts between
  frames.
- `browser::rendering::render_phase_boundary_debug_snapshot(...)`
  Records deterministic style, layout, paint-input, and orchestration
  snapshots for representative pipeline flows without depending on backend
  paint command output.
- browser tests covering style-cache reuse, attribute invalidation, and
  text-only downstream invalidation

These surfaces are intentionally structured and stable so later milestones can
extend them instead of re-describing rendering behavior in comments.

## Out Of Scope For V1

This issue does not introduce:

- retained layout caching
- retained paint/display-list caching
- partial layout invalidation beyond the current style-driven baseline
- compositing, layers, transforms, or stacking-context architecture
- major new layout or paint feature support
- async scene building or render-thread ownership transfer

Those belong to later rendering milestones once the ownership model defined
here is being enforced consistently.
