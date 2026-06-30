# AD7: CSS-Owned Invalidation Impact Classification

Last updated: 2026-06-29
Status: implemented contract for Milestone AD issue 7

This document defines Borrowser's CSS-owned invalidation impact classification
for the current supported longhand property subset.

AD7 replaces AD4's narrow `repaint-only` versus `relayout-and-repaint`
metadata with composable CSS-owned flags. Browser/runtime may consume a
narrow computed-style invalidation projection, but it must not infer CSS
property meaning from property names, declaration text, or a local
`PropertyId` table.

AD7 does not add new CSS properties, selector dependency invalidation,
descendant dependency graphs, media/container query invalidation, custom
properties, animations, compositor layers, GPU behavior, transform/opacity
support, dirty-region rendering, or broad CSS conformance.

Related code:

- `crates/css/src/properties/types.rs`
- `crates/css/src/properties/data.rs`
- `crates/css/src/properties/registry.rs`
- `crates/css/src/computed/impact.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/rendering/tests/runtime_state.rs`

Related documents:

- `docs/css/ad1-css-value-property-ownership-architecture.md`
- `docs/css/ad4-css-property-registry-longhand-metadata.md`
- `docs/css/ad5-specified-computed-value-boundaries.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/rendering/ac3-explicit-dirty-state-tracking.md`
- `docs/rendering/ac4-deterministic-render-work-plans.md`
- `docs/rendering/ac10-retained-rendering-runtime-closeout.md`
- `docs/engine-feature-gap-tracker.md`

## Ownership

CSS owns:

- the invalidation impact flags attached to supported longhand metadata;
- the conservative classification for each supported property;
- comparison of computed styles;
- projection from property impact flags to the narrow runtime-facing
  invalidation result.

Browser/runtime owns:

- retained dirty-state lifetime;
- retained style/layout/paint generation counters;
- render work planning;
- conservative fallback when CSS reports unknown document shape or
  layout-affecting impact.

Browser/runtime may consume only the CSS-owned computed-document impact result:

```text
ComputedDocumentStyle::invalidation_impact_against(previous)
```

Browser/runtime must not inspect `PropertyId`, property names, registry flags,
authored CSS, or computed values to define CSS property impact.

## Impact Flags

`PropertyInvalidationImpact` is a composable flags struct. Raw registry
metadata contains only positive CSS impact facts. Projection results such as
`NoVisualImpact` and `StyleOnly` are derived by the computed-style comparison
layer, not stored as raw longhand metadata flags. Flags are serialized in
deterministic canonical order in the registry metadata debug snapshot.

| flag | meaning | owner | consumers | intentionally not implemented |
| --- | --- | --- | --- | --- |
| `inherited-style` | the property participates in inherited computed-style propagation | CSS | CSS classification and computed comparison | full descendant invalidation/dependency graphs |
| `box-tree` | the property can change generated box-tree structure or display participation | CSS | runtime projection as layout-affecting | targeted box-tree invalidation |
| `layout` | the property can affect layout inputs, geometry, or retained layout validity | CSS | runtime projection as layout-affecting | subtree/minimal relayout execution |
| `text-metrics` | the property can affect text measurement or line layout | CSS | runtime projection as layout-affecting | full font system or advanced line layout |
| `paint` | the property can affect paint primitives or visual output | CSS | runtime projection as paint work | dirty-region rendering |
| `paint-order` | the property can affect stacking or paint ordering | CSS | runtime projection as paint work; conservative layout when still required | retained paint-order dependency invalidation |
| `overflow-clip` | the property can affect overflow policy, clipping, or related layout/paint inputs | CSS | runtime projection as layout-affecting | overflow-x/y split, scroll offsets, scrollbars, viewport/body propagation |
| `future-compositor` | metadata hook for future compositor-relevant properties | CSS | none in current runtime | compositor, GPU, transforms, opacity, animations |

The `conservative` marker is not a separate engine feature. It records that the
classification intentionally widens current runtime work because the engine
does not yet have narrower safe retained dependency data.

## Supported Longhand Classification

AD7 classifies only currently supported longhands:

| longhands | impact |
| --- | --- |
| `background-color`, border color longhands, outline longhands, `text-decoration-line` | `paint` |
| `color` | `inherited-style+paint` |
| border style and width longhands, `height`, margin longhands, `max-width`, `min-width`, padding longhands, `width` | `layout+paint` |
| `display` | `box-tree+layout+paint` |
| `font-size` | `inherited-style+layout+text-metrics+paint` |
| `overflow` | `layout+paint+overflow-clip` |
| `position` | `layout+paint+paint-order` |
| `z-index` | `layout+paint+paint-order+conservative` |

`z-index` remains conservatively layout-affecting in the runtime projection.
The property is paint-order relevant, but current retained runtime contracts do
not yet provide a narrower paint-order invalidation path that can safely skip
layout in every supported case.

## Runtime Projection

`crates/css/src/computed/impact.rs` compares computed styles in canonical
property order and projects changed property flags into:

| projection | meaning | source |
| --- | --- | --- |
| `NoVisualImpact` | no changed property has current downstream runtime-relevant impact | derived from absence of positive runtime impact |
| `StyleOnly` | a changed property affects CSS style state but not current runtime layout or paint work | derived from positive CSS metadata such as `inherited-style` when it is not paired with layout or paint impact; no current supported longhand projects this way |
| `PaintOnly` | changed properties require paint work but not layout work | derived from positive paint-related metadata without runtime layout impact |
| `LayoutAffecting` | changed properties require layout work, and paint dirtiness cascades from layout | derived from positive layout, box-tree, text-metrics, or overflow/clip metadata |
| `Unknown` | CSS cannot compare the document styles safely | derived from document-shape mismatch |

`Unknown` is reserved for document-shape mismatches, such as different element
counts or selector element identities. Browser/runtime treats
`LayoutAffecting` and `Unknown` conservatively.

For paint-only style changes, Browser/runtime may remove style-derived layout
dirty entries and mark paint dirty through `PaintOnlyStyleChanged`. Existing
unrelated layout dirtiness, such as viewport dirtiness, is preserved.

For derived no-visual or style-only changes, Browser/runtime may remove
style-derived layout and paint cascades. AD7 does not currently classify any
supported longhand this way.

## Debug And Test Surface

`property_registry_metadata_debug_snapshot()` emits the canonical impact flags
for every supported longhand. The fixture
`crates/css/tests/fixtures/properties/registry_metadata.snap` is an internal
regression contract, not CSSOM.

Targeted CSS tests verify:

- every supported longhand has explicit impact metadata;
- representative flags such as inherited style, box tree, text metrics,
  overflow clip, paint order, and conservative impact are present;
- computed-style comparison projects property flags deterministically;
- unknown document shape stays conservative.

AD7 follow-up coverage treats the supported longhand source of truth as the
relationship between `PropertyId::ALL`, `property_registry().entries()`,
registry lookup data, and supported shorthand outputs. Tests must fail when a
supported longhand is added without a registry registration, lookup entry,
explicit `PropertyInvalidationImpact`, or deterministic debug snapshot impact
label.

Shorthands remain outside the supported longhand registry. A supported
shorthand is valid only by expanding into registered longhands, and those
longhands must carry the same explicit impact metadata as directly authored
longhand declarations.

Browser/runtime tests verify:

- paint-only CSS-owned impact can avoid relayout through the real retained
  style recomputation path;
- unrelated layout dirtiness is preserved when a paint-only style change is
  narrowed;
- paint-order changes such as `z-index` remain conservatively layout-affecting
  until narrower retained paint-order invalidation exists.

## Invariants

- CSS remains the sole owner of property invalidation semantics.
- The supported longhand registry is the canonical metadata source.
- Every supported longhand has explicit invalidation impact metadata.
- Impact flag debug labels are deterministic.
- Browser/runtime consumes only the CSS-owned computed-document projection.
- Browser/runtime does not maintain a CSS property-impact table.
- Conservative classifications are visible and documented.
- Unsupported properties remain unsupported and are not added as placeholders.

## Deliberate Exclusions

AD7 deliberately excludes:

- new supported CSS properties;
- full known-property modeling for unsupported CSS;
- selector dependency invalidation;
- descendant dependency graphs;
- media or container query invalidation;
- custom properties and `var(...)`;
- animations and transitions;
- transforms, opacity, filters, and compositor/GPU behavior;
- dirty-region rendering;
- targeted relayout;
- retained paint-order dependency graphs;
- broad CSS conformance work.

Future property work should classify impact when a supported longhand enters
the registry. If a property's exact impact is not yet proven by layout, paint,
and runtime contracts, the classification must remain conservative and explain
why.
