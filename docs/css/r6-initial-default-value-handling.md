# R6: Define And Implement Initial/Default Value Handling

Last updated: 2026-04-17  
Status: contract and code implemented

This document is the source-of-truth contract for Milestone R issue 6: the
initial/default value model Borrowser's cascade engine uses for the current
supported property subset.

Related code:
- `crates/css/src/cascade/contract.rs`
- `crates/css/src/cascade.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/r1-cascade-architecture-style-resolution-contract.md`
- `docs/css/r3-core-cascade-winner-resolution.md`
- `docs/css/r5-inheritance-behavior-supported-properties.md`

## Implemented Result

R6 makes initial/default handling an explicit cascade-owned surface through:

- `CascadePropertyId::initial_value()`
- `InitialStyleValue`
- `ResolvedValueSource::Initial(...)`
- `ResolvedStyleBuilder::record_initial(...)`
- `resolve_initial_style()`
- `resolve_cascade_style(...)`

This means missing supported properties are never represented as absence in the
final resolved-style output. They resolve either through inheritance, when the
property inherits and a parent resolved style exists, or through an explicit
initial/default value owned by the cascade contract.

## Initial/Default Table

The supported subset has one canonical initial/default value per property:

| property | initial/default contract |
| --- | --- |
| `background-color` | `transparent` |
| `color` | `black` |
| `display` | `inline` |
| `font-size` | `16px` |
| `height` | `auto` |
| `margin-bottom` | `0px` |
| `margin-left` | `0px` |
| `margin-right` | `0px` |
| `margin-top` | `0px` |
| `max-width` | `none` |
| `min-width` | `auto` |
| `padding-bottom` | `0px` |
| `padding-left` | `0px` |
| `padding-right` | `0px` |
| `padding-top` | `0px` |
| `width` | `auto` |

The table is implemented by `CascadePropertyId::metadata()` and exposed through
`CascadePropertyId::initial_value()`. No downstream code should invent fallback
values for this supported subset independently.

## Defaulting Algorithm

For each property in `CascadePropertyId::ALL`, `resolve_cascade_style(...)`
uses this order:

1. if a local authored winner exists, record `ResolvedValueSource::Winner`
2. otherwise, if the property inherits and a parent resolved style exists,
   record `ResolvedValueSource::Inherited`
3. otherwise, record `ResolvedValueSource::Initial(property.initial_value())`

`resolve_initial_style()` is the canonical all-default resolved style. It is
equivalent to resolving an empty winner set with no parent resolved style, and
it exists so the default surface can be tested and consumed directly.

## Boundary With Computed Style

Initial/default values are cascade outputs, not typed computed values.

The computed-style layer remains responsible for interpreting values into
runtime data such as colors, lengths, and display enums. It must consume the
resolved-style contract instead of guessing what should happen when a supported
property is missing.

Important distinction:

- the CSS initial value of `display` in this cascade contract is `inline`
- temporary HTML/UA-style element defaults, such as block display for `div`,
  remain bridge-phase computed-style behavior until the runtime cutover moves
  those responsibilities into an explicit user-agent style source

## Determinism Requirements

R6 establishes these invariants:

- every supported property has exactly one initial/default value
- `resolve_initial_style()` produces a total `ResolvedStyle`
- defaulted entries are explicit `ResolvedValueSource::Initial(...)` records
- root-level inherited properties fall back to initial/default values
- non-inherited properties fall back to initial/default values even when a
  parent resolved style exists
- authored winners always outrank initial/default values
- default snapshot output is canonical property order and independent of caller
  insertion order

## Representative Interactions Covered By Tests

The test surface now covers:

- the full supported-property initial/default table
- canonical all-initial resolved-style construction
- root fallback matching the canonical all-initial style
- explicit winners suppressing defaulting for their property
- missing properties defaulting deterministically
- inheritance and defaulting interacting without downstream guesswork

## Non-Goals

R6 does not:

- broaden the supported property subset
- compute typed values
- implement CSS-wide keywords such as `inherit`, `initial`, `unset`, or
  `revert`
- introduce user-agent stylesheet defaults
- retire the legacy DOM-attached style bridge

## Exit Condition For This Issue

This issue is complete when Borrowser can answer, in code and tests:

- what every supported property's initial/default value is
- how default values are materialized when no declaration wins
- how defaulting interacts with inherited properties at roots and children
- and how downstream code should obtain a complete defaulted style without
  falling back to missing data or implicit guesses

That contract now exists and is covered by unit tests.
