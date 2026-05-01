# V3: Retained-State Versus Rebuilt-State Ownership

Last updated: 2026-05-01  
Status: implemented retained-state ownership contract for Milestone V issue 3

This document is the source-of-truth contract for Milestone V3. It defines
which rendering artifacts Borrowser retains across updates, which artifacts are
rebuilt on demand or per frame, and where that ownership lives in code.

Related code:
- `crates/browser/src/page.rs`
- `crates/browser/src/rendering.rs`
- `crates/browser/src/view.rs`
- `crates/gfx/src/viewport.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/layout/src/lib.rs`
- `crates/css/src/computed/style_tree.rs`

Related documents:
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/architecture/ARCHITECTURE.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`

## Purpose

V1 defined the rendering ownership model. V2 defined the explicit phase output
types. V3 makes retained-state ownership explicit so later incremental layout,
paint invalidation, display-list work, or retained scene work can extend a
named storage boundary rather than relying on Borrowser's current rebuild path
as an implicit design.

The normative direction is:

```text
retained page/runtime state
  + rebuilt style/layout/paint artifacts
  = one deterministic rendering pipeline
```

Retained state is not "whatever survived the last frame". It is an explicit
owner contract.

## Normative Retention Model

Borrowser now exposes a stable artifact-retention table through:

- `browser::rendering::render_artifact_ownership_contracts()`
- `browser::rendering::render_phase_contracts()`
- `PageState::render_pipeline_debug_snapshot()`

The current artifact lifetime contract is:

| artifact | semantic owner | retained owner | lifetime |
| --- | --- | --- | --- |
| `Dom` | browser runtime | browser runtime | retained across updates |
| `StylesheetSet` | browser runtime | browser runtime | retained across updates |
| `ResolvedDocumentStyle` | CSS engine | browser runtime | retained across updates |
| `ComputedDocumentStyle` | CSS engine | browser runtime | retained across updates |
| `StyledTree` | CSS engine | none | borrow-backed rebuilt on demand |
| `ViewportMetrics` | browser view | none | frame-local input |
| `TextMeasurement` | layout engine | none | frame-local input |
| `ReplacedElementMetadata` | layout engine | none | frame-local input |
| `LayoutTree` | layout engine | none | frame-local rebuilt per frame |
| `ResourceState` | browser runtime | browser runtime | retained across updates |
| `InputState` | browser runtime | browser runtime | retained across updates |
| `PaintCommands` | paint engine | none | immediate frame output |

Interpretation:

- "semantic owner" defines the meaning of the artifact
- "retained owner" identifies the runtime subsystem that stores the artifact
  across updates
- "none" means the artifact is intentionally rebuilt or emitted rather than
  retained as a long-lived rendering object
- `TextMeasurement` is layout-owned because layout defines the measurement
  contract it consumes; the concrete measurer may still be provided by the
  viewport/backend runtime for a frame

## Page-Owned Retained Rendering State

`PageState` now owns its retained rendering state through an explicit sub-owner:

```rust
struct PageState {
    base_url: Option<String>,
    dom: Option<Box<Node>>,
    head: HeadMetadata,
    visible_text_cache: String,
    form_controls: FormControlIndex,
    rendering: RetainedRenderState,
}
```

The retained rendering storage boundary is:

```rust
struct RetainedRenderState {
    document_styles: DocumentStyleSet,
    generations: PageStyleGenerations,
    style_cache: Option<PageStyleCache>,
    style_dirty: bool,
    layout_dirty: bool,
    last_restyle_trigger: Option<RestyleTrigger>,
    pending_style_invalidation: Option<StyleInvalidationScope>,
    last_style_recalc: Option<StyleRecalcKind>,
    last_style_reuse: Option<ComputedStyleReuseStats>,
}
```

This is the normative owner for page-local retained rendering artifacts and
invalidations. It is not a layout cache and not a retained paint scene.

### Retained In `RetainedRenderState`

- `DocumentStyleSet`
- `PageStyleGenerations`
- `PageStyleCache { resolved, computed }`
- dirty bits and invalidation scope
- style recompute diagnostics used for deterministic tests/debugging

### Retained In `PageState` But Outside `RetainedRenderState`

- DOM
- base URL
- head metadata
- visible text cache
- form-control index

These remain page-owned because they are broader document/runtime state, not
just rendering-pipeline storage.

### Retained Outside `PageState`

- resource/image state owned by `ResourceManager`
- input/focus/selection state owned by browser input/runtime structures

These are retained runtime inputs to rendering, but they are not page-local
render caches.

## Rebuilt Artifacts

The following artifacts remain intentionally rebuilt:

- `StylePhaseOutput`
- borrow-backed `StyledNode`
- `LayoutPhaseInput`
- `LayoutPhaseOutput`
- `PaintPhaseInput`
- fragment-rect maps used during input routing/paint
- immediate paint output

V3 keeps this baseline explicit:

- retained style artifacts may be reused across updates
- rebuilt downstream artifacts are still reconstructed from retained inputs
- no current API implies retained layout, retained display lists, or partial
  paint caches already exist

## Invalidation Direction

Retained state ownership also defines invalidation ownership:

- DOM replacement, tree mutation, attribute mutation, and text mutation enter
  through browser/runtime code
- `PageState` translates those events into retained render-state dirtiness
- `RetainedRenderState` tracks whether style artifacts can be reused
- rebuilt style/layout/paint artifacts are derived from the current retained
  state; they are not invalidated independently because they are not retained

This is the architectural direction for later incremental work:

- future retained layout state must become its own explicit retained owner
  rather than silently expanding `layout_dirty`
- future retained paint/display-list state must become its own explicit
  retained owner rather than being inferred from the current paint call path

## Phase-Contract Alignment

V3 does not replace V1 or V2. It tightens them:

- V1 still defines phase ownership and invalidation boundaries
- V2 still defines the typed handoff structures
- V3 defines where artifacts live between those handoffs

The combined contract is now:

```text
PageState / runtime retained owners
  -> StylePhaseOutput
  -> LayoutPhaseInput
  -> LayoutPhaseOutput
  -> PaintPhaseInput
  -> immediate paint output
```

The wrappers from V2 are not retention points. They are rebuilt transport
objects across phase boundaries.

## Determinism And Assertions

The repository now pins retained-state behavior through:

- `render_artifact_ownership_contracts_pin_retained_vs_rebuilt_lifetimes()`
- `debug_snapshot_reports_retained_style_artifacts_and_ephemeral_downstream_trees()`
- `attribute_mutation_keeps_style_cache_but_marks_it_stale_until_restored()`
- `text_mutation_dirties_layout_without_invalidating_computed_style()`
- `navigation_reset_clears_page_owned_retained_render_state()`

These tests validate:

- retained style artifacts live in page-owned retained state
- downstream styled/layout/paint artifacts are not retained across updates
- invalidation mutates retained ownership state, not implicit downstream caches
- navigation reset clears page-owned retained rendering state deterministically

## Non-Goals

V3 does not introduce:

- retained layout trees
- retained display lists or paint caches
- layout generations or layout cache keys
- partial layout invalidation
- compositing/layer-tree ownership
- async scene building

Those belong to later Milestone V work once the retained owner boundaries
defined here are extended deliberately.
