# S3: Define Parsed Specified Value Representations Per Property

Last updated: 2026-04-21  
Status: implemented

This document is the contract for Milestone S issue 3: property-aware parsed
specified values for Borrowser's supported CSS subset.

Related code:
- `crates/css/src/specified.rs`
- `crates/css/src/cascade/contract/declarations.rs`
- `crates/css/src/cascade/integration.rs`
- `crates/css/src/properties.rs`

Related documents:
- `docs/css/s1-property-system-architecture-computed-style-contract.md`
- `docs/css/s2-property-identifiers-property-registry.md`

## Implemented Result

Supported declarations now cross from the model layer into cascade through a
typed specified-value representation:

- `SpecifiedPropertyValue`
- `SpecifiedValue`
- property-family value types such as `SpecifiedColor`,
  `SpecifiedDisplay`, `SpecifiedLength`, `SpecifiedLengthOrAuto`, and
  `SpecifiedLengthOrNone`
- `SpecifiedValueParseError` for rejected authored values

`CascadeSpecifiedValue` is no longer a generic wrapper around
`DeclarationValue` for supported cascade candidates. Supported candidates must
carry a parsed `SpecifiedPropertyValue` whose `property` matches the candidate
property id. Unsupported, custom, invalid-name, and invalid-value declarations
may retain preserved CSS text for debug output, but they cannot become cascade
candidates.

## Parsing Boundary

The S3 parser consumes the model-layer `DeclarationValue` component tree. It
does not reparse serialized CSS strings. Trivia components are ignored for the
single-value forms currently supported by the property registry.

The parser is property-id driven:

1. `PropertyId` selects the expected `PropertySpecifiedValueKind`.
2. `parse_specified_value(...)` validates the model value against that shape.
3. The resulting `SpecifiedPropertyValue` records both the property id and the
   typed specified value.

This means a typed specified value cannot be silently paired with a different
property during cascade candidate construction.

## Supported Specified Value Shapes

The current subset has these typed specified families:

| kind | representation | examples |
| --- | --- | --- |
| `Color` | `SpecifiedColor` | `red`, `transparent`, `#fff`, `#ff00aa` |
| `DisplayKeyword` | `SpecifiedDisplay` | `block`, `inline`, `inline-block`, `list-item`, `none` |
| `AbsoluteLength` | `SpecifiedLength` | `12px`, `0`, `-4px` for margins |
| `AbsoluteLengthOrAuto` | `SpecifiedLengthOrAuto` | `auto`, `10px`, `0` |
| `AbsoluteLengthOrNone` | `SpecifiedLengthOrNone` | `none`, `10px`, `0` |

Unitless zero is represented explicitly as specified syntax. It is not
converted into computed px data in S3.

## Validation Rules

S3 rejects values at the property pipeline boundary when they do not match the
supported specified shape.

Examples:

- `display: grid` is rejected for the current `display` subset.
- `width: none` is rejected because `width` accepts `auto`, not `none`.
- `max-width: auto` is rejected because `max-width` accepts `none`, not `auto`.
- non-zero unitless lengths are rejected.
- non-margin negative lengths are rejected according to
  `PropertyMetadata::length_sign`.
- unsupported function syntax such as `rgb(...)` or `calc(...)` is rejected
  until later milestones add those value families.

Rejected supported-property declarations become
`CascadeDeclarationApplicability::InvalidValue(property)` and do not enter
winner resolution. Layout and paint never receive invalid supported-property
values for recovery.

## Determinism

Specified values are deterministic:

- supported values have one typed representation per property
- value-range facts such as length sign policy come from the property registry,
  not parser-local property tables
- keyword matching is ASCII case-insensitive and serialized canonically
- hex colors are serialized with lowercase digits
- canonical property order remains owned by the property registry
- invalid value reasons are explicit and testable

The specified-value serializer used by cascade debug output is still a debug
surface, not computed CSS serialization.

## Out Of Scope

S3 does not implement:

- computed-value conversion from `ResolvedStyle`
- inheritance/default application beyond the existing cascade contract
- layout-dependent resolution
- relative units, percentages, shorthands, CSS-wide keywords, custom property
  substitution, or function values
- moving the legacy `compute_style(...)` string bridge to the typed pipeline

The next S issues should consume `ResolvedStyle` winners through
`SpecifiedPropertyValue` and assemble `ComputedStyle` through
`ComputedStyleBuilder`.

## Test Surface

S3 adds and relies on these tests:

- `specified::tests::parses_representative_property_aware_specified_values`
- `specified::tests::parses_unitless_zero_as_specified_length_without_computing_it`
- `specified::tests::rejects_values_that_do_not_match_the_property_specified_shape`
- `specified::tests::supported_property_metadata_matches_emitted_specified_value_kinds`
- cascade rule-input tests for `InvalidValue(...)`
- document cascade tests proving invalid supported values are rejected before
  winner resolution

These tests are part of the property/value contract for the current supported
subset.
