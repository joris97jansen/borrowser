# AD4: CSS Property Registry And Longhand Metadata

Last updated: 2026-06-27
Status: implemented contract for Milestone AD issue 4

This document defines Borrowser's CSS-owned supported-longhand property
registry for Milestone AD. AD4 refines the existing S-era property registry
instead of introducing a second property table.

AD4 does not add broad CSS property coverage, shorthand expansion, selector
invalidation, media queries, custom properties, animations, compositor
behavior, or the full AD7 invalidation taxonomy.

Related code:

- `crates/css/src/properties`
- `crates/css/src/specified/parse.rs`
- `crates/css/src/computed/value.rs`
- `crates/css/src/computed/style.rs`
- `crates/css/src/computed/impact.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/page/retained_render_state.rs`

Related documents:

- `docs/css/ad1-css-value-property-ownership-architecture.md`
- `docs/css/ad2-typed-core-css-value-model.md`
- `docs/css/ad3-css-wide-keyword-handling.md`
- `docs/css/ad7-css-owned-invalidation-impact-classification.md`
- `docs/css/s1-property-system-architecture-computed-style-contract.md`
- `docs/css/s2-property-identifiers-property-registry.md`
- `docs/css/s3-parsed-specified-value-representations.md`
- `docs/css/s4-computed-value-representations-normalization.md`
- `docs/css/s5-invalid-value-handling-fallback.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/rendering/ac3-explicit-dirty-state-tracking.md`
- `docs/rendering/ac10-retained-rendering-runtime-closeout.md`
- `docs/engine-feature-gap-tracker.md`

## Purpose

The supported longhand registry is the canonical place where CSS describes
known supported property semantics:

- canonical property name;
- inherited-by-default behavior;
- initial value;
- specified-value parser kind;
- computed-value kind;
- invalid-value policy;
- length sign policy;
- current narrow invalidation impact.

These facts belong to CSS. Browser/runtime, Layout, Paint, and GFX may consume
computed CSS outputs or CSS-owned impact results, but they must not own or
duplicate property meaning.

## Registry Shape

The registry lives in `css::properties` and is exposed through:

- `PropertyId`
- `PropertyMetadata`
- `PropertyRegistration`
- `PropertyRegistry`
- `property_registry()`
- `property_registry_metadata_debug_snapshot()`

`PropertyId::ALL` and `property_registry().entries()` define the canonical
supported longhand order. Debug output, total style assembly, cascade default
fill, and computed-style iteration must use this canonical order rather than
map insertion order.

`PropertyMetadata` is the normative metadata surface for one supported
longhand. It records inheritance, initial value, parser/value-kind contracts,
invalid-value policy, length sign policy, and invalidation impact.

## Parser And Value-Kind Contract

AD4 keeps the existing typed parser-kind model:

```text
PropertySpecifiedValueKind -> specified parser dispatch
PropertyComputedValueKind  -> computed normalization and builder validation
```

The registry does not store parser function pointers. The typed enum dispatch
in `crates/css/src/specified/parse.rs` is the current maintainable parser hook
surface. A future parser architecture may change this only if code, tests, and
docs preserve the single CSS-owned metadata source.

Property-specific parsers must not add CSS-wide keyword handling. CSS-wide
keywords remain declaration-level CSS semantics handled by the shared AD3
path.

## Explicit Invalidation Impact

AD4 introduced the first narrow CSS-owned property impact metadata field for
the supported longhand registry. AD7 later replaced that two-category model
with a composable `PropertyInvalidationImpact` flags struct while preserving
the same ownership rule: every supported longhand registration must pass an
explicit impact value, and `PropertyMetadata` constructors do not provide a
default impact category.

This prevents incomplete property classification from hiding behind a silent
conservative fallback. The current AD7 taxonomy is documented in
`docs/css/ad7-css-owned-invalidation-impact-classification.md`.

## Runtime Consumption Boundary

`crates/css/src/computed/impact.rs` maps registry metadata into the
runtime-facing computed-style invalidation API:

```text
ComputedDocumentStyle::invalidation_impact_against(previous)
```

The runtime-facing projection is intentionally narrow:

- `NoVisualImpact`
- `StyleOnly`
- `PaintOnly`
- `LayoutAffecting`
- `Unknown`

`NoVisualImpact` and `StyleOnly` are derived projection categories, not raw
registry metadata flags. Raw `PropertyInvalidationImpact` metadata records
positive CSS impact facts only.

Browser/runtime consumes this CSS-owned result through retained rendering
state. Browser/runtime does not inspect declaration names, parse CSS values,
inspect registry flags, or maintain a CSS property-impact table.

## Unknown And Unsupported Properties

The AD4 registry is a supported-longhand registry, not a complete known CSS
property universe. A property name outside the registry remains deterministic
and unsupported:

```text
property_registry().lookup_id(name) == None
```

Known-but-unsupported CSS families, shorthands, and future longhands should not
be added merely as placeholders. They should enter the registry only when the
issue also defines their supported parser contract, computed-value contract,
metadata, tests, and documentation.

## Debug And Test Surface

`property_registry_metadata_debug_snapshot()` emits deterministic metadata in
canonical registry order. The golden fixture under
`crates/css/tests/fixtures/properties/registry_metadata.snap` is an internal
regression contract for:

- supported longhand order;
- canonical names;
- inheritance and initial values;
- specified and computed value kinds;
- invalid-value and length-sign policy;
- explicit invalidation impact labels.

This debug output is not CSSOM and not a public web API.

## Adding A Supported Longhand

Adding a supported longhand requires:

1. Add the `PropertyId` entry in canonical order.
2. Add exactly one `PropertyRegistration` with explicit metadata, including
   invalidation impact.
3. Add the canonical lookup entry.
4. Extend specified parser support through `PropertySpecifiedValueKind` and
   `specified` parsing, if needed.
5. Extend computed value support through `PropertyComputedValueKind`,
   normalization, `ComputedValue`, and `ComputedStyleBuilder`, if needed.
6. Add or update tests for lookup, parsing, invalid handling, computed
   materialization, debug output, and impact classification.
7. Update docs and the feature gap tracker when the supported feature set or
   extension point changes.

Do not add property behavior directly to Browser/runtime, Layout, Paint, or
GFX. If a downstream subsystem needs a new CSS fact, model it in CSS metadata
or computed values first.

## Invariants

- There is exactly one CSS-owned supported longhand registry.
- Registry entries are deterministic and canonical.
- Every supported longhand has complete metadata.
- Every supported longhand has an explicit invalidation impact.
- Unknown property names remain deterministic and unsupported.
- Browser/runtime consumes CSS-owned computed-style impact results.
- Layout and Paint consume computed style and layout/paint inputs, not
  authored CSS or property metadata tables.
- Debug output uses registry order and stable labels.

## Deliberate Exclusions

AD4 deliberately excludes:

- broad CSS property coverage;
- shorthand expansion;
- complete known-unsupported property modeling;
- selector invalidation;
- cascade origins/layers beyond the existing model;
- media queries and container queries;
- custom properties and `var(...)`;
- animations and transitions;
- compositor/GPU behavior;
- full AD7 invalidation taxonomy; AD7 now documents and implements that
  taxonomy for the current supported longhand subset;
- public CSSOM metadata APIs.
