# S2: Introduce Property Identifiers And Property Registry

Last updated: 2026-04-20  
Status: implemented

This document is the source-of-truth contract for Milestone S issue 2: the
engine-owned property identifier registry for Borrowser's current supported CSS
subset.

Related code:
- `crates/css/src/properties.rs`
- `crates/css/src/cascade/integration.rs`
- `crates/css/src/cascade/contract/resolved_style.rs`
- `crates/css/src/computed.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/s1-property-system-architecture-computed-style-contract.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`

## Implemented Result

Borrowser now has:

- a typed `PropertyId` identifier enum for the supported subset
- a first-class `PropertyRegistry`
- explicit `PropertyRegistration` records binding id, canonical CSS name, and
  property metadata
- deterministic lookup from canonical parsed property names into property ids
- registry-backed canonical property iteration used by cascade and computed
  style

This replaces the previous "enum plus separate match tables" shape with one
engine-owned registry-backed property system.

## Registry Structure

The shared registry lives in `css::properties` and is exposed through:

- `property_registry()`
- `PropertyRegistry`
- `PropertyRegistration`
- `PropertyId`

Each registration owns:

- `PropertyId`
- canonical CSS property name
- `PropertyMetadata`

`PropertyId::name()`, `PropertyId::from_name(...)`, and
`PropertyId::metadata()` now delegate into the registry rather than owning
separate lookup logic.

`PropertyMetadata` is also the owner for value-range facts needed by
property-aware parsers, such as the length sign policy for properties with
length branches.

## Lookup Contract

Name lookup is:

- deterministic
- exact over canonical lowercase parsed property names
- backed by an explicit registry lookup index

The model layer remains responsible for canonicalizing standard property names
to lowercase. The property registry does not perform case folding itself.

Unknown names return `None` and therefore remain explicit unsupported-property
inputs in cascade.

## Cascade And Computed Integration

The registry now participates directly in the hot path:

- cascade declaration materialization resolves supported standard property
  names through `property_registry().lookup_id(...)`
- resolved-style default fill iterates the registry's canonical property ids
- computed-style initial assembly and completeness checks iterate the registry's
  canonical property ids

This keeps the supported property subset synchronized across cascade and
computed-style assembly without re-encoding property tables in those layers.

## Invariants

S2 establishes these additional invariants:

- the registry is total over `PropertyId::ALL`
- registry entries are stored in canonical property order
- every supported property id resolves to exactly one registration
- canonical property-name lookup is deterministic
- cascade and computed-style iteration use the registry's canonical property
  universe rather than local string tables

If future work adds supported properties, the registry, tests, and downstream
consumers must change together.

## Test Surface

S2 adds and relies on these tests:

- `properties::tests::property_registry_entries_are_total_canonical_and_metadata_backed`
- `properties::tests::property_registry_lookup_is_deterministic_for_representative_property_names`
- `properties::tests::property_registry_get_returns_registration_for_every_supported_id`

These tests are part of the product contract for supported-property mapping and
registry behavior.

## S2 Out Of Scope

S2 itself did not implement:

- typed specified-value parsers
- `ResolvedStyle` to typed specified-value conversion
- full replacement of the legacy string-driven computed-style bridge
- broader property coverage beyond the current supported subset

Typed specified-value parsing is introduced by S3.
