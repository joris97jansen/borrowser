# AA1: Paint Model Architecture And Ordering Contracts

Last updated: 2026-06-09
Status: implemented architecture contract for Milestone AA issue 1

This document defines Borrowser's paint architecture before Milestone AA adds
new visual primitives. The goal is to keep paint work explicit and
deterministic so borders, outlines, text decorations, clipping refinements,
stacking, compositing, and retained scene work can extend a named model instead
of accumulating ad hoc drawing logic.

Related code:
- `crates/gfx/src/paint/mod.rs`
- `crates/gfx/src/paint/contracts.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/context.rs`
- `crates/gfx/src/paint/inline.rs`
- `crates/gfx/src/paint/replaced.rs`
- `crates/gfx/src/paint/images.rs`
- `crates/gfx/src/paint/text_control.rs`
- `crates/gfx/src/viewport.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/flow.rs`
- `crates/browser/src/rendering/contracts.rs`

Related documents:
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/rendering/aa2-paint-primitives-input-model.md`
- `docs/rendering/w8-box-generation-formatting-debug-surfaces.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
- `docs/rendering/y4-overflow-semantics-supported-subset.md`

## Purpose

Milestone V established the typed rendering pipeline:

```text
StylePhaseOutput
  -> LayoutPhaseInput
  -> LayoutPhaseOutput
  -> PaintPhaseInput
  -> immediate paint output
```

Milestones W through Z expanded layout-owned box generation, flow, sizing, and
flex metadata. AA1 defines how the current paint system consumes those outputs
before expanding visual fidelity.

The current paint result is immediate frame output. Borrowser does not yet have
a retained display list, retained paint scene, compositor tree, GPU pipeline
contract, or pixel snapshot surface.

## Ownership Boundaries

### Layout

Layout owns the semantic data that paint consumes:

- generated box identity and child order
- layout geometry
- formatting output and inline fragments
- list marker metadata
- replaced-element layout metadata
- overflow policy
- overflow clip metadata

Paint must not recompute geometry, infer containing blocks, reinterpret
formatting-context membership, or inspect raw CSS overflow declarations.

### Paint

Paint owns translating layout output plus runtime paint context into the
current frame's visual output for the supported subset:

- box backgrounds
- list markers
- inline text fragments
- inline-block subtree painting at inline fragment positions
- replaced fragments and form-control visuals
- image/fallback visuals
- selection, caret, and focused control visuals
- applying layout-owned overflow clips to content and descendant painting

Paint does not own CSS parsing, cascade, computed style assembly, layout
geometry, retained scene storage, or compositor behavior.

### GFX

GFX owns low-level drawing execution through the egui painter and backend
runtime context. `PaintArgs` is runtime/backend context for one frame; it is
not the semantic layout-to-paint handoff.

### Browser Runtime

Browser/runtime orchestration owns phase scheduling, retained page/runtime
state, resource/input state, and invalidation entry points. It must not infer
paint order, geometry, clipping, or layout behavior.

## Layout-To-Paint Handoff

`PaintPhaseInput` is the semantic paint input. It wraps
`LayoutPhaseOutput` and exposes the layout root, document rectangle, viewport
width, and layout debug snapshot data.

`PaintArgs` is separate from `PaintPhaseInput`. It carries runtime/backend
execution context such as:

- `egui::Painter`
- viewport origin
- text measurer
- base URL and image provider
- input values, focused target, active target, and selection styling
- optional fragment-rect cache for input routing

This split is normative:

```text
PaintPhaseInput = semantic layout-to-paint handoff
PaintArgs       = runtime/backend context for one paint execution
```

Future display-list or retained-scene work must extend the semantic paint
handoff deliberately. It must not hide retained paint state inside `PaintArgs`.

## Supported Paint Ordering Subset

AA1 documents the current supported paint order. This is not full CSS painting
order and does not define stacking-context behavior.

Paint traverses paintable `LayoutBox` nodes in deterministic layout-tree
preorder. The supported order inside each visited box is:

1. paint the box background from computed style on the `LayoutBox`
2. paint the list marker when layout exposes list marker metadata
3. when layout exposes `OverflowClip`, apply that clip to contents and
   descendants
4. paint inline formatting content from layout inline fragments
5. paint child subtrees in layout child order

Inline content is ordered by the layout inline fragment sequence. Inline-block
and replaced fragments are painted at their inline fragment positions; the
later recursive child walk skips those boxes in the outer subtree so they are
not emitted twice.

The static contract surface in `crates/gfx/src/paint/contracts.rs` exposes this
subset through `paint_order_contracts()`.

## Overflow And Clipping

Overflow policy and clipping metadata are layout-owned. Paint consumes
`LayoutBox::overflow_clip()` and applies the resulting clip rectangle to the
box contents and descendant subtree.

Paint does not decide whether a box clips, creates a scroll container,
establishes an independent formatting context, or changes layout size. Those
rules remain layout-owned and are documented in the Y4 overflow contract.

Current clipping is immediate and backend-executed through an egui clipped
painter. It is not a retained clip node, scroll container, layer, or
compositing boundary.

## Determinism Expectations

For a fixed DOM, computed style tree, layout output, viewport, text measurer,
resource state, and input state, paint contracts must remain deterministic:

- paint order contract tables are static and exact
- layout tree traversal is preorder
- child order comes from layout output
- inline fragment order comes from layout inline layout
- overflow clip rectangles come from layout metadata
- immediate paint output is not retained across updates
- debug and contract tests must describe semantic behavior, not backend pixels

Visual backend output may be inspected manually as a smoke test, but AA1 does
not introduce pixel snapshot testing.

## Future Extension Points

Later AA issues may extend the paint model at named points:

- borders and box decorations: add paint primitives after background ordering
  is made explicit for the supported subset
- outlines: add a paint step with explicit geometry and ordering rules
- text decorations: extend inline fragment painting deliberately
- clipping refinements: extend layout-owned clip metadata and paint
  consumption rules
- stacking and `z-index`: introduce explicit stacking-context model and
  ordering contracts before changing traversal behavior
- compositing and GPU work: introduce compositor/layer ownership separately
  from immediate paint output
- retained display lists or retained paint scenes: add explicit retained
  artifact ownership, invalidation, and determinism contracts

## Deliberate Exclusions

AA1 deliberately does not implement or define:

- borders
- outlines
- text decorations
- full CSS painting order
- stacking contexts
- `z-index`
- compositing
- GPU pipeline behavior
- retained display lists
- retained paint scenes
- scrollbars
- scroll offsets
- pixel snapshot testing
- new visual behavior

These exclusions are exposed in code through `paint_excluded_features()` so
future work has to change the contract deliberately when one of these features
enters scope. AA2 refines this by defining a border primitive vocabulary while
still excluding visual border rendering.
