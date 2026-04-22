# S8 - Deterministic Debug Output And Regression Coverage

## Status

Implemented.

## Debug Surfaces

S8 defines stable, versioned debug output for the property/value and
computed-style pipeline:

- `ComputedValue::to_debug_label()`
- `computed_value_debug_snapshot(PropertyId, &DeclarationValue)`
- `ComputedStyle::to_debug_snapshot()`
- `ComputedDocumentStyle::to_debug_snapshot()`

These surfaces are maintenance and regression contracts. They are aligned with
engine-facing property ids, specified values, computed values, and computed
styles rather than authored CSS formatting.

Because these helpers are exported from the CSS crate, they are public debug
contracts for engine tests and tools. Changes to their labels, ordering, or
field set must be reviewed as intentional contract changes.

## Determinism Rules

Snapshots must be deterministic across platforms and runs:

- every snapshot starts with `version: 1`
- property iteration follows the canonical property registry order
- computed lengths are serialized in canonical CSS px form
- colors are serialized as canonical RGBA channels
- invalid supported values are reported by stable error labels
- unsupported properties do not appear in computed-value fixtures
- output must not depend on map order, struct field order, or Rust derived
  `Debug`

Snapshots must stay architecture-facing. They may include property ids,
specified-value contracts, computed-value contracts, normalized values, and
stable error labels. They must not grow raw parser internals or incidental
authored formatting unless that data becomes part of the property/value
contract.

## Covered Behavior

The package-level computed golden tests cover:

- specified-value parsing and canonical specified text
- computed-value normalization
- normalization-only failures such as runtime length overflow
- invalid values rejected before computed style
- property-specific range behavior, such as negative margins versus
  non-negative sizing/padding properties
- final document computed-style assembly, including inheritance, defaults,
  invalid fallback, and deterministic document order

## Fixtures

S8 adds golden fixtures under `crates/css/tests/fixtures/computed/`:

- `property_values.css`
- `property_values.snap`
- `document_style.css`
- `document_style.snap`

Fixture changes should be reviewed as contract changes. Behavior changes that
intentionally alter computed output should update these snapshots alongside the
implementation.

## Scope

S8 does not add new CSS property coverage or new computed semantics. It adds the
debug and regression surface needed to keep the existing property system,
specified-value parser, computed normalization, invalid handling, and
computed-style assembly stable as later Milestone S work expands the engine.
