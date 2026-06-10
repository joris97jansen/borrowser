# AA3: Border Rendering And Box Decoration

Last updated: 2026-06-10
Status: implemented supported border subset for Milestone AA issue 3

This document defines Borrowser's first real border rendering subset. It
extends the AA1 paint ordering contract and AA2 paint primitive model without
introducing shorthand parsing, rounded geometry, compositing, or retained paint
state.

Related code:
- `crates/css/src/properties/types.rs`
- `crates/css/src/properties/data.rs`
- `crates/css/src/specified/border.rs`
- `crates/css/src/computed/style.rs`
- `crates/layout/src/sizing.rs`
- `crates/layout/src/layout_box.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/gfx/src/paint/contracts.rs`

Related documents:
- `docs/rendering/aa1-paint-model-architecture-ordering-contracts.md`
- `docs/rendering/aa2-paint-primitives-input-model.md`
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/y4-overflow-semantics-supported-subset.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`

## Supported CSS Surface

AA3 supports only physical border longhands:

- `border-top-width`
- `border-right-width`
- `border-bottom-width`
- `border-left-width`
- `border-top-style`
- `border-right-style`
- `border-bottom-style`
- `border-left-style`
- `border-top-color`
- `border-right-color`
- `border-bottom-color`
- `border-left-color`

Supported values are:

- px border widths
- `solid` and `none` border styles
- explicit supported colors
- rectangular physical borders

The initial border state is transparent, `none`, and `0px`. A side has a
layout-used border width when its computed width is greater than zero and its
style is `solid`. Color alpha does not affect layout-used border width. A side
is paint-visible only when it has a layout-used border width and its color has
nonzero alpha.

## Ownership

CSS owns parsing, cascade, and computed border data. Supported border longhands
enter the same property registry, specified-value parser, cascade, and
computed-style builder as the rest of the supported property set.

Layout owns used border widths as part of box metrics. Content boxes are
computed from the final border box by subtracting used border widths and
padding. Width and height sizing return content-box used sizes and border-box
sizes that include padding and used border widths. Transparent solid borders
still reserve layout space.

Paint owns deterministic border primitive construction and ordering. Paint
consumes computed border data on `LayoutBox` plus layout-owned border-box
geometry. It does not infer shorthand behavior or recompute layout geometry.

GFX/backend owns low-level drawing. The immediate backend draws physical border
sides from the semantic `PaintBorder` primitive as filled rectangles so
different side widths remain deterministic.

Browser/runtime orchestration does not infer, emulate, schedule specially, or
repair border behavior.

## Paint Order

The supported per-box paint order is:

1. background
2. border
3. list marker
4. overflow clip for contents and descendants
5. inline formatting primitives
6. child paint nodes in layout child order

This remains a supported subset. It does not define full CSS stacking,
compositing, or z-index behavior.

## Layout Geometry

`LayoutBox::rect` remains the final physical border box. For non-anonymous
boxes, `LayoutBox::content_x_and_width()`, `LayoutBox::content_y()`, and
`LayoutBox::content_height()` subtract used border widths and padding.

Anonymous boxes expose zero border metrics. They do not inherit or reinterpret
their anchor node's authored border values.

Overflow clipping remains layout-owned. AA3 does not introduce scrollbars,
scroll offsets, border-radius clipping, or background clipping semantics.

## Determinism

For a fixed DOM, style tree, viewport, text measurer, resource state, and input
state:

- supported border properties are resolved through the property registry;
- used border widths are deterministic computed-style output;
- layout debug snapshots expose border metrics separately from padding;
- paint input snapshots expose border primitives after backgrounds;
- backend drawing consumes the same border primitive used by the semantic paint
  model.

## Deliberate Exclusions

AA3 deliberately does not implement:

- border shorthand expansion;
- `border-width`, `border-style`, or `border-color` shorthands;
- border-radius;
- dashed, dotted, double, inset, outset, groove, or ridge styles;
- border-image;
- logical border properties;
- collapsed table borders;
- outline rendering;
- text decorations;
- background-clip or background-origin expansion;
- inline fragmentation edge cases;
- stacking, compositing, retained display lists, GPU-specific behavior, or
  pixel snapshot testing.
