# AD9: CSS Property Coverage And Metadata Debug Snapshots

Last updated: 2026-06-30
Status: implemented contract for Milestone AD issue 9

This document defines Borrowser's deterministic debug and golden-fixture
surfaces for inspecting the current CSS property system.

AD9 does not add broad CSS property support, new shorthand families, selector
invalidation, media queries, custom properties, animations, CSSOM, layout
behavior, paint behavior, or Browser/runtime-owned CSS semantics.

Related code:

- `crates/css/src/properties`
- `crates/css/src/specified/shorthand.rs`
- `crates/css/src/computed/impact.rs`
- `crates/css/tests/property_registry_golden.rs`
- `crates/css/tests/fixtures/properties`

Related documents:

- `docs/css/ad4-css-property-registry-longhand-metadata.md`
- `docs/css/ad5-specified-computed-value-boundaries.md`
- `docs/css/ad6-shorthand-expansion-foundation.md`
- `docs/css/ad7-css-owned-invalidation-impact-classification.md`
- `docs/css/ad8-deterministic-declaration-parsing-diagnostics.md`
- `docs/css/s8-deterministic-debug-regression-coverage.md`
- `docs/css/n6-stable-debug-serialization.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/engine-feature-gap-tracker.md`

## Purpose

AD9 makes the supported CSS property foundation inspectable as a deterministic
internal regression contract. Future property work should change a visible
fixture when it changes supported longhand coverage, metadata, value parsing,
shorthand registration, shorthand expansion, or invalidation classification.

"Property coverage" means coverage of Borrowser's currently supported
longhand subset. Unsupported properties remain unsupported and are not added as
placeholder rows.

## Debug Surfaces

AD9 adds or formalizes these CSS-owned debug surfaces:

- `property_coverage_debug_snapshot()`
- `shorthand_registry_debug_snapshot()`
- `shorthand_expansion_debug_snapshot(...)`
- `property_invalidation_classification_debug_snapshot()`

It also adds AD9 golden fixture coverage that uses the existing
`computed_value_debug_snapshot(...)` path for representative, registry-complete
value parsing and normalization output.

These are maintenance and regression surfaces. They are not CSSOM, public web
APIs, rendering inputs, or runtime behavior paths.

## Source Of Truth

Every AD9 snapshot is derived from existing engine-owned data or behavior:

- supported longhands come from `property_registry()`;
- longhand facts come from real `PropertyMetadata`;
- shorthand support comes from `shorthand_registry()`;
- shorthand expansion comes from `expand_shorthand_declaration(...)`;
- emitted shorthand longhands are parsed through the existing specified-value
  parser;
- value parsing snapshots parse authored fixture CSS through the stylesheet
  parser before using `computed_value_debug_snapshot(...)`;
- invalidation snapshots derive from `PropertyInvalidationImpact` metadata and
  the existing computed-style projection used by runtime consumers.

AD9 deliberately avoids duplicate debug-only metadata tables. If a snapshot row
is wrong, the underlying registry, metadata, parsing, expansion, or
classification source should be fixed first.

## Ordering Rules

Snapshot output is deterministic:

- longhand property output follows `property_registry().entries()` order;
- shorthand registry output follows `shorthand_registry().entries()` order;
- shorthand expansion output follows the expansion order emitted by the
  shorthand implementation;
- representative fixture cases remain in authored fixture order only for
  scenario snapshots, while registry-complete property parsing output is
  emitted in registry order;
- output uses stable debug labels rather than Rust-derived `Debug` formatting.

Snapshots must not depend on hash-map, hash-set, or caller insertion order.

## Golden Fixtures

AD9 fixtures live under `crates/css/tests/fixtures/properties/`:

- `property_coverage.snap`
- `ad9_property_values.css`
- `ad9_property_values.snap`
- `shorthand_registry.snap`
- `ad9_shorthand_expansion.css`
- `ad9_shorthand_expansion.snap`
- `invalidation_classification.snap`

`crates/css/tests/property_registry_golden.rs` asserts that the AD9 property
value fixture contains exactly one declaration for every currently supported
longhand. This prevents fixture drift when the registry changes.

## Updating Snapshots

When CSS property support changes:

1. Update the real CSS source of truth first: property registry metadata,
   specified-value parser, computed-value normalization, shorthand registry,
   shorthand expansion, or invalidation metadata.
2. Add or update representative fixture declarations only for implemented
   support.
3. Run the targeted property golden tests.
4. Inspect fixture diffs as engine contract diffs.
5. Update fixtures only when the behavior or metadata change is intentional.
6. Update AD docs and the feature gap tracker when a feature gap narrows or a
   new extension point becomes contractual.

Do not update snapshots to bless ad-hoc behavior, placeholder unsupported
properties, or Browser/runtime-owned CSS semantics.

## Invariants

- CSS owns property metadata, value parsing, shorthand expansion, and
  invalidation classification.
- Browser/runtime consumes CSS-owned computed-style impact projections and
  does not own property-name, metadata, or invalidation tables.
- Supported longhand coverage is visible through deterministic snapshots.
- Shorthand support and representative expansion behavior are visible through
  deterministic snapshots.
- Invalidation flags and computed-style projections are visible through
  deterministic snapshots.
- Unsupported properties remain deterministic non-candidates until deliberately
  implemented.

## Deliberate Exclusions

AD9 excludes:

- broad CSS property coverage;
- additional shorthand families beyond the current supported subset;
- full CSS parser conformance matrices;
- selector dependency invalidation;
- media and container queries;
- custom properties and `var(...)`;
- animations and transitions;
- layout, paint, compositor, or Browser/runtime behavior changes;
- public CSSOM metadata APIs.
