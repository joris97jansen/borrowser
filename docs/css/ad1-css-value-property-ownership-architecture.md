# AD1: CSS Value And Property Ownership Architecture

Last updated: 2026-06-25
Status: architecture contract for Milestone AD issue 1

This document defines the ownership boundary for Borrowser's CSS value model,
property system, shorthand expansion foundation, and future invalidation impact
classification. It is an architecture contract only. AD1 does not add new
supported properties, implement shorthand parsing, implement CSS-wide keywords,
change runtime invalidation behavior, or move existing impact logic into
registry metadata.

Related code:

- `crates/css/src/model`
- `crates/css/src/properties`
- `crates/css/src/specified`
- `crates/css/src/cascade`
- `crates/css/src/computed`
- `crates/css/src/computed/impact.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/page/retained_render_state.rs`
- `crates/layout/src`
- `crates/gfx/src/paint`

Related documents:

- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`
- `docs/rendering/ac3-explicit-dirty-state-tracking.md`
- `docs/rendering/ac10-retained-rendering-runtime-closeout.md`
- `docs/engine-feature-gap-tracker.md`

## Purpose

Milestone AD strengthens the CSS subsystem so future property work has one
professional place to add parsing, metadata, defaults, inheritance behavior,
shorthand expansion, computed value handling, and invalidation impact facts.

AD1 defines that architecture boundary before later AD issues add concrete
implementation. The goal is to prevent future CSS work from spreading property
semantics across browser/runtime, layout, paint, or ad hoc helper code.

## Ownership Summary

CSS is the canonical owner of:

- property names and supported-property identity;
- supported property metadata;
- property value parsing;
- CSS-wide keyword handling;
- initial values;
- inherited-by-default behavior;
- shorthand expansion into longhands;
- specified value representation;
- computed value representation;
- future property invalidation impact classification.

Browser/runtime may consume CSS-owned invalidation impact facts to update dirty
state, retained artifact generations, render work plans, and conservative
fallbacks. Browser/runtime must not own CSS property semantics, parse property
values, duplicate property metadata, or maintain a parallel CSS property-impact
table.

Layout consumes computed layout-relevant values. Layout owns used geometry,
formatting behavior, containing-size resolution, text and replaced-element
layout, and layout artifacts. Layout must not parse CSS, decide property
inheritance/defaults, expand shorthands, or recover from invalid supported
declarations.

Paint and GFX consume computed paint-relevant values and layout output. Paint
owns paint primitives, paint ordering, stacking, semantic paint artifacts, and
backend-independent paint debug output. Paint and GFX must not parse CSS,
derive missing supported property values, expand shorthands, or classify CSS
property invalidation impact.

## CSS Subsystem Boundary

The CSS subsystem is layered, but the semantic ownership stays inside CSS:

1. `css::model`
   - owns parsed stylesheet, rule, declaration, property-name, and value
     component structure;
   - preserves authored value components for later CSS stages;
   - does not own cascade winners, specified values, or computed values.
2. `css::properties`
   - owns supported property identity and metadata;
   - is the home for supported longhand metadata and future metadata needed by
     shorthand and invalidation work;
   - must remain the shared source for facts consumed by cascade and computed
     style.
3. `css::specified`
   - owns property-aware specified-value parsing and validation;
   - consumes model-layer declaration values and property metadata;
   - rejects invalid supported declarations before layout or paint can observe
     them.
4. `css::cascade`
   - owns selector-driven winner resolution, inheritance source selection, and
     initial/default source selection;
   - produces `ResolvedStyle` and `ResolvedDocumentStyle`;
   - does not own computed normalization or runtime scheduling.
5. `css::computed`
   - owns specified-to-computed normalization, `ComputedValue`,
     `ComputedStyle`, `ComputedDocumentStyle`, and `StyledNode` construction;
   - exposes the typed runtime handoff consumed by layout, paint, and browser
     orchestration;
   - owns computed-style invalidation-impact projection in
     `crates/css/src/computed/impact.rs`.

## Value Lifecycle Terminology

AD uses these lifecycle names when describing property work:

| stage | Borrowser surface | owner | meaning |
| --- | --- | --- | --- |
| parsed component values | `css::model::ValueComponent`, `DeclarationValue` component lists | CSS model | Syntax-derived value components from parsed CSS. These preserve authored structure and are not property-valid runtime values. |
| declared values | `css::model::Declaration` and `DeclarationValue` paired with a parsed property name | CSS model / property pipeline | One authored declaration before supported-property filtering, specified-value parsing, cascade winner selection, or shorthand expansion. Unknown and unsupported declarations may remain visible in debug surfaces but do not become supported computed values. |
| specified values | `SpecifiedPropertyValue`, `SpecifiedValue`, `CascadeSpecifiedValue` | CSS specified/cascade | Property-aware values after validation against the supported property grammar and metadata. They may preserve pre-computed distinctions such as `auto`, `none`, unitless zero, or unresolved percentages. |
| computed values | `ComputedValue`, `ComputedStyle`, `ComputedDocumentStyle` | CSS computed | Normalized runtime-facing values for the supported subset. These are the values layout and paint consume. Percentages that require a layout basis remain represented as computed inputs, not used geometry. |
| used values | future layout-owned used geometry and resolved layout inputs | Layout | Values after layout has applied containing blocks, formatting context rules, intrinsic sizing, line construction, and other layout-dependent resolution. CSS does not produce used geometry in AD1. |
| actual values | future post-layout/backend/device-constrained values | Layout / Paint / backend-specific consumers | Values after final constraints such as device/backend limits, rasterization policy, or future browser compatibility details. AD1 does not introduce actual-value computation. |

The current supported pipeline already separates parsed, specified, and
computed values. AD1 documents that boundary for future AD work; it does not
add new runtime value stages.

## Property Metadata Contract

For the current supported subset, property metadata already records property
identity, canonical name, inheritance behavior, initial/default value,
specified-value kind, computed-value kind, invalid-value policy, and
length-range policy.

Future property work must extend the CSS property system rather than adding
consumer-local property facts. Examples:

- value grammar support belongs in CSS specified-value parsing;
- initial and inherited-by-default behavior belong in CSS property/cascade
  metadata and resolution;
- computed representation belongs in CSS computed values and `ComputedStyle`;
- layout-specific interpretation starts only after layout receives computed
  values;
- paint-specific interpretation starts only after paint receives computed
  values and layout output.

## CSS-Wide Keywords

CSS-wide keywords such as `inherit`, `initial`, `unset`, `revert`, and
`revert-layer` are CSS-owned value semantics. They affect the path from
declared values through specified/cascade/computed values and must not be
implemented by layout, paint, or browser/runtime as missing-value recovery.

AD1 does not implement CSS-wide keywords. Later AD work must define how these
keywords are represented before and after cascade, how they interact with
inheritance/defaulting, and which subset is supported.

AD3 implements the current shared CSS-wide keyword contract for supported
properties. See `docs/css/ad3-css-wide-keyword-handling.md`.

## Shorthand Expansion

Shorthand expansion is CSS-owned. A supported shorthand declaration must expand
into explicit longhand declarations before downstream consumers observe final
specified or computed values.

The shorthand expansion contract is:

- longhand identity and metadata remain owned by CSS;
- shorthand parsing and expansion order remain deterministic;
- unsupported shorthand grammar fails deterministically;
- expansion must not cause layout or paint to parse multi-component CSS text;
- shorthand support must be representative and documented when implemented.

AD1 does not implement new shorthand parsing or broad multi-component
declaration expansion. Existing narrow CSS behavior remains unchanged until
later AD issues explicitly extend it.

## Invalidation Impact Ownership

Property invalidation impact is CSS-owned because it depends on CSS property
meaning. Browser/runtime may consume CSS-owned impact facts, but it must not
derive them from declaration names, `PropertyId` match tables, authored text,
or duplicated property metadata.

Current behavior:

- `crates/css/src/computed/impact.rs` contains the current CSS-owned
  computed-style invalidation-impact comparison.
- Browser/runtime consumes the resulting computed-document impact through the
  retained rendering path.
- AD7 defines the richer CSS-owned invalidation impact classification model,
  including the concrete taxonomy, registry-backed impact metadata, the
  narrow Browser/runtime consumption API, and the focused CSS and
  Browser/runtime tests for that behavior.

Contributors must not move CSS property impact classification into
browser/runtime. If a style change cannot be safely classified through
CSS-owned behavior, runtime must use an explicit conservative fallback.

## Browser/runtime Consumption Boundary

Browser/runtime owns:

- retained state lifetime;
- DOM/style input and stylesheet generations;
- dirty-state aggregation and propagation;
- retained artifact keys, lifecycle counters, and debug output;
- render work planning and conservative fallbacks.

Browser/runtime may consume:

- `ResolvedDocumentStyle` and `ComputedDocumentStyle` as CSS-owned retained
  style artifacts;
- `StyledNode` as a CSS-owned borrow-backed view;
- CSS-owned style impact results exposed through CSS APIs.

Browser/runtime must not:

- parse authored CSS or declaration values;
- compute selector specificity or cascade winners;
- invent inheritance/default behavior;
- expand shorthands;
- synthesize supported-property initial values;
- classify CSS property impact in a local property table.

## Layout Consumption Boundary

Layout owns box generation, formatting contexts, sizing, line layout, retained
layout artifact materialization, and used geometry.

Layout may consume:

- `StyledNode`;
- `ComputedStyle`;
- typed computed accessors and `ComputedStyle::get(PropertyId)` when needed;
- explicit layout environment inputs such as viewport and resource metadata.

Layout must not:

- inspect cascade winners or resolved-style provenance;
- parse CSS strings or component values;
- define property initial/default behavior;
- recover from invalid supported property values;
- resolve unsupported CSS grammar locally.

Computed percentages and keywords that need layout context are inputs to
layout. The resulting used sizes and positions are layout output, not computed
CSS values.

## Paint/GFX Consumption Boundary

Paint owns semantic paint artifacts, paint primitives, stacking contexts,
paint ordering, and backend-independent paint debug output. GFX owns viewport,
input, and backend-facing rendering integration.

Paint and GFX may consume:

- layout output;
- computed paint-relevant values exposed through `ComputedStyle`;
- resource and input state owned by browser/runtime;
- retained paint lifetime decisions from browser/runtime.

Paint and GFX must not:

- parse authored CSS;
- inspect cascade winners;
- infer missing supported property values;
- classify CSS property invalidation impact;
- mutate computed style or layout geometry to compensate for unsupported CSS.

## AD1 Scope

AD1 implements:

- this architecture contract;
- explicit CSS/browser/runtime/layout/paint ownership boundaries;
- terminology for parsed component values, declared values, specified values,
  computed values, and future used/actual values;
- documentation that current impact comparison remains CSS-owned behavior;
- a feature gap tracker update that records AD1 as architecture-only progress.

AD1 does not implement:

- new supported properties;
- new typed Rust property impact categories beyond the architecture boundary;
- registry-backed invalidation impact metadata; AD7 implements this separately;
- a Browser/runtime impact API; AD7 implements this separately;
- Browser/runtime integration tests for paint-only style changes; AD7 adds
  these separately;
- shorthand completeness;
- CSS-wide keyword parsing;
- full selectors, full cascade, media queries, custom properties, animations,
  transitions, or complete property coverage;
- layout used-value or actual-value computation.

## AD7 Handoff

AD7 is the implementation issue for concrete CSS-owned invalidation impact
classification. AD7 defines:

- the typed impact taxonomy for supported properties;
- whether impact is represented directly as registry metadata, derived from
  registry metadata, or split between metadata and a CSS-owned classifier;
- the narrow API Browser/runtime consumes;
- deterministic CSS tests for classification;
- Browser/runtime tests proving runtime consumes CSS-owned impact without
  owning property semantics.

AD1 intentionally stops before those choices become Rust APIs. See
`docs/css/ad7-css-owned-invalidation-impact-classification.md` for the
implemented contract.

## Invariants

- CSS remains the only owner of CSS property semantics.
- Property work extends CSS property/value/cascade/computed contracts before
  downstream consumers depend on it.
- Browser/runtime consumes CSS-owned style artifacts and impact facts; it does
  not duplicate CSS property semantics.
- Layout consumes computed values and owns used geometry.
- Paint/GFX consume computed visual values and layout output; they own paint
  semantics, not CSS semantics.
- Unsupported and unimplemented CSS features remain explicit gaps rather than
  hidden fallback behavior.
