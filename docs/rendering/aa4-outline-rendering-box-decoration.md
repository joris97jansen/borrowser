# AA4: Outline Rendering And Box Decoration

Last updated: 2026-06-10
Status: implemented supported outline subset for Milestone AA issue 4

This document defines Borrowser's first outline rendering subset. It extends
the AA1 paint ordering contract and AA2 paint primitive model without making
outline part of layout, box metrics, retained scene state, or compositor
behavior.

Related code:
- `crates/css/src/properties/types.rs`
- `crates/css/src/properties/data.rs`
- `crates/css/src/specified/outline.rs`
- `crates/css/src/computed/style.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/gfx/src/paint/contracts.rs`
- `crates/layout/src/box_tree/tests/projection.rs`

Related documents:
- `docs/rendering/aa1-paint-model-architecture-ordering-contracts.md`
- `docs/rendering/aa2-paint-primitives-input-model.md`
- `docs/rendering/aa3-border-rendering-box-decoration.md`
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/y4-overflow-semantics-supported-subset.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`

## Supported CSS Surface

AA4 supports only explicit outline longhands:

- `outline-width`
- `outline-style`
- `outline-color`

Supported values are:

- px outline widths;
- `solid` and `none` outline styles;
- explicit supported colors;
- rectangular physical outlines around the border box.

The initial outline state is transparent, `none`, and `0px`. An outline is
paint-visible only when its computed width is greater than zero, its style is
`solid`, and its color has nonzero alpha.

## Ownership

CSS owns parsing, cascade, and computed outline data. Supported outline
longhands enter the property registry, specified-value parser, cascade, and
computed-style builder as ordinary supported properties.

Computed style exposes outline as a separate `Outline` grouping. Outline width
must not be stored in `BoxMetrics`, `BorderEdges`, or any layout-owned metric
structure.

Layout owns the border-box and content-box geometry that paint consumes.
Outline does not affect layout sizing, flow placement, inline layout,
content-box geometry, border-box geometry, margin handling, or box metrics.

Paint owns deterministic outline primitive construction and ordering. Paint
consumes computed outline data on `LayoutBox` plus layout-owned border-box
geometry. It does not emulate outline through border data and does not
recompute layout geometry.

GFX/backend owns low-level drawing. The immediate backend draws the semantic
`PaintOutline` primitive as rectangular filled side strips outside the border
box.

Browser/runtime orchestration does not infer, emulate, schedule specially, or
repair outline behavior.

## Paint Order

The supported per-box paint order is:

1. background
2. border
3. list marker
4. overflow clip for contents and descendants
5. inline formatting primitives
6. child paint nodes in layout child order
7. outline

AA4 introduces post-child paint primitives so the semantic paint model can
represent this order directly. A box's own overflow clip applies to its
contents and descendants; AA4 does not make that own clip suppress the box's
own outline. Ancestor clips still apply through the current clipped painter
execution path.

This remains a supported subset. It does not define full CSS painting order,
stacking-context behavior, compositing, or z-index behavior.

## Geometry

`LayoutBox::rect` remains the authoritative border box. `PaintOutline` stores:

- source identity;
- the layout-owned border rectangle;
- an outer rectangle expanded outward by the outline width;
- outline width;
- outline color.

The outer rectangle is paint-owned semantic data derived from layout geometry
and computed outline data. It must not feed back into layout, hit testing,
sizing, or invalidation.

## Determinism

For a fixed DOM, style tree, viewport, text measurer, resource state, and input
state:

- supported outline properties are resolved through the property registry;
- computed outline data is deterministic and separate from border data;
- layout debug snapshots remain unchanged by outline declarations;
- paint input snapshots expose outline as `PaintPrimitive::Outline`;
- outline primitives are emitted after child paint nodes;
- backend drawing consumes the same outline primitive used by the semantic
  paint model.

## Deliberate Exclusions

AA4 deliberately does not implement:

- `outline` shorthand expansion;
- `outline-offset`;
- `auto`, dashed, dotted, double, inset, outset, groove, or ridge outline
  styles;
- rounded outline geometry;
- text decorations;
- underline, overline, or line-through rendering;
- complex clipping semantics beyond the current supported paint/overflow
  model;
- stacking, compositing, retained display lists, GPU-specific behavior, or
  pixel snapshot testing.
