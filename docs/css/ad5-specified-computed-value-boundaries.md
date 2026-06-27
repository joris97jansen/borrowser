# AD5: Specified And Computed Value Boundaries

Last updated: 2026-06-27
Status: implemented contract for Milestone AD issue 5

This document defines Borrowser's current specified-value and computed-value
boundaries for the supported longhand subset. It does not describe complete CSS
specification coverage.

AD5 formalizes existing typed CSS behavior and adds a deterministic inventory
surface for review. It does not add new property coverage, shorthands, used
values, actual values, layout-dependent percentage resolution, full cascade
conformance, custom properties, media queries, animations, font shaping, CSSOM,
or runtime invalidation taxonomy changes.

Related code:

- `crates/css/src/properties`
- `crates/css/src/properties/boundary.rs`
- `crates/css/src/specified`
- `crates/css/src/cascade`
- `crates/css/src/computed`
- `crates/layout/src`
- `crates/gfx/src/paint`
- `crates/browser/src/rendering`

Related documents:

- `docs/css/ad1-css-value-property-ownership-architecture.md`
- `docs/css/ad2-typed-core-css-value-model.md`
- `docs/css/ad3-css-wide-keyword-handling.md`
- `docs/css/ad4-css-property-registry-longhand-metadata.md`
- `docs/css/s3-parsed-specified-value-representations.md`
- `docs/css/s4-computed-value-representations-normalization.md`
- `docs/css/s6-computed-style-assembly-pipeline.md`
- `docs/css/s7-structured-computed-style-representation.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/aa1-paint-model-architecture-ordering-contracts.md`
- `docs/engine-feature-gap-tracker.md`

## Purpose

Borrowser must keep parser output, specified values, computed values, layout
used values, and future actual values separate.

AD5 makes the current boundary explicit for every supported longhand. Future
CSS property work must extend these CSS-owned representations and tests rather
than relying on parser-shaped data in Layout, Paint/GFX, or Browser/runtime.

## Value Lifecycle

| stage | owner | representative surface | meaning |
| --- | --- | --- | --- |
| parser/model values | CSS syntax/model | `CssComponentValue`, `DeclarationValue`, `ValueComponent` | Syntax-derived authored value components. They are not property-valid runtime values. |
| specified values | CSS specified/cascade | `SpecifiedDeclarationValue`, `SpecifiedPropertyValue`, `SpecifiedValue` | Property-aware values accepted by the supported property grammar after validation. CSS-wide keywords are declaration values resolved by cascade before computed materialization. |
| computed values | CSS computed | `ComputedValue`, `ComputedStyle`, `ComputedDocumentStyle` | CSS-owned normalized runtime handoff values. Percentages that need a layout basis remain represented as computed inputs. |
| used values | future Layout | layout-owned sizing, positioning, and geometry data | Values after layout applies containing blocks, formatting context rules, intrinsic sizing, line construction, and layout constraints. AD5 does not implement this stage. |
| actual values | future Layout/Paint/backend | backend/device-constrained paint or layout output | Values after device, backend, rasterization, compatibility, or compositor constraints. AD5 does not implement this stage. |

Debug labels, snapshots, and golden fixtures serialize typed state for
inspection. They are not the value boundary and must not be used as the
implementation path for specified-to-computed behavior.

## Ownership Boundary

CSS owns:

- supported property identity and metadata;
- specified-value parsing and validation;
- CSS-wide keyword resolution for supported declarations;
- initial and inherited-by-default behavior;
- specified-to-computed normalization;
- computed-value and computed-style construction;
- deterministic value-boundary debug inventories.

Layout may consume typed computed layout-relevant values and owns used
geometry, containing-size resolution, formatting contexts, line construction,
and layout artifacts. Layout must not parse CSS declarations, inspect parser
values, or duplicate CSS property metadata.

Paint/GFX may consume typed computed paint-relevant values plus layout output
and owns visual primitives, paint ordering, clipping execution, backend
painting, and future actual/backend-constrained results. Paint/GFX must not
parse CSS declarations, recover invalid CSS, or maintain a private CSS
property-meaning table.

Browser/runtime may consume CSS-owned computed artifacts and CSS-owned impact
classifications. Browser/runtime owns scheduling, retained state, dirty state,
and render work planning; it must not own CSS property semantics.

## Boundary Inventory Surface

AD5 adds `property_value_boundaries()` and
`property_value_boundary_debug_snapshot()` in `css::properties`.

The inventory is derived from `PropertyMetadata` and the canonical property
registry. It is not a second property registry and does not normalize values.
Its conversion rule is a narrow classification for documentation and tests:

- `color-to-rgba`
- `keyword-to-computed-enum`
- `absolute-length-to-css-px`
- `length-percentage-or-auto-preserving-percentages`
- `length-percentage-or-none-preserving-percentages`
- `z-index-auto-or-integer`

The actual normalization path remains:

```text
SpecifiedPropertyValue
  -> normalize_specified_value(...)
  -> ComputedValue
  -> ComputedStyleBuilder
  -> ComputedStyle
```

## Supported Longhand Matrix

This matrix describes Borrowser's currently supported subset only. Deferred
items are not implemented or approximated by AD5.

| property id | property name | specified value | computed value | inherited by default | initial behavior | conversion rule | intentionally deferred |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `BackgroundColor` | `background-color` | `Color` | `AbsoluteColor` | no | `transparent` to RGBA transparent | color to RGBA | background images, multiple backgrounds, actual backend color constraints |
| `BorderBottomColor` | `border-bottom-color` | `Color` | `AbsoluteColor` | no | `transparent` to RGBA transparent | color to RGBA | broad border syntax, border images, actual paint/backend values |
| `BorderBottomStyle` | `border-bottom-style` | `BorderStyleKeyword` | `BorderStyleKeyword` | no | `none` | keyword to computed enum | border styles beyond current `none`/`solid` subset |
| `BorderBottomWidth` | `border-bottom-width` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px | layout used border geometry beyond current supported subset |
| `BorderLeftColor` | `border-left-color` | `Color` | `AbsoluteColor` | no | `transparent` to RGBA transparent | color to RGBA | broad border syntax, border images, actual paint/backend values |
| `BorderLeftStyle` | `border-left-style` | `BorderStyleKeyword` | `BorderStyleKeyword` | no | `none` | keyword to computed enum | border styles beyond current `none`/`solid` subset |
| `BorderLeftWidth` | `border-left-width` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px | layout used border geometry beyond current supported subset |
| `BorderRightColor` | `border-right-color` | `Color` | `AbsoluteColor` | no | `transparent` to RGBA transparent | color to RGBA | broad border syntax, border images, actual paint/backend values |
| `BorderRightStyle` | `border-right-style` | `BorderStyleKeyword` | `BorderStyleKeyword` | no | `none` | keyword to computed enum | border styles beyond current `none`/`solid` subset |
| `BorderRightWidth` | `border-right-width` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px | layout used border geometry beyond current supported subset |
| `BorderTopColor` | `border-top-color` | `Color` | `AbsoluteColor` | no | `transparent` to RGBA transparent | color to RGBA | broad border syntax, border images, actual paint/backend values |
| `BorderTopStyle` | `border-top-style` | `BorderStyleKeyword` | `BorderStyleKeyword` | no | `none` | keyword to computed enum | border styles beyond current `none`/`solid` subset |
| `BorderTopWidth` | `border-top-width` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px | layout used border geometry beyond current supported subset |
| `Color` | `color` | `Color` | `AbsoluteColor` | yes | root/no-parent uses `black`; descendants inherit when unspecified | color to RGBA | full named color set, color functions, actual text paint constraints |
| `Display` | `display` | `DisplayKeyword` | `DisplayKeyword` | no | `inline` | keyword to computed enum | broad display modes and complete formatting behavior |
| `FontSize` | `font-size` | `AbsoluteLength` | `AbsoluteLength` | yes | root/no-parent uses `16px`; descendants inherit when unspecified | px/unitless-zero to CSS px | relative units, font metrics, font shaping, line-height model |
| `Height` | `height` | `LengthPercentageOrAuto` | `LengthPercentageOrAuto` | no | `auto` | preserve `auto`; convert lengths to CSS px; preserve percentages for Layout | used height resolution and layout-dependent percentage resolution |
| `MarginBottom` | `margin-bottom` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px; negative values allowed by metadata | margin collapsing and used geometry rules beyond current subset |
| `MarginLeft` | `margin-left` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px; negative values allowed by metadata | used geometry rules beyond current subset |
| `MarginRight` | `margin-right` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px; negative values allowed by metadata | used geometry rules beyond current subset |
| `MarginTop` | `margin-top` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px; negative values allowed by metadata | margin collapsing and used geometry rules beyond current subset |
| `MaxWidth` | `max-width` | `LengthPercentageOrNone` | `LengthPercentageOrNone` | no | `none` | preserve `none`; convert lengths to CSS px; preserve percentages for Layout | used max-size resolution and layout-dependent percentage resolution |
| `MinWidth` | `min-width` | `LengthPercentageOrAuto` | `LengthPercentageOrAuto` | no | `auto` | preserve `auto`; convert lengths to CSS px; preserve percentages for Layout | used min-size resolution and layout-dependent percentage resolution |
| `Overflow` | `overflow` | `OverflowKeyword` | `OverflowKeyword` | no | `visible` | keyword to computed enum | overflow-x/y split, scroll containers, scrollbars, viewport/body propagation |
| `OutlineColor` | `outline-color` | `Color` | `AbsoluteColor` | no | `transparent` to RGBA transparent | color to RGBA | `currentcolor`, actual paint/backend values |
| `OutlineStyle` | `outline-style` | `OutlineStyleKeyword` | `OutlineStyleKeyword` | no | `none` | keyword to computed enum | `auto` and styles beyond current `none`/`solid` subset |
| `OutlineWidth` | `outline-width` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px | outline offset, rounded outline geometry, actual paint/backend values |
| `PaddingBottom` | `padding-bottom` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px | percentage padding and used geometry rules beyond current subset |
| `PaddingLeft` | `padding-left` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px | percentage padding and used geometry rules beyond current subset |
| `PaddingRight` | `padding-right` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px | percentage padding and used geometry rules beyond current subset |
| `PaddingTop` | `padding-top` | `AbsoluteLength` | `AbsoluteLength` | no | `0px` | px/unitless-zero to CSS px | percentage padding and used geometry rules beyond current subset |
| `Position` | `position` | `PositionKeyword` | `PositionKeyword` | no | `static` | keyword to computed enum | full positioned used geometry and containing-block behavior |
| `TextDecorationLine` | `text-decoration-line` | `TextDecorationLineKeyword` | `TextDecorationLineKeyword` | no | `none` | keyword to computed enum | shorthand, overline, line-through, style/color/thickness, skip-ink |
| `Width` | `width` | `LengthPercentageOrAuto` | `LengthPercentageOrAuto` | no | `auto` | preserve `auto`; convert lengths to CSS px; preserve percentages for Layout | used width resolution and layout-dependent percentage resolution |
| `ZIndex` | `z-index` | `ZIndex` | `ZIndex` | no | `auto` | preserve `auto`; preserve supported integer values | full stacking/compositing behavior and compositor-layer semantics |

## Initial And Inherited Materialization

Cascade owns source selection:

1. a valid winning specified declaration;
2. an inherited source when metadata says the property inherits and a parent
   style exists;
3. the metadata initial value.

Computed style owns typed materialization of that source:

- winners normalize through `normalize_specified_value(...)`;
- inherited entries copy the parent computed value for the same property;
- initial entries convert `InitialStyleValue` into the property-specific
  `ComputedValue` through `ComputedValue::from_initial(...)`.

The computed layer validates that the materialized value kind matches
`PropertyMetadata::computed_value`.

## Debug And Regression Surfaces

AD5 debug surfaces are internal regression contracts:

- `property_value_boundary_debug_snapshot()` lists every supported longhand
  boundary in registry order.
- `computed_value_debug_snapshot(...)` prints specified and computed contracts,
  the AD5 conversion classification, and the result of parsing/normalization
  for one authored declaration value.
- `ComputedStyle::to_debug_snapshot()` serializes total computed style entries
  in registry order.

These surfaces must stay deterministic, but production behavior must continue
to flow through typed values, not through string labels or snapshots.

## Invariants

- `PropertyMetadata` remains the source of truth for property name, inheritance,
  initial value, specified kind, computed kind, invalid-value policy,
  length-sign policy, and AD4 invalidation impact.
- The AD5 boundary inventory is derived from the registry and does not replace
  registry metadata.
- Every supported longhand has exactly one specified/computed boundary row.
- Specified parsing consumes model-layer declaration values, not serialized
  CSS strings.
- Computed normalization consumes `SpecifiedPropertyValue`, not parser tokens
  or debug text.
- Layout consumes typed computed values and owns used geometry.
- Paint/GFX consumes typed computed paint values plus layout output and owns
  visual/backend output.
- Browser/runtime consumes CSS-owned artifacts and impact facts without
  duplicating CSS property meaning.

## Deliberate Exclusions

AD5 deliberately excludes:

- used-value resolution;
- actual-value resolution;
- layout-dependent percentage resolution;
- full cascade conformance;
- shorthand expansion;
- custom properties and `var(...)`;
- media queries and container queries;
- animations and transitions;
- font shaping and full typography;
- CSSOM;
- broader property coverage;
- runtime invalidation taxonomy changes beyond preserving AD4 behavior.
