# AA2: Paint Primitives And Input Model

Last updated: 2026-06-09
Status: implemented architecture and model contract for Milestone AA issue 2

This document defines Borrowser's structured paint primitive and paint input
model. AA2 turns the layout-to-paint handoff into explicit paint-owned semantic
data while keeping the existing immediate backend drawing path mostly
unchanged.

Related code:
- `crates/gfx/src/paint/mod.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/contracts.rs`
- `crates/gfx/src/paint/inline.rs`
- `crates/gfx/src/paint/replaced.rs`
- `crates/layout/src/phase.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/inline/types.rs`

Related documents:
- `docs/rendering/aa1-paint-model-architecture-ordering-contracts.md`
- `docs/rendering/aa3-border-rendering-box-decoration.md`
- `docs/rendering/aa4-outline-rendering-box-decoration.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/y4-overflow-semantics-supported-subset.md`

## Purpose

AA1 documented the existing paint architecture and ordering subset. AA2 adds
named Rust model objects for the semantic paint data produced from layout:

```text
LayoutPhaseOutput
  -> PaintPhaseInput
  -> PaintInput
  -> PaintTree
  -> PaintPrimitive
  -> immediate backend drawing
```

`PaintInput`, `PaintTree`, and `PaintPrimitive` are paint-owned. They are
frame-local semantic data, not retained scene state.

## Ownership Boundaries

Layout owns:

- generated box identity and child order
- layout geometry
- inline formatting fragments exposed through layout APIs
- replaced-element layout metadata
- overflow policy and overflow clip metadata
- computed layout results

Paint owns:

- structured paint input construction from `PaintPhaseInput`
- paint primitive vocabulary
- paint ordering for the supported subset
- semantic paint debug snapshots

GFX/backend owns:

- egui painter execution
- backend coordinate conversion
- runtime frame context carried by `PaintArgs`

Browser/runtime owns:

- render phase orchestration
- invalidation and retained page/runtime state
- deciding when layout and paint phases run

Browser/runtime must not infer paint semantics. `PaintArgs` must not become the
semantic layout-to-paint handoff.

## Paint Input Model

`PaintPhaseInput` remains the explicit semantic phase boundary from layout to
paint. `PaintPhaseInput::to_paint_input()` materializes a `PaintInput` using
the layout output and the layout text-measurement environment needed by the
current inline fragment API.

`PaintInput` retains a borrowed reference to `LayoutPhaseOutput`; it does not
copy, replace, or own layout geometry. Its `PaintTree` is paint-owned semantic
data derived from that layout output for the current paint phase.

This keeps the ownership rule explicit:

```text
Layout owns source geometry and formatting data.
Paint owns semantic primitive construction from that data.
Backend owns drawing execution.
```

## Primitive Vocabulary

AA2 defines these paint primitive categories:

- `Background`
- `Border`
- `Outline`
- `ListMarker`
- `Clip`
- `Text`
- `InlineBox`
- `Replaced`

The border primitive was vocabulary only in AA2. AA3 populates and renders this
primitive for the supported physical solid rectangular border subset.
AA4 adds a distinct outline primitive for the supported paint-only rectangular
outline subset.

Primitives store semantic CSS-pixel rectangles, source identity, colors, font
sizes, marker kinds, and replaced kinds. They do not store `egui::Painter`,
`egui::Rect`, GPU handles, retained scene nodes, or compositor data.

## Ordering

For each paint node in the current supported subset, primitive construction
uses this order:

1. background
2. border for the AA3 supported subset
3. list marker
4. overflow clip
5. inline formatting primitives
6. child paint nodes in layout child order
7. outline for the AA4 supported subset

This matches the AA1 supported ordering subset without claiming full CSS
painting order, stacking-context behavior, or z-index semantics.

## Relation To Immediate Drawing

AA2 intentionally keeps `paint_page()` on the existing immediate drawing path.
The new primitive model is available as a deterministic semantic surface and
future execution input, but AA2 does not require routing every backend draw
through `PaintPrimitive`.

Later issues may replace direct drawing paths incrementally after the primitive
contracts are stable.

## Determinism Expectations

For fixed DOM, computed style, layout output, viewport, text measurement,
resource state, and input state, paint input construction must be deterministic.

AA2 tests should assert semantic model behavior:

- primitive presence and order
- layout-to-paint source identity
- use of layout-owned overflow and inline fragment data
- backend-independent debug snapshots

Pixel snapshots and manual visual inspection are not the AA2 contract.

## Deliberate Exclusions

AA2 does not implement:

- full display lists
- retained paint scenes
- compositor or layer trees
- GPU abstractions
- full CSS painting order
- stacking contexts or `z-index`
- unsupported border styles, border-radius, border-image, logical borders, and
  border shorthands
- outline shorthand, outline offset, unsupported outline styles, and rounded
  outline geometry
- text decorations
- pixel snapshot testing
- broad visual fidelity changes

Those features must extend this paint input model deliberately in later issues.
