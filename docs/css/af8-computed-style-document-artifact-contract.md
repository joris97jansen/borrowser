# AF8: Computed-Style Document Artifact Contract

Status: implemented conformance closure for Milestone AF issue 8

Last updated: 2026-08-24

AF8 makes the existing CSS-owned computed document output the authoritative
style input for rendering. It closes the supported path from AF7 resolved
sources through computed materialization and removes the remaining Paint-side
HTML visibility override. It does not broaden Borrowser's CSS property or value
coverage.

## Related code

- `crates/css/src/properties`
- `crates/css/src/computed`
- `crates/browser/src/document_style.rs`
- `crates/browser/src/page/style_cache.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/layout/src`
- `crates/gfx/src/paint`

## Related contracts

- AD4: `docs/css/ad4-css-property-registry-longhand-metadata.md`
- AD5: `docs/css/ad5-specified-computed-value-boundaries.md`
- AF1: `docs/css/af1-selector-cascade-computed-style-architecture-contract.md`
- AF7: `docs/css/af7-specified-value-defaulting-source-resolution.md`
- S9: `docs/css/s9-property-system-computed-style-runtime-contract.md`
- U1: `docs/css/u1-runtime-integration-architecture-css-pipeline-ownership.md`
- AC5: `docs/rendering/ac5-retained-style-artifact-reuse.md`
- V1: `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`

## Authoritative document path

The production path is:

```text
DOM + UA/author/inline stylesheet inputs
  -> cascade / ResolvedDocumentStyle
  -> computed materialization / ComputedDocumentStyle
  -> exact borrow-backed StyledNode / StylePhaseOutput
  -> Layout used values and geometry
  -> Paint primitives and backend output
```

`ComputedDocumentStyle` is the authoritative CSS-owned element-style artifact.
It contains one `ComputedElementStyle` for every selector-visible element in
canonical document order. Each entry retains selector element identity,
namespace, local name, and a total `ComputedStyle`.

Browser/runtime may retain the resolved and computed artifacts, but it does not
own their property meaning. `build_style_tree_from_computed_styles(...)`
validates the current DOM projection against the retained entries and copies
each entry's `ComputedStyle` exactly into the borrow-backed `StyledNode` view.
It does not read or update the legacy DOM-attached declaration vector.

## Canonical initial construction

AD4 `PropertyMetadata.initial` is the only semantic source for supported
property initial values. Initial computed style construction is:

```text
PropertyMetadata.initial
  -> PropertyId::initial_value()
  -> ComputedValue::from_initial(property)
  -> ComputedStyleBuilder::record(...)
  -> ComputedStyleBuilder::build()
  -> ComputedStyle
```

The builder checks computed-value kind, duplicate properties, and total
registry coverage. `ComputedStyle::initial()` exposes an infallible convenience
boundary because this path consumes only compiled engine metadata. A failure is
therefore an internal registry/builder invariant violation and preserves the
underlying `ComputedStyleBuildError` in its diagnostic. It is not eligible for
document/CSS recovery, and there is no handwritten fallback style table.

Invalid or unsupported authored CSS remains on the existing fallible parsing,
candidate classification, cascade, specified-value, and computed-normalization
paths. AF8 does not convert document input failures into panics.

## Inheritance

AF7 records symbolic inherited/defaulted sources after winner selection.
Computed materialization walks elements top-down. For an inherited source it
copies the corresponding property from the immediate parent's already
materialized `ComputedStyle`; for a root or initial source it uses the AD4
initial token. It does not infer inheritance from an ancestor search or from
raw declarations.

## Computed and used values

CSS owns supported layout-independent specified-to-computed normalization,
including typed keywords, canonical colors, CSS-pixel lengths, integer
`z-index`, `auto`/`none`, and percentages for the supported sizing properties.

Percentages that require a containing-size basis remain typed computed
`LengthPercentage::Percentage` inputs. Layout owns their used-value resolution,
indefinite-basis deferral, formatting-context decisions, intrinsic sizing, and
geometry. Paint consumes Layout geometry and computed paint-relevant values; it
does not resolve CSS sizing.

## Layout and Paint authority

Layout consumes `StylePhaseOutput` and reads typed `ComputedStyle` accessors for
box generation, sizing, metrics, positioning, overflow, text metrics, and other
supported behavior. It does not parse CSS, inspect cascade provenance, or own a
duplicate property table.

If computed `display:none` prevents box generation, no corresponding box enters
Paint. Once Layout has generated a box, Paint must not independently suppress
it because of the source namespace, element name, attributes, raw declaration,
or bridge-era UA rule. Paint consumes Layout geometry plus computed background,
border, outline, color, font, decoration, display, and stacking values.

Browser supplies ordinary HTML defaults through its explicit UA-origin
stylesheet. Rules such as `head, title, meta, link, style, script {
display: none; }` participate in the normal cascade and may be overridden by an
author declaration. CSS initial `display` remains `inline`.

## Reuse boundaries

Two distinct reuse mechanisms remain:

- CSS pass-local computed-style memoization is recreated for one computation
  pass and keys the current supported pure conversion by resolved style plus
  optional parent computed style. It is an optimization, not retained state.
- Browser/runtime retains `ResolvedDocumentStyle` and
  `ComputedDocumentStyle` under `RetainedStyleArtifactKey`, dirty state, and a
  CSS-owned matching-environment compatibility check. `StyledNode` and
  `StylePhaseOutput` are rebuilt borrow-backed views and are not retained style
  artifacts.

AF8 does not introduce computed-style interning, cross-document sharing, or new
cache-key semantics.

## Deterministic regression surfaces

`ComputedStyle::to_debug_snapshot()`,
`ComputedDocumentStyle::to_debug_snapshot()`, `StylePhaseOutput` snapshots,
Layout snapshots, and semantic Paint snapshots remain internal deterministic
regression contracts. They are not CSSOM or JavaScript-facing APIs.

## Supported subset and partial values

The supported property families remain those registered by AD4: foreground and
background color, physical margin/padding/border and outline fields, font size,
display, overflow, position, `z-index`, text-decoration line, and the current
width/height/min/max sizing subset. AD5 remains the property-by-property source
for the exact specified kind, computed kind, normalization, and future used
work.

The following remain unsupported rather than approximated by AF8:

- `em`, `rem`, viewport units, and physical units;
- `calc()`, custom properties, `var()`, and environment substitution;
- font-relative computation, font shaping, and a complete line-height model;
- broad CSS property, selector, media-query, animation, and transition support;
- CSSOM, `getComputedStyle()`, and JavaScript-facing resolved values;
- complete used-value and actual-value coverage.

## Legacy compatibility

`attach_styles(...)`, legacy `compute_style(...)`, and legacy
`build_style_tree(...)` remain isolated compatibility APIs for older callers
and tests. Their DOM-attached declaration vectors and handwritten link, button,
or default-display behavior are not production semantics. New Browser, Layout,
and Paint work must use the structured document path.

AF8 does not remove those APIs or migrate all legacy fixtures. Compatibility
code directly involved in base-style construction uses the same registry and
builder invariant as production and cannot fall back to a second handwritten
initial style.

## Deliberate exclusions

AF8 does not add property families or value units, redesign Layout or Paint,
change retained-style keys, implement style sharing, remove broad legacy APIs,
or expose computed values through CSSOM or JavaScript. Those require separately
scoped work.
