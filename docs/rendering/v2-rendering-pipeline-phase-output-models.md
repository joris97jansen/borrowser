# V2: Rendering Pipeline Phase Output Models

Last updated: 2026-05-01  
Status: implemented phase-output contract for Milestone V issue 2

This document is the source-of-truth contract for Milestone V2. It defines the
structured data types that carry rendering state across the style, layout, and
paint boundaries.

Related code:
- `crates/browser/src/page.rs`
- `crates/browser/src/rendering.rs`
- `crates/browser/src/view.rs`
- `crates/css/src/computed/style_tree.rs`
- `crates/layout/src/lib.rs`
- `crates/gfx/src/viewport.rs`
- `crates/gfx/src/paint/mod.rs`

Related documents:
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/architecture/ARCHITECTURE.md`

## Purpose

Milestone V1 defined who owns each rendering phase and which artifacts are
retained versus rebuilt. V2 turns that ownership model into explicit code-level
handoff structures so phase boundaries are not inferred from call ordering or
borrow patterns.

The normative rendering handoff model is now:

```text
browser::rendering::prepare_page_frame(...)
  -> css::StylePhaseOutput
  -> layout::LayoutPhaseInput
  -> layout::LayoutPhaseOutput
  -> gfx::paint::PaintPhaseInput
  -> gfx::paint::paint_page(..., PaintArgs { ... })
```

Each phase exposes one structured boundary type for the next phase. Runtime
backend concerns remain separate from semantic phase outputs.

## Normative Handoff Types

### Style Phase Output

The CSS/style phase hands off:

```rust
pub struct StylePhaseOutput<'a> {
    root: StyledNode<'a>,
}
```

Contract:

- `StylePhaseOutput` is the only normative style-to-layout handoff in the
  browser render path.
- `StyledNode` remains the semantic style tree, but it now crosses the
  subsystem boundary wrapped in an explicit phase-output type.
- `StylePhaseOutput` is rebuilt from retained `ResolvedDocumentStyle` and
  `ComputedDocumentStyle` artifacts owned by `PageState`.
- `StylePhaseOutput` is borrow-backed and frame-scoped. It is not retained in
  `PageState`.

Ownership:

- `PageState` owns when the output is rebuilt.
- `crates/css` owns the meaning and construction of the underlying `StyledNode`
  tree.

### Layout Phase Input

Layout consumes:

```rust
pub struct LayoutPhaseInput<'style_tree, 'dom, 'runtime> {
    style_root: &'style_tree StyledNode<'dom>,
    available_width: f32,
    measurer: &'runtime dyn TextMeasurer,
    replaced_info: Option<&'runtime dyn ReplacedElementInfoProvider>,
}
```

Contract:

- layout receives styled content only through `LayoutPhaseInput`
- viewport width, text measurement, and replaced-element metadata are explicit
  runtime-fed layout dependencies
- layout does not reach back into `PageState`, `browser::view`, or paint code
  to discover missing input
- the frame-scoped borrow of the rebuilt style tree is distinct from the DOM
  lifetime stored inside `StyledNode`

`LayoutPhaseInput::from_style_output(...)` is the normative adapter from
`StylePhaseOutput` into layout.

### Layout Phase Output

Layout produces:

```rust
pub struct LayoutPhaseOutput<'style_tree, 'dom> {
    root: LayoutBox<'style_tree, 'dom>,
    available_width: f32,
}
```

Contract:

- `LayoutPhaseOutput` is the only normative layout-to-paint handoff
- `LayoutBox` remains the semantic layout tree, but it crosses the boundary in
  a dedicated output wrapper
- `available_width` is retained explicitly as part of the layout environment
  for this pass
- geometry accessors such as `document_rect()`, `viewport_width()`, and
  `content_height()` are provided by the output itself rather than by
  viewport-local reconstruction

Retention:

- `LayoutPhaseOutput` is frame-local rebuilt state
- no retained layout cache is introduced in V2

### Paint Phase Input

Paint consumes:

```rust
pub struct PaintPhaseInput<'layout, 'style_tree, 'dom> {
    layout: &'layout LayoutPhaseOutput<'style_tree, 'dom>,
}
```

Contract:

- paint receives layout geometry through `PaintPhaseInput`
- paint consumes the full structured layout output, not a raw geometry root
  passed through ad hoc viewport locals
- future display-list or retained-paint work must extend this typed handoff
  rather than bypass it

### Runtime Paint Arguments

The paint boundary intentionally separates semantic layout output from backend
execution state:

```rust
pub struct PaintArgs<'a> {
    painter: &'a Painter,
    origin: Pos2,
    measurer: &'a EguiTextMeasurer,
    base_url: Option<&'a str>,
    resources: &'a dyn ImageProvider,
    input_values: &'a InputValueStore,
    ...
}
```

`PaintArgs` is not the semantic layout-to-paint handoff. It is runtime/backend
execution context for one frame.

This split is normative:

- `PaintPhaseInput` carries semantic render-phase output
- `PaintArgs` carries viewport/backend execution dependencies

## Browser Runtime Flow

The shipped browser path is:

```text
browser::rendering::prepare_page_frame(...)
  -> StylePhaseOutput
  -> browser::view::content(...)
  -> browser::rendering::execute_prepared_page_frame(...)
  -> gfx::viewport::execute_viewport_frame(...)
  -> layout::layout_document(LayoutPhaseInput::from_style_output(...))
  -> LayoutPhaseOutput
  -> gfx::paint::paint_page(PaintPhaseInput::new(...), PaintArgs { ... })
```

The important boundary rule is that the viewport orchestrates phase execution,
but it does not redefine the phase outputs. It consumes and forwards the typed
handoff structures defined by the engine crates.

## Retained Versus Rebuilt State

V2 does not change the retained-state baseline from V1. It makes the rebuilt
artifacts explicit in code.

Retained in `PageState`:

- DOM
- `DocumentStyleSet`
- `ResolvedDocumentStyle`
- `ComputedDocumentStyle`
- dirty flags, generations, and invalidation state

Rebuilt on demand or per frame:

- `StylePhaseOutput`
- `LayoutPhaseInput`
- `LayoutPhaseOutput`
- `PaintPhaseInput`
- immediate paint output

The wrappers introduced in V2 are not retention points. They are explicit
handoff contracts. V3 now makes that storage boundary explicit through the
page-owned `RetainedRenderState` contract and
`render_artifact_ownership_contracts()`.

## Determinism And Invariants

The structured phase outputs must satisfy these invariants:

- `StylePhaseOutput` always reflects the current DOM plus the retained computed
  style artifacts chosen by `PageState`
- `LayoutPhaseInput::from_style_output(...)` forwards the exact styled root
  produced by style resolution without reinterpretation
- `LayoutPhaseOutput::viewport_width()` reports the explicit input width used
  for layout, not a derived guess from the current root box geometry
- `LayoutPhaseOutput::document_rect()` and `LayoutPhaseOutput::root().rect`
  describe the same document geometry
- `PaintPhaseInput::new(&layout_output)` forwards the exact layout-phase
  output selected by the viewport for the current frame
- no later phase re-derives earlier-phase ownership from runtime-only state

## Test Contract

The repository now has targeted tests for the phase output models:

- `render_phase_contracts_pin_expected_phase_boundaries()`
- `style_to_layout_handoff_uses_explicit_phase_output_models()`
- `layout_to_paint_handoff_wraps_layout_phase_output_without_reinterpretation()`
- `render_phase_boundary_debug_snapshot_is_stable_for_simple_text_flow()`
- `render_phase_boundary_debug_snapshot_is_stable_for_replaced_element_flow()`
- `RenderPipelineDebugSnapshot` tests that continue to pin retained versus
  rebuilt state and invalidation behavior

These tests are intentionally structural. They validate the existence,
determinism, and ownership of the handoff types before later milestones add
retained layout, display lists, paint invalidation, or compositing.

## Non-Goals

V2 does not introduce:

- retained layout trees
- retained paint/display-list outputs
- paint invalidation regions
- compositing or layer trees
- advanced resource dependency invalidation
- broader rendering feature coverage

Those remain later Milestone V work. V2 only establishes the explicit typed
boundary that later work must extend.
