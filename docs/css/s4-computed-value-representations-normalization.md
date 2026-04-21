# S4: Implement Computed Value Representations And Normalization

Last updated: 2026-04-21  
Status: implemented

This document is the contract for Milestone S issue 4: transforming typed
specified values into canonical computed values for Borrowser's supported CSS
subset.

Related code:
- `crates/css/src/computed.rs`
- `crates/css/src/specified.rs`
- `crates/css/src/properties.rs`

Related documents:
- `docs/css/s1-property-system-architecture-computed-style-contract.md`
- `docs/css/s3-parsed-specified-value-representations.md`

## Implemented Result

S4 adds a deterministic normalization path:

```text
SpecifiedPropertyValue
        ↓
ComputedValue
```

The primary API is:

- `ComputedValue::from_specified(...)`
- `normalize_specified_value(...)`
- `ComputedValueNormalizationError`

This path is independent from the legacy `compute_style(...)` string bridge.
The bridge remains temporary compatibility code; it is not the S4 computed-value
contract.

Normalization consumes validated data carried by `SpecifiedPropertyValue`. It
does not reparse authored strings or model-layer syntax.

## Computed Value Surface

The current runtime computed-value variants are:

- `ComputedValue::Color((u8, u8, u8, u8))`
- `ComputedValue::Display(Display)`
- `ComputedValue::Length(Length)`
- `ComputedValue::LengthOrAuto(Option<Length>)`
- `ComputedValue::LengthOrNone(Option<Length>)`

These variants match `PropertyComputedValueKind` and remain the values accepted
by `ComputedStyleBuilder`.

## Normalization Rules

S4 normalization is property-aware but layout-independent.

Current rules:

- color keywords and hex colors normalize to RGBA tuples
- `transparent` normalizes to `(0, 0, 0, 0)`
- 3-digit hex colors are expanded deterministically
- display keywords normalize to the runtime `Display` enum
- `px` lengths normalize to `Length::Px(f32)`
- unitless zero normalizes to `Length::Px(0.0)`
- negative zero normalizes to positive zero
- `auto` in length-or-auto properties normalizes to `None`
- `none` in length-or-none properties normalizes to `None`

S4 does not perform inheritance, initial/default fallback, layout-dependent
resolution, or UA/HTML bridge defaults. Those remain separate pipeline stages.

## Metadata Validation

After normalization, `ComputedValue::from_specified(...)` checks the actual
computed-value discriminant against `property.metadata().computed_value`.

That keeps computed-value normalization aligned with the property registry and
prevents a specified value from silently producing a computed value with the
wrong runtime shape.

## Error Surface

`ComputedValueNormalizationError` exists for normalization failures that can
occur after specified parsing, such as:

- length values outside the current runtime `f32` range
- property metadata / computed-value kind mismatches

Specified-value validation remains responsible for rejecting unsupported
authored grammar, invalid hex data, and invalid numeric data before
normalization begins.

## Out Of Scope

S4 does not implement:

- inheritance/default resolution in the computed layer
- relative units, percentages, font-relative resolution, or layout-dependent
  values
- shorthands, CSS-wide keywords, custom property substitution, or function
  values
- retirement of the legacy `compute_style(...)` bridge

S5 adds the `ResolvedStyle` to `ComputedStyle` assembly path and keeps using
this S4 normalization boundary for winning specified values.

## Test Surface

S4 adds and relies on tests for:

- color keyword and hex normalization
- display keyword normalization
- px length normalization
- unitless zero normalization
- `auto` and `none` branch preservation
- metadata alignment between `PropertyComputedValueKind` and emitted
  `ComputedValue`
- length out-of-range normalization errors
- metadata/value-kind mismatch errors

These tests define the current computed-value normalization contract.
