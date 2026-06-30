# AD10: CSS Value And Property Foundation Closeout

Last updated: 2026-06-30
Status: closeout contract for Milestone AD issue 10

This document closes Milestone AD as a CSS value/property foundation
milestone. It does not close broad CSS property coverage, selector
conformance, full cascade conformance, media queries, custom properties,
animations, WPT integration, layout behavior, paint behavior, compositor
behavior, or runtime behavior.

Milestone AD makes the supported CSS property pipeline explicit and
inspectable so future CSS work can add properties without inventing ad hoc
paths outside CSS.

Related code:

- `crates/css/src/values.rs`
- `crates/css/src/model`
- `crates/css/src/properties`
- `crates/css/src/specified`
- `crates/css/src/cascade`
- `crates/css/src/computed`
- `crates/css/src/computed/impact.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/page/retained_render_state.rs`

Related documents:

- `docs/css/ad1-css-value-property-ownership-architecture.md`
- `docs/css/ad2-typed-core-css-value-model.md`
- `docs/css/ad3-css-wide-keyword-handling.md`
- `docs/css/ad4-css-property-registry-longhand-metadata.md`
- `docs/css/ad5-specified-computed-value-boundaries.md`
- `docs/css/ad6-shorthand-expansion-foundation.md`
- `docs/css/ad7-css-owned-invalidation-impact-classification.md`
- `docs/css/ad8-deterministic-declaration-parsing-diagnostics.md`
- `docs/css/ad9-css-property-coverage-metadata-debug-snapshots.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`
- `docs/rendering/ac3-explicit-dirty-state-tracking.md`
- `docs/rendering/ac10-retained-rendering-runtime-closeout.md`
- `docs/engine-feature-gap-tracker.md`

## What AD Completed

AD completed the foundation for CSS-owned property support:

- a typed value model for the current supported subset;
- a supported longhand registry with explicit metadata;
- centralized CSS-wide keyword handling for supported declarations;
- documented specified and computed value boundaries;
- a CSS-owned shorthand expansion path with one narrow representative subset;
- CSS-owned invalidation impact classification for supported longhands;
- deterministic declaration parsing, unsupported-property handling, and debug
  snapshots for the property pipeline.

This is foundation completion only. The current supported property set remains
small and does not represent broad CSS property or web-platform coverage.

## CSS Value Model

AD2 formalizes reusable CSS-owned value primitives in `crates/css/src/values.rs`
and the specified-value parser. The current model covers the categories needed
by the supported property subset, including keywords, finite numeric values,
integers, `px` lengths, unitless zero where accepted, percentages for supported
sizing values, length-percentage branches, a narrow color subset, and authored
URL/string/function wrappers used for deterministic future extension and
rejection.

The value layers remain separate:

| layer | owner | current role |
| --- | --- | --- |
| syntax/model values | CSS syntax/model | preserve authored tokens and declaration components |
| core CSS values | CSS values/specified | reusable validated primitives for current parser support |
| specified values | CSS specified/cascade | property-aware accepted declaration values |
| computed values | CSS computed | normalized runtime-facing values |
| used values | Layout | future and current layout-owned geometry resolution |
| actual values | Layout/Paint/backend | future backend- or device-constrained results |

Parser/model values are not runtime values, and runtime-facing computed values
must not carry parser tokens or raw authored CSS strings as their behavior
source.

## Property Registry And Metadata

AD4 formalizes `css::properties` as the single supported longhand registry.
`PropertyId`, `PropertyRegistration`, `PropertyMetadata`, and
`property_registry()` define the current supported longhand universe and its
canonical order.

Each supported longhand has explicit CSS-owned metadata for:

- canonical property name;
- inherited-by-default behavior;
- initial value;
- specified-value kind;
- computed-value kind;
- invalid-value policy;
- length sign policy where relevant;
- invalidation impact.

The registry is not a complete known-CSS-property database. Unsupported
families must not be added as placeholder rows. A property should enter the
registry only when its parser contract, specified/computed representation,
metadata, tests, debug output, docs, and feature-gap wording are implemented
together.

## CSS-Wide Keywords

AD3 centralizes CSS-wide keyword parsing and resolution for supported
properties. `initial`, `inherit`, and `unset` are supported through the CSS
cascade/resolved-style path and materialized by computed style from resolved
CSS-owned sources.

`revert` and `revert-layer` are recognized but unsupported because correct
behavior depends on cascade origin and cascade layer semantics that Borrowser
does not yet implement.

Property-specific parsers must not add local branches for CSS-wide keywords.
Browser/runtime, Layout, Paint, and GFX must not interpret CSS-wide keyword
semantics.

## Specified And Computed Boundaries

AD5 documents the boundary between authored declaration components, specified
values, computed values, layout used values, and future actual values.

Supported declarations flow through:

```text
model::Declaration
  -> property registry lookup
  -> specified-value parsing
  -> cascade winner/source resolution
  -> computed-value normalization
  -> ComputedStyle / ComputedDocumentStyle
```

Layout consumes typed computed values and owns used geometry. Paint consumes
computed visual values plus layout output and owns paint primitives and visual
output. Browser/runtime consumes CSS-owned style artifacts and impact facts
while owning scheduling, retained state, dirty state, and render work planning.

The AD5 value-boundary inventory and computed debug snapshots are internal
regression surfaces derived from the real registry and typed values. They are
not implementation inputs, CSSOM, or public APIs.

## Shorthand Expansion

AD6 adds a CSS-owned shorthand registry and expansion path separate from the
longhand registry. The current supported shorthand subset is intentionally
narrow:

```text
outline -> outline-color, outline-style, outline-width
```

Expanded longhands flow through the same longhand registry, specified parser,
CSS-wide keyword handling, cascade behavior, computed normalization, and
invalidation metadata as directly authored longhands.

Shorthand expansion is atomic. Invalid shorthand declarations emit no longhand
candidates and remain visible only through deterministic debug classification.

Broad shorthand expansion remains missing, including `border`,
`border-width`, `border-style`, `border-color`, `background`, `font`,
`margin`, `padding`, `text-decoration`, flex shorthands, and other
multi-component CSS families.

## Invalidation Impact Classification

AD7 moves property invalidation facts into CSS-owned metadata for supported
longhands. `PropertyInvalidationImpact` records composable CSS impact flags,
and `crates/css/src/computed/impact.rs` projects changed computed style into
the narrow runtime-facing result:

- `NoVisualImpact`
- `StyleOnly`
- `PaintOnly`
- `LayoutAffecting`
- `Unknown`

Browser/runtime may consume only the CSS-owned computed-document impact result.
It must not inspect property names, authored CSS, registry flags, or computed
values to define CSS property impact.

Conservative classifications, such as current `z-index` layout-affecting
projection, are intentional until narrower retained paint-order invalidation
contracts exist.

## Declaration Parsing And Unsupported Behavior

AD8 adds deterministic declaration-list parsing and classification. The
current CSS-owned declaration pipeline classifies declarations before they can
become cascade candidates:

- supported longhands use `property_registry()`;
- supported shorthands use `shorthand_registry()` and atomic expansion;
- unknown standard names become unsupported-property declarations;
- custom-shaped names remain explicit custom-property declarations without
  implementing custom property semantics;
- invalid property names remain invalid-property-name declarations;
- invalid supported longhand values are rejected declarations;
- invalid supported shorthand values emit no longhands.

Unsupported, custom, invalid, and malformed declarations are deterministic
non-candidates. Layout, Paint, GFX, and Browser/runtime must not recover
unsupported or invalid CSS after CSS rejects it.

## Debug And Regression Surfaces

AD8 makes declaration-list parsing and classification inspectable through
deterministic declaration pipeline diagnostics.

AD9 makes the current property system inspectable through deterministic CSS
debug snapshots and golden fixtures for:

- property coverage;
- registry metadata;
- value boundary inventory;
- representative property value parsing;
- shorthand registry and expansion;
- invalidation classification;

These surfaces are internal regression contracts. They are not CSSOM, public
web APIs, rendering inputs, or a substitute for typed implementation paths.

## Future Extension Points

Future milestones should extend the AD foundation deliberately:

- selector and cascade conformance: expand selector coverage, specificity,
  origins, layers, `!important` behavior, dependency tracking, and invalidation
  with CSS-owned contracts and deterministic tests;
- custom properties: add parsed custom property storage, inheritance,
  substitution, invalid-at-computed-value-time behavior, and `var(...)`
  handling without treating custom-shaped declarations as ordinary longhands;
- media queries: add media/container condition parsing, stylesheet rule
  filtering, viewport/input dependencies, and invalidation contracts;
- CSS color expansion: add named colors, `currentcolor`, alpha syntax,
  functions such as `rgb()`/`hsl()`, color spaces, and computed color
  normalization with paint-facing tests;
- typography: add font families, weights, styles, line height, relative units,
  font metrics, shaping dependencies, and text layout contracts;
- layout-affecting property expansion: add new layout inputs through the
  property registry, specified/computed values, layout-owned used-value
  resolution, and retained invalidation metadata;
- paint-affecting property expansion: add backgrounds, border radius,
  shadows, opacity, transforms, filters, clipping, and paint/compositor impact
  only through CSS-owned metadata and Paint-owned output contracts;
- WPT integration: add focused WPT lanes for CSS parser, selector, cascade,
  computed style, layout, and paint behavior after the relevant engine
  contracts exist.

## Known Missing Families

AD does not complete:

- broad shorthand expansion;
- fonts and full typography;
- CSS Text and full Text Decoration;
- backgrounds;
- border radius;
- transforms;
- filters;
- custom properties and `var(...)`;
- media queries and container queries;
- animations and transitions;
- broad selector coverage and selector invalidation;
- full cascade origin/layer conformance;
- broad CSS Values and Units;
- full CSS Color;
- CSSOM;
- WPT-backed broad CSS conformance.

The feature gap tracker must continue to list these families until later
milestones implement them with code, tests, and docs.

## Closeout Invariants

- CSS owns property meaning, parsing, metadata, CSS-wide keywords, shorthand
  expansion, specified values, computed values, and invalidation
  classification.
- Browser/runtime consumes CSS-owned style artifacts and impact projections
  without owning CSS property semantics.
- Layout consumes computed values and owns used geometry.
- Paint consumes computed values plus layout output and owns visual output.
- Supported-property coverage is visible through deterministic debug
  snapshots.
- Unsupported properties and unsupported values remain deterministic and safe.
- Milestone AD completion means foundation completion only.

## Deliberate Exclusions

AD10 deliberately excludes:

- new CSS property support;
- new shorthand support;
- selector behavior changes;
- cascade behavior changes;
- custom properties;
- media queries;
- animations and transitions;
- WPT integration;
- runtime, layout, paint, compositor, or GFX behavior changes;
- new Rust APIs or data structures;
- snapshot updates for behavior that did not change.
