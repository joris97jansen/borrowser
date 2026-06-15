# AA9: Paint Model Invariants And Extension Points

Last updated: 2026-06-13
Status: implemented close-out contract for Milestone AA issue 9

This document closes Milestone AA by consolidating the paint model that exists
after AA1 through AA8. It records the supported subset, ownership boundaries,
invariants, limitations, and named extension points for future paint work.

AA9 does not introduce new visual behavior, new Rust APIs, retained paint
state, display lists, compositor abstractions, GPU abstractions, invalidation
machinery, or pixel-based regression infrastructure. Detailed source contracts
remain in AA1 through AA8; this document is the close-out map for the supported
Milestone AA paint subset.

Related code:
- `crates/gfx/src/paint/contracts.rs`
- `crates/gfx/src/paint/stacking.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/debug.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/gfx/src/paint/context.rs`
- `crates/gfx/src/paint/inline.rs`
- `crates/gfx/src/paint/replaced.rs`
- `crates/gfx/src/paint/images.rs`
- `crates/gfx/src/paint/text_control.rs`
- `crates/gfx/src/viewport.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/phase.rs`
- `crates/layout/src/inline/types.rs`
- `crates/browser/src/rendering/contracts.rs`

Detailed source contracts:
- `docs/rendering/aa1-paint-model-architecture-ordering-contracts.md`
- `docs/rendering/aa2-paint-primitives-input-model.md`
- `docs/rendering/aa3-border-rendering-box-decoration.md`
- `docs/rendering/aa4-outline-rendering-box-decoration.md`
- `docs/rendering/aa5-text-decoration-rendering-subset.md`
- `docs/rendering/aa6-overflow-clipping-paint-behavior.md`
- `docs/rendering/aa7-deterministic-paint-ordering-layering-rules.md`
- `docs/rendering/aa8-paint-debug-visual-regression-surface.md`
- `docs/rendering/ab1-stacking-layering-invalidation-architecture.md`
- `docs/rendering/ab2-stacking-context-representation.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/rendering/y4-overflow-semantics-supported-subset.md`

## Milestone AA Close-Out Scope

Milestone AA expanded Borrowser paint from basic rendering into an explicit,
deterministic, documented paint model for the current supported subset:

- box backgrounds;
- physical solid rectangular borders for the AA3 supported longhand subset;
- rectangular paint-only outlines for the AA4 supported longhand subset;
- list markers exposed by layout;
- underline-only text decoration for the AA5 supported text-fragment subset;
- inline text, inline boxes, replaced fragments, images, and current
  text-control visuals;
- layout-owned overflow clipping applied to contents and descendants;
- deterministic semantic paint-order and paint-operation debug snapshots.

AA is complete only for this supported subset. It does not make Borrowser's
paint system browser-complete, and it does not claim full CSS painting order or
compositor behavior.

## Ownership Boundaries

CSS owns authored and computed style:

- parsing supported paint-related declarations;
- participating in cascade and computed-style assembly;
- exposing computed values for backgrounds, borders, outlines, text decoration,
  and overflow.

Layout owns semantic geometry and paint input sources:

- generated layout tree structure and source order;
- layout box geometry;
- inline fragment order and geometry;
- list marker metadata;
- replaced-element layout metadata;
- overflow policy and overflow clip metadata.

Paint owns the current supported paint model:

- consuming `PaintPhaseInput` from layout output;
- constructing frame-local `PaintInput`, `PaintTree`, `PaintNode`, and
  `PaintPrimitive` data;
- defining the supported paint primitive vocabulary;
- applying paint-owned ordering semantics for the supported subset;
- producing immediate paint output for the current frame;
- serializing deterministic paint-owned debug snapshots.

GFX/backend owns low-level immediate drawing execution through backend runtime
context. `PaintArgs` carries the painter, frame origin, text measurer, resource
access, image provider, input state, and selection/focus context for one paint
execution. It is not the semantic layout-to-paint handoff.

Browser/runtime owns orchestration:

- render phase scheduling;
- invalidation entry points;
- retained page/runtime state;
- passing semantic phase outputs forward.

Browser/runtime must not emulate, repair, or reinterpret paint order, clipping,
geometry, or primitive semantics.

## Current Paint Model

The supported paint pipeline is:

```text
LayoutPhaseOutput
  -> PaintPhaseInput
  -> PaintInput
  -> PaintTree
  -> PaintPrimitive
  -> immediate paint output
```

`PaintPhaseInput` is the semantic layout-to-paint phase boundary.
`PaintInput`, `PaintTree`, and `PaintPrimitive` are paint-owned, frame-local
semantic data derived from layout output. They are not retained display lists,
retained scenes, backend command streams, compositor layers, or GPU resources.

Immediate painting remains the runtime output path. The semantic paint model and
immediate path must stay aligned for the supported subset.

## Primitive Vocabulary

The supported primitive vocabulary is:

- `Background`
- `Border`
- `Outline`
- `ListMarker`
- `Clip`
- `Text`
- `TextDecoration`
- `InlineBox`
- `Replaced`

Primitives carry backend-independent semantic data such as source identity,
CSS-pixel rectangles, colors, font sizes, marker kinds, replaced kinds, and clip
scope. They do not carry `egui::Painter`, backend shape objects, texture ids,
GPU handles, retained scene nodes, or compositor state.

## Supported Ordering Rules

Paint order is deterministic by construction. It follows layout-owned traversal
and paint-owned per-box sequencing; it must not be manufactured by sorting
primitives after construction.

For each paintable layout box, the supported AA order is:

1. box background;
2. box border;
3. list marker;
4. overflow clip for contents and descendants;
5. inline formatting content;
6. child subtrees in layout child order;
7. box outline.

Inline formatting content follows layout-owned inline fragment order. For the
AA5 supported text-decoration subset, decorated text fragments emit text first
and then text decoration. Inline-block and replaced fragments paint at their
inline fragment positions, and the outer child walk avoids emitting them twice.

This order is the supported Borrowser AA subset. It is not full CSS painting
order, and it does not define stacking contexts, `z-index`, compositor layers,
or opacity/transform/filter ordering.

## Overflow Clip Scope

Overflow semantics are layout-owned. Paint consumes `LayoutBox::overflow_clip()`
and emits/executes a paint-owned clip scope only when layout exposes clip
metadata.

A box's own overflow clip applies to:

- inline content emitted for that box;
- child subtrees in layout child order;
- descendant painting reached through those children, including descendant
  outlines.

A box's own overflow clip does not apply to:

- the box's own background;
- the box's own border;
- the box's own list marker;
- the box's own outline.

Ancestor clips remain active for descendant painting. The current clip model is
an immediate execution scope, not a retained clip node, scroll container,
stacking context, layer, or compositor primitive.

## Debug And Regression Surfaces

The supported paint debug surfaces are internal deterministic regression
contracts:

- `PaintInput::to_debug_snapshot()`;
- `PaintInput::to_order_debug_snapshot()`;
- `PaintInput::to_operation_debug_snapshot()`.

These snapshots are semantic and structural. They are not public APIs, retained
display lists, retained paint scenes, backend command streams, egui draw-command
serialization, compositor state, GPU resources, or pixel/raster comparison
surfaces.

The AA8 paint-operation snapshot describes paint-owned structural operations
derived from primitives and AA ordering rules. It intentionally avoids backend
shape internals, texture identifiers, resource URLs, platform font details, and
pixel output.

## Determinism Invariants

For a fixed DOM, computed style tree, layout output, viewport, text measurer,
resource state, and input state:

- paint input construction is deterministic;
- layout tree traversal order is deterministic;
- sibling order comes from layout-owned child order;
- inline item order comes from layout-owned inline fragments;
- supported primitive construction is paint-owned and backend-independent;
- border and outline edge operation order is deterministic;
- overflow clip rectangles come from layout-owned metadata;
- own background, border, list marker, and outline stay outside the box's own
  overflow clip;
- ancestor clips remain active for descendant painting;
- immediate painting and semantic paint debug surfaces remain aligned for the
  supported subset;
- debug snapshots use stable labels, fixed field ordering, and deterministic
  geometry/color formatting where applicable.

## Supported Subset Limitations

The AA paint model deliberately excludes:

- full CSS painting order;
- behavioral stacking contexts beyond AB2's root representation;
- `z-index`;
- compositing;
- retained display lists;
- retained paint scenes;
- GPU layer trees or GPU pipeline abstractions;
- invalidation or retained paint cache machinery;
- transforms;
- opacity;
- blend modes;
- filters;
- scrollbars and scroll offset painting;
- border radius, border images, and unsupported border styles;
- outline offset, unsupported outline styles, and rounded outline geometry;
- text decoration beyond the AA5 underline-only text-fragment subset;
- advanced background images, repeat, positioning, sizing, attachment, and
  multiple backgrounds;
- pixel-perfect, screenshot, raster, or platform-dependent visual regression
  testing.

Those features require explicit follow-up contracts before they change paint
ordering, primitive vocabulary, retained state, debug surfaces, or runtime
orchestration.

## Future Extension Points

The following are named attachment points for future milestones. They are not
implementation commitments and do not imply support today:

- richer box decorations: extend border, outline, background, clipping, and
  geometry contracts without re-owning CSS or layout;
- full text decoration: extend CSS property support, layout inline decoration
  metadata, and paint decoration primitives deliberately;
- stacking contexts and `z-index`: extend the AB1/AB2 stacking model before
  changing traversal or layering behavior;
- compositing and layers: define compositor ownership separately from immediate
  paint output and semantic paint primitives;
- retained display lists or retained paint scenes: add explicit retained
  artifact ownership, invalidation, and determinism contracts;
- GPU acceleration: introduce backend and resource ownership without storing GPU
  state in semantic paint primitives;
- scrollable overflow painting: extend layout/runtime overflow ownership before
  adding scrollbars, scroll offsets, or retained clip/scroll nodes;
- pixel or raster visual regression: add a platform-aware determinism contract
  separate from AA8 structural snapshots;
- incremental invalidation: route through browser/runtime invalidation contracts
  and preserve explicit phase boundaries.

AB1 is the architecture contract that connects these extension points. It does
not remove the current exclusions by itself; it defines the ownership,
determinism, debug-surface, and invalidation rules future issues must satisfy
before doing so.

Any future extension must state which subsystem owns the new semantics, which
existing contract changes, which debug surface proves determinism, and which
AA exclusions are deliberately removed.

## Close-Out Rule

Milestone AA is complete and unambiguous because the supported paint subset now
has explicit architecture, primitives, ordering rules, overflow clip behavior,
debug surfaces, limitations, invariants, and future attachment points.

That completion is scoped to the documented supported subset only. Future work
must extend the model through explicit contracts rather than treating AA9 as
evidence that unsupported browser paint features already exist.
