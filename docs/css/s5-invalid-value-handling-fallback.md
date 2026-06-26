# S5: Invalid Value Handling And Fallback Behavior

Last updated: 2026-04-21  
Status: implemented

This document is the contract for Milestone S issue 5: invalid-value handling
and fallback behavior for Borrowser's supported CSS property subset.

Related code:
- `crates/css/src/properties.rs`
- `crates/css/src/specified.rs`
- `crates/css/src/cascade/contract/declarations.rs`
- `crates/css/src/cascade/integration.rs`
- `crates/css/src/cascade/contract/resolved_style.rs`
- `crates/css/src/computed.rs`

Related documents:
- `docs/css/s1-property-system-architecture-computed-style-contract.md`
- `docs/css/s3-parsed-specified-value-representations.md`
- `docs/css/s4-computed-value-representations-normalization.md`

## Implemented Result

Invalid supported-property values are rejected at the property pipeline
boundary and do not become cascade candidates.

The current invalid-value policy for every supported property is:

- `PropertyInvalidValuePolicy::RejectDeclaration`

That policy is stored in `PropertyMetadata` and is applied by the cascade
integration layer when `CascadeSpecifiedValue::parse(...)` returns a
`SpecifiedValueParseError`.

Rejected declarations remain visible as
`CascadeDeclarationApplicability::InvalidValue(property)` for debug output and
tests. They preserve rejected CSS text and the parse error, but they cannot
become `CascadeDeclarationCandidate`s and cannot win cascade resolution.

## Invalid Detection

Invalid detection is property-aware and driven by the registry:

1. `PropertyId` identifies the supported property.
2. `PropertyMetadata::specified_value` selects the expected specified value
   shape.
3. `PropertyMetadata::length_sign` supplies property-owned range policy for
   length branches.
4. `parse_specified_value(...)` validates the model value against that shape.
5. `PropertyMetadata::invalid_value_policy` decides the declaration-level
   handling for parse failures.

Current representative invalid cases include:

- unsupported keywords such as `display: grid`
- unsupported color names such as `color: nonsense`
- invalid hex colors
- `width: none` and `max-width: auto`
- non-zero unitless lengths
- negative non-margin lengths
- unsupported functions such as `rgb(...)` and `calc(...)`

Layout, paint, and other runtime consumers must not recover from invalid
supported-property values after computed style has been assembled.

## Fallback Contract

Fallback is not local to the rejected declaration. A rejected declaration is
removed from the candidate set, and the existing cascade/default pipeline
selects the final value:

1. another valid winning declaration for the same property, if one exists
2. inherited value when the property inherits and a parent resolved style is
   present
3. the property's initial/default value from `PropertyMetadata::initial`

`ResolvedStyle` records that decision explicitly as:

- `ResolvedValueSource::Winner(...)`
- `ResolvedValueSource::Inherited`
- `ResolvedValueSource::Initial(...)`

This keeps fallback deterministic and inspectable.

## Computed Style Assembly

S5 introduced the typed per-element assembly entrypoint:

- `compute_style_from_resolved_style(...)`
- `ComputedStyle::from_resolved_style(...)`

This path consumes `ResolvedStyle` and records values only through
`ComputedStyleBuilder`.

For each supported property:

- `Winner(...)` must carry a parsed `SpecifiedPropertyValue` for the same
  property, then normalizes through S4
- `Inherited` copies the parent computed value for inherited properties
- `Initial(...)` must match the property's initial/default metadata and is
  converted through `ComputedValue::from_initial(...)`

If an impossible malformed handoff is encountered, assembly returns
`ComputedStyleResolutionError` instead of constructing a partial or invalid
`ComputedStyle`.

S4 normalization failures, such as values outside the current runtime scalar
range, also return errors. They do not enter `ComputedStyle`.

S6 builds on this by adding document-level `ResolvedDocumentStyle` to
`ComputedDocumentStyle` assembly and a structured styled-tree construction
path.

## Determinism

S5 preserves these invariants:

- invalid-value policy comes only from property metadata
- invalid declarations are represented explicitly but produce no candidates
- resolved fallback is total and ordered by the property registry
- computed style assembly iterates the property registry in canonical order
- `ComputedStyleBuilder` remains the invariant gate for totality and value
  kind correctness
- computed style never contains preserved CSS text or invalid specified values

## Out Of Scope

S5 does not implement:

- alternate invalid-value policies beyond `RejectDeclaration`
- CSS-wide keywords such as `inherit`, `initial`, `unset`, `revert`, or
  `revert-layer`
- invalid-at-computed-value-time recovery by rerunning cascade candidate
  selection
- shorthands, custom property substitution, relative units, percentages, or
  function values
- full retirement of the legacy `compute_style(...)` compatibility bridge

Later AD3 adds the current CSS-wide keyword handling contract for supported
properties. In that contract, supported CSS-wide keywords participate in
cascade winner selection, while unsupported CSS-wide keywords such as `revert`
and `revert-layer` are rejected with a dedicated recognized-but-unsupported
error.

## Test Surface

S5 adds and relies on tests for:

- invalid declarations falling back to inherited and initial/default resolved
  sources
- typed `ResolvedStyle` to `ComputedStyle` assembly
- invalid specified values not reaching computed style
- normalization failures aborting computed-style assembly
- inherited resolved entries requiring a parent computed style

These tests define the current invalid-value and fallback contract.
