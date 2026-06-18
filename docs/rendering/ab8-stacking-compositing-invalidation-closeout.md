# AB8: Stacking, Compositing Semantics, And Invalidation Close-Out

Last updated: 2026-06-18
Status: implemented close-out contract for Milestone AB issue 8

This document closes Milestone AB by consolidating the stacking, semantic
layering, paint invalidation, repaint execution, debug surface, invariant, and
extension-point model implemented by AB1 through AB7.

AB8 does not introduce new rendering behavior, Rust APIs, debug snapshot
formats, CSS support, layout behavior, compositor layers, GPU concepts,
retained display lists, retained paint scenes, dirty regions, animations, or
broader CSS stacking behavior. Detailed source contracts remain in AB1 through
AB7; this document is the close-out map for the supported Milestone AB subset.

Related code:
- `crates/gfx/src/paint/stacking.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/contracts.rs`
- `crates/gfx/src/paint/debug.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/browser/src/rendering/types.rs`
- `crates/browser/src/rendering/invalidation.rs`
- `crates/browser/src/rendering/debug.rs`
- `crates/browser/src/rendering/frame.rs`
- `crates/gfx/src/viewport.rs`
- `crates/css/src/specified/z_index.rs`
- `crates/css/src/specified/position.rs`
- `crates/css/src/computed/style.rs`

Detailed source contracts:
- `docs/rendering/ab1-stacking-layering-invalidation-architecture.md`
- `docs/rendering/ab2-stacking-context-representation.md`
- `docs/rendering/ab3-z-order-layering-semantics.md`
- `docs/rendering/ab4-stacking-context-paint-order.md`
- `docs/rendering/ab5-structured-paint-invalidation-model.md`
- `docs/rendering/ab6-basic-targeted-repaint-behavior.md`
- `docs/rendering/ab7-deterministic-debug-regression-coverage.md`
- `docs/rendering/aa9-paint-model-invariants-extension-points.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`

## Milestone AB Close-Out Scope

Milestone AB moves Borrowser's rendering pipeline from the AA paint model's
deterministic per-box traversal toward explicit stacking and invalidation
foundations:

- a paint-owned, frame-local `StackingContextTree`;
- deterministic root stacking context identity for one frame;
- a narrow child stacking-context subset for positioned generated boxes with
  computed integer `z-index`;
- paint-owned semantic z-order layer buckets;
- one canonical `StackingContextTree::ordered_slots(...)` path shared by
  order snapshots, operation snapshots, layering snapshots, and immediate
  painting;
- structured paint invalidation requests derived from runtime render
  invalidation entry points;
- conservative `Document` and `Viewport` paint invalidation and repaint
  execution scopes;
- deterministic debug surfaces for stacking, layering, invalidation, repaint
  planning, and frame orchestration.

AB is complete only for this supported subset. It does not make Borrowser's
painting model CSS-complete, and it does not introduce a retained scene graph,
display list, compositor, GPU pipeline, or dirty-region repaint system.

## Ownership Boundaries

CSS owns authored and computed style inputs:

- parsing and computing `position`;
- parsing and computing `z-index: auto | <integer>`;
- exposing `ComputedStyle::position()` and `ComputedStyle::z_index()`.

CSS does not decide stacking-context membership, paint order, semantic layer
assignment, repaint scope, compositor promotion, retained scene lifetime, or
backend execution.

Layout owns semantic geometry and paint-order inputs:

- generated layout box identity;
- deterministic layout tree order;
- final box geometry;
- positioning metadata;
- inline fragment order and geometry;
- replaced-element metadata;
- overflow policy and overflow clip metadata.

Layout does not sort paint primitives, create stacking contexts, construct
compositor layers, retain paint artifacts, or decide invalidation entry points.

Paint/GFX owns the current AB stacking and paint-order model:

- building frame-local `PaintInput` and `StackingContextTree` values from
  layout output;
- assigning stackable paint items to paint-owned contexts and semantic layer
  buckets;
- resolving cross-context paint order through
  `StackingContextTree::ordered_slots(...)`;
- constructing semantic paint primitives;
- serializing paint-owned stacking, layering, order, and operation debug
  snapshots;
- producing immediate paint output for the current backend path.

GFX/backend owns low-level immediate drawing execution. Backend clip objects,
textures, painter state, and future GPU resources are execution details, not
semantic stacking or compositing identities.

Browser/runtime owns invalidation and repaint orchestration:

- runtime render invalidation entry points;
- retained page/render state;
- pending render work;
- paint invalidation derivation from render invalidation requests;
- repaint execution planning;
- frame orchestration debug output.

Browser/runtime does not reinterpret stacking, semantic layer assignment, or
z-order. It may request paint work and choose conservative repaint execution
scope, but it must not decide which paint source appears above another.

## Supported Stacking Model

The supported AB stacking representation is frame-local and paint-owned:

```text
LayoutPhaseOutput
  -> PaintPhaseInput
  -> PaintInput
  -> StackingContextTree
  -> StackingOrderSlot sequence
  -> semantic snapshots and immediate painting
```

`StackingContextId::ROOT` identifies the root stacking context inside one
`StackingContextTree`. `StackingContextId` values are assigned deterministically
for a frame-local paint input. They are not retained scene IDs, compositor layer
IDs, invalidation keys, backend resources, GPU handles, or stable identities
across frames.

The supported child-context trigger is deliberately narrow:

- the layout box is a generated paintable box;
- the layout-owned positioning scheme is not `static`;
- the computed `z-index` is an integer.

`z-index: auto` does not create a child stacking context in AB. An integer
`z-index` on a static generated box is computed CSS data, but it has no AB
paint-order effect.

Overflow clips remain layout-owned clip metadata consumed by paint. An overflow
clip is not a stacking context, semantic paint layer, compositor layer,
retained clip node, scroll container, or invalidation boundary.

## Supported Layering And Z-Order Rules

AB uses paint-owned semantic paint layers inside each stacking context:

1. negative integer `z-index` child contexts;
2. the context source subtree;
3. zero integer `z-index` child contexts;
4. positive integer `z-index` child contexts.

Child contexts inside a z-index bucket are ordered by:

1. semantic layer;
2. integer `z-index` value;
3. stable layout preorder.

The context source subtree preserves the supported AA per-box order:

1. box background;
2. box border;
3. list marker;
4. overflow clip for contents and descendants;
5. inline formatting content;
6. in-context child subtrees in layout child order;
7. box outline.

Child stacking contexts are atomic relative to sibling parent-context content.
If a child layout box starts a different stacking context, that child root is
skipped during the parent's source-subtree traversal and emitted only through
its explicit `ChildContext` slot.

## Compositing Semantics

In AB, "compositing semantics" means the semantic ordering model that a future
compositor can build on:

- paint-owned stacking contexts;
- paint-owned semantic z-order layers;
- deterministic child-context ordering;
- explicit source-subtree versus child-context slots;
- backend-independent debug surfaces that prove the semantic order.

These semantic layers are not compositor layers. They are not retained scene
nodes, display lists, GPU layers, textures, command buffers, damage regions, or
backend partial-raster instructions.

A future compositor milestone may decide that some semantic paint data should
feed retained scene nodes or compositor layers. That future work must introduce
separate ownership, lifetime, invalidation, fallback, debug, and test
contracts. It must not reinterpret AB's frame-local stacking context IDs as
retained compositor identities.

## Paint Invalidation And Repaint Execution

Runtime invalidation enters through the existing rendering model:

```text
RenderInvalidationEntryPoint
  -> RenderInvalidationRequest
  -> RenderWorkPlan
  -> PendingRenderWork
```

AB5 derives structured paint invalidation from that runtime queue:

```text
PendingRenderWork
  -> PaintInvalidationRequest
  -> PendingPaintInvalidations
  -> conservative effective paint scope
```

The supported paint invalidation scopes are:

- `Document`: conservative full-document paint invalidation;
- `Viewport`: conservative visible-viewport paint invalidation.

Full-document repaint is represented explicitly as `Document`, not as missing
invalidation structure. `Viewport` is a supported conservative repaint scope,
not arbitrary dirty-region, per-node, paint-source, stacking-context, or
compositor-layer invalidation.

AB6 maps pending paint invalidations and synthesized viewport changes into a
runtime-owned `RepaintExecutionPlan`, then into a GFX-facing
`ViewportRepaintPolicy`. GFX may clip immediate execution for a viewport-scope
repaint, but it does not infer invalidation causes from DOM, style, layout,
paint primitives, or backend draw operations.

## Debug And Regression Surfaces

AB's debug surfaces are internal deterministic regression contracts:

- `PaintInput::to_stacking_context_debug_snapshot()`;
- `PaintInput::to_layering_debug_snapshot()`;
- `PaintInput::to_order_debug_snapshot()`;
- `PaintInput::to_operation_debug_snapshot()`;
- `browser::rendering::paint_invalidation_debug_snapshot(...)`;
- `RenderFrameExecutionTrace::to_debug_snapshot()`.

Paint-side snapshots consume paint-owned stacking order and semantic paint
data. Runtime-side snapshots consume runtime-owned pending work, paint
invalidation, repaint planning, and frame orchestration data.

These snapshots are not public APIs, retained display lists, retained scenes,
backend command streams, compositor state, GPU state, pixel screenshots, or
raster comparisons.

## Invariants

For a fixed DOM, computed style tree, layout output, viewport, text measurer,
resource state, input state, and pending render work:

- stacking-context construction is deterministic;
- `StackingContextId::ROOT` exists in every frame-local stacking tree;
- `StackingContextId` values are frame-local and are not retained identities;
- child contexts are created only for the supported positioned integer
  `z-index` trigger;
- `z-index: auto` and static integer `z-index` boxes remain in the source
  subtree order;
- same-layer and same-`z-index` ties resolve by stable layout preorder;
- child stacking contexts are atomic relative to sibling parent-context
  content;
- `StackingContextTree::ordered_slots(...)` is the canonical cross-context
  paint-order source;
- order snapshots, layering snapshots, operation snapshots, and immediate
  painting stay aligned with that canonical slot path;
- overflow clips remain clip scopes and still apply across child-context
  emission;
- paint invalidation is derived from explicit runtime invalidation requests;
- effective repaint scope selection is deterministic and conservative;
- `Document` remains the conservative repaint fallback;
- `Viewport` remains a conservative supported scope, not dirty-region repaint;
- browser/runtime invalidation does not reinterpret paint order;
- paint and GFX/backend do not own pending runtime invalidation state;
- no frame-local paint, layout, or stacking identifiers are retained as runtime
  invalidation keys;
- debug snapshots use stable labels, fixed field ordering, and
  backend-independent semantic data.

## Deliberate Exclusions

Milestone AB deliberately excludes:

- full CSS painting order;
- full CSS stacking-context trigger set;
- `z-index` behavior beyond the positioned integer subset;
- full positioned layout geometry for absolute, fixed, and sticky positioning;
- opacity-created stacking contexts;
- transform-created stacking contexts;
- filter and backdrop-filter stacking behavior;
- perspective and 3D transform stacking behavior;
- mix-blend-mode, isolation, blending, and advanced compositing behavior;
- containment and will-change-related stacking or compositing behavior;
- inline, float, table, flex, grid, and pseudo-element painting-order
  interactions beyond the currently supported paint/layout subset;
- top layer behavior;
- masks, clip-path, border-radius clipping, and advanced clipping;
- compositor layers;
- GPU layers, GPU promotion, or a GPU pipeline;
- retained display lists;
- retained paint scenes;
- arbitrary dirty rectangles;
- minimal dirty-region propagation;
- paint-source-scoped repaint;
- stacking-context-scoped repaint;
- compositor-layer invalidation;
- per-node repaint;
- dependency graphs from DOM/style/layout nodes to paint artifacts;
- animations and transitions;
- backend partial raster or partial repaint execution;
- backend command serialization as a semantic contract;
- pixel, raster, or screenshot regression infrastructure.

Future issues may remove individual exclusions only by defining explicit
ownership, stable identifiers where retention is needed, dependency derivation,
conservative fallback behavior, deterministic debug output, and representative
tests.

## Future Extension Points

AB establishes the following attachment points for future milestones:

- broader stacking triggers: extend CSS/computed inputs and paint-owned context
  construction without moving ordering into CSS or layout;
- complete CSS painting order: extend paint-owned semantic ordering while
  preserving layout-owned geometry and tree-order inputs;
- retained display lists or retained paint scenes: introduce browser/runtime
  ownership, retained artifact lifetime, invalidation, and debug contracts
  before retaining paint artifacts;
- compositor layers: define compositor ownership separately from semantic
  paint layers and immediate paint output;
- GPU acceleration: introduce backend/resource ownership without storing GPU
  state in semantic paint primitives or debug snapshots;
- incremental invalidation: extend AB5/AB6 through explicit runtime
  invalidation contracts, stable dependency data, and conservative fallbacks;
- animation-driven invalidation: add animation timing and style/layout/paint
  dependency contracts before scheduling animation repaint work;
- pixel or raster visual regression: add a platform-aware determinism contract
  separate from AB7 semantic snapshots.

Any future extension must state which subsystem owns the new semantics, which
existing contract changes, which debug surface proves determinism, which
fallback preserves correctness when dependency data is incomplete, and which
AB exclusion is deliberately removed.

## Close-Out Rule

Milestone AB is complete and unambiguous because the supported subset now has
explicit architecture, stacking-context representation, z-order and layering
rules, shared paint-order execution, structured paint invalidation, conservative
repaint execution, deterministic debug surfaces, invariants, limitations, and
future attachment points.

That completion is scoped to the documented supported subset only. Future work
must extend the AB model through explicit contracts rather than treating AB8 as
evidence that unsupported compositor, GPU, retained-scene, dirty-region,
animation, or full CSS stacking behavior already exists.
