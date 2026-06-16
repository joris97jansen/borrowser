# AB1: Stacking, Layering, And Invalidation Architecture

Last updated: 2026-06-15
Status: architecture contract for Milestone AB issue 1

This document defines Borrowser's architecture for future stacking-context,
layering, paint-ordering, and paint-invalidation work. It does not introduce
new visual behavior, new CSS property support, behavioral `z-index` sorting,
layer-tree construction, retained display lists, compositor layers, targeted
repaint execution, opacity, transforms, filters, blend modes, or GPU behavior.

AB1 exists so later Milestone AB issues can remove specific exclusions through
explicit, testable contracts instead of patching paint traversal, invalidation,
or runtime scheduling ad hoc.

Related code:
- `crates/gfx/src/paint/contracts.rs`
- `crates/gfx/src/paint/stacking.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/debug.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/browser/src/rendering/contracts.rs`
- `crates/browser/src/rendering/invalidation.rs`
- `crates/browser/src/page/retained_render_state.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/phase.rs`
- `crates/css/src/computed/style.rs`

Related documents:
- `docs/rendering/aa1-paint-model-architecture-ordering-contracts.md`
- `docs/rendering/aa6-overflow-clipping-paint-behavior.md`
- `docs/rendering/aa7-deterministic-paint-ordering-layering-rules.md`
- `docs/rendering/aa9-paint-model-invariants-extension-points.md`
- `docs/rendering/ab2-stacking-context-representation.md`
- `docs/rendering/ab3-z-order-layering-semantics.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`

## Purpose

Milestone AA made the current paint subset deterministic, but its ordering is
still intentionally limited to layout-tree traversal plus paint-owned per-box
sequencing. Milestone V made runtime invalidation explicit, but paint output is
still immediate frame output with no retained paint scene.

AB1 defines the next architectural layer:

```text
computed style inputs
  -> layout-owned geometry and tree order
  -> paint-owned stacking-context model
  -> paint-owned semantic paint layers
  -> deterministic cross-context paint order
  -> immediate paint output today
  -> explicit invalidation boundaries for future retained paint work
```

The current implementation remains:

```text
LayoutPhaseOutput
  -> PaintPhaseInput
  -> PaintInput
  -> StackingContextTree
  -> PaintTree
  -> PaintPrimitive
  -> immediate paint output
```

AB1 does not replace that pipeline. AB2 adds the first explicit
root-context-only `StackingContextTree` representation inside `PaintInput`
without changing visual paint order. AB3 refines that representation with the
first narrow behavioral child-context and z-order subset for positioned boxes
with computed integer `z-index`.

## Ownership Boundaries

### CSS

CSS owns authored and computed inputs that may later create stacking contexts
or affect stacking order. Examples include positioned layout values, `z-index`,
opacity, transforms, filters, blend modes, isolation, containment, and other
future style features.

CSS does not decide paint order, layer membership, compositor promotion, paint
invalidation regions, or retained paint artifact lifetime.

### Layout

Layout owns the geometry and structural facts consumed by paint:

- layout tree structure and deterministic child order;
- generated box identity;
- containing-block and formatting-context relationships;
- final box geometry;
- inline fragment order and geometry;
- replaced-element metadata;
- overflow policy and clip metadata.

Layout does not sort paint primitives, construct compositor layers, retain
paint scenes, or decide invalidation entry points.

### Paint / GFX

Paint owns the semantic stacking and layering model:

- identifying stacking-context boundaries from computed style and layout facts
  once those inputs are supported;
- assigning paintable items to paint-owned stacking categories and semantic
  paint layers;
- determining deterministic ordering within and across stacking contexts;
- preserving backend-independent debug surfaces for stacking and ordering;
- emitting immediate paint output for the current backend path.

GFX/backend owns immediate low-level drawing execution. Backend clipping,
textures, GPU resources, and egui painter state are execution details, not the
semantic stacking model.

### Browser / Runtime

Browser/runtime owns:

- invalidation entry points;
- retained page/render state;
- render-work queueing;
- phase rerun policy;
- frame orchestration.

Browser/runtime must not reinterpret paint semantics. It may request that paint
rerun, retain future paint artifacts through explicit contracts, or narrow
future repaint work through explicit dependency data, but it must not decide
which box paints above another.

## Core Concepts

### Stacking Context

A stacking context is a paint-owned ordering boundary. It groups a box and its
descendant paintable content into an atomic unit relative to sibling stacking
contexts in the parent context.

AB2 implements the root stacking context as an explicit paint-owned
representation. AB3 adds child stacking contexts only for positioned generated
boxes with computed integer `z-index`. Future issues that broaden stacking
behavior must explicitly define:

- which computed style and layout facts establish a context;
- the context's parent relationship;
- whether the context participates in normal flow, positioned flow, or a later
  specialized category;
- how its contents are ordered internally;
- how it is represented in debug output.

The root document establishes the root stacking context. AB2 represents that
context with `StackingContextId::ROOT`; all currently paintable layout boxes
belong to that context and current AA paint behavior remains unchanged.

### Semantic Paint Layer

A semantic paint layer is a paint-owned grouping used to order paintable items
inside a stacking context. It is not a compositor layer, retained scene node,
GPU layer, texture, backend command buffer, or display-list allocation.

Future AB behavior should model layers as semantic ordering categories first.
Only a later retained scene or compositor milestone may decide whether a
semantic layer becomes a retained artifact or backend-composited resource.

### Stackable Paint Item

A stackable paint item is the paint-owned unit that participates in
stacking-context ordering. It may correspond to a layout box, an inline
fragment group, a generated paint node, or a child stacking context, depending
on the supported subset introduced by a future issue.

Stackable paint items must be built from existing phase outputs. They must not
be discovered by scanning already-emitted backend draw commands.

### Paint Invalidation Boundary

A paint invalidation boundary describes the smallest future semantic paint
artifact that can be considered dirty independently of the full frame. In AB1
this is an architecture concept only. The current runtime still requests paint
work through existing `RenderInvalidationEntryPoint` and `RenderWorkPlan`
contracts.

Future targeted repaint work must define:

- which retained or frame-local artifact can be invalidated;
- which runtime entry point or dependency change dirties it;
- how dirty regions or dirty context identifiers are derived;
- how conservative fallback to broader repaint is represented;
- which debug surface proves determinism.

## Layering And Paint Ordering Model

Future stacking behavior must be deterministic by construction. Paint must not
sort already-emitted primitives as a late fix for ordering. Ordering must be
derived before primitive emission from:

- computed style values owned by CSS;
- geometry, tree order, formatting structure, and clip metadata owned by
  layout;
- paint-owned stacking-context and semantic-layer rules.

The current AA per-box order remains unchanged:

1. box background;
2. box border;
3. list marker;
4. overflow clip for contents and descendants;
5. inline formatting content;
6. child subtrees in layout child order;
7. box outline.

Future AB issues may refine how boxes and child contexts are grouped around
this order, but must name which part of the AA order changes and update
contracts, tests, and debug snapshots accordingly.

## Overflow Clips Are Not Stacking Contexts

Overflow clipping remains a scoped paint execution context derived from
layout-owned `OverflowClip` metadata. It does not establish a stacking context,
semantic paint layer, compositor layer, retained clip node, scroll container,
or invalidation boundary by itself.

Future interactions between overflow clips and stacking contexts must preserve
the AA6 rule that paint consumes layout-owned clip metadata without inspecting
raw CSS overflow declarations.

## Invalidation Architecture

Current invalidation remains runtime-owned and phase-oriented:

```text
RenderInvalidationEntryPoint
  -> RenderInvalidationRequest
  -> RenderWorkPlan
  -> PendingRenderWork
  -> next frame orchestration
```

AB1 does not add new invalidation entry points. Existing entry points continue
to describe whether style, layout, paint, and frame orchestration rerun
directly or as a cascade from earlier phases.

Future paint invalidation must extend this model rather than bypass it:

- style changes may affect stacking inputs, so they conservatively invalidate
  downstream layout and paint unless a future dependency model proves a smaller
  scope;
- layout changes may affect geometry, tree order, containing relationships,
  clip metadata, and stacking-context placement;
- resource or input changes may affect paint without changing style;
- retained paint state may exist only after explicit ownership, lifetime, and
  invalidation contracts are added;
- targeted repaint must have a conservative fallback when dependency tracking
  is incomplete.

Full-frame paint rerun remains the current baseline. It is not the final
architecture for Milestone AB, and future issues must name the boundary between
full-frame fallback and targeted invalidation.

## Debug And Determinism Expectations

Future stacking and invalidation work must provide deterministic debug surfaces
before being treated as complete. Depending on the implementation issue, those
surfaces may include:

- stacking-context tree snapshots;
- semantic layer ordering snapshots;
- cross-context paint-order snapshots;
- invalidation work-plan or dirty-boundary snapshots;
- retained paint-scene snapshots, if a retained scene is introduced later.

These surfaces must remain backend-independent. Pixel snapshots, egui command
serialization, GPU resources, texture ids, and platform font details are not
acceptable as the only proof of stacking or invalidation correctness.

## Invariants

For a fixed DOM, computed style tree, layout output, viewport, text measurer,
resource state, and input state:

- stacking-context discovery must be deterministic once implemented;
- semantic layer assignment must be deterministic once implemented;
- sibling ordering must preserve layout-owned child order unless an explicit
  stacking rule overrides it;
- paint order must be decided before immediate backend drawing;
- browser/runtime invalidation must request work, not reinterpret paint order;
- overflow clips must remain clip scopes, not stacking contexts;
- compositor and GPU decisions must not leak into semantic paint primitives;
- retained paint artifacts must have explicit ownership and invalidation
  contracts before they exist.

## Deliberate Exclusions

AB1 deliberately does not implement:

- behavioral child stacking-context construction;
- `z-index` parsing or sorting;
- full CSS painting order;
- opacity, transforms, filters, blend modes, or isolation behavior;
- semantic layer tree construction beyond the AB2 root context representation;
- retained display lists;
- retained paint scenes;
- compositor layers;
- GPU layer trees or promotion logic;
- targeted repaint execution;
- dirty-region computation;
- pixel or raster snapshot infrastructure;
- new CSS property support.

These remain excluded because AB1 is the architecture issue. Later issues must
remove exclusions one at a time with code, tests, debug surfaces, and updated
contracts.

AB3 removes the first narrow slice of these exclusions by adding CSS
`z-index: auto | <integer>` and paint-owned child contexts only for positioned
generated boxes with computed integer `z-index`. Full CSS stacking-context and
`z-index` behavior remains excluded.

## Extension Rules For Future AB Issues

Any future issue that implements part of this architecture must state:

1. which AB1 concept it implements;
2. which CSS, layout, paint, or runtime owner provides each input;
3. which existing AA/V exclusion is being removed;
4. which Rust data structure or contract table represents the concept;
5. which deterministic test or debug snapshot proves ordering or invalidation;
6. which compositor, retained-scene, GPU, or targeted-invalidation behavior
   remains deferred.

If a future change cannot answer those points, it is not ready to modify paint
ordering, layering, or invalidation behavior.
