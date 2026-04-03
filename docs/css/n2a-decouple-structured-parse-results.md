# N2a: Decouple Structured Parse Results From Compatibility Stylesheet Outputs

Last updated: 2026-04-03  
Status: queued after N1

## Issue

Introduce a real syntax-layer stylesheet representation for
`parse_stylesheet_with_options` so the structured parse result no longer exposes
`CompatStylesheet` directly. Preserve the current cascade path by adding an
explicit compatibility projection from the syntax-layer representation into
`CompatStylesheet`.

## Why This Exists

N1 correctly established that `CompatSelector`, `CompatRule`, and
`CompatStylesheet` are transitional adapter types for the existing cascade path.
However, `StylesheetParse` still exposes `CompatStylesheet` as its primary parse
result, which means the structured parse result is still coupled to the
compatibility layer.

That coupling is acceptable for N1 because the goal was contract clarity rather
than final parser internals. It should not survive into the tokenizer-driven
implementation work for later Milestone N issues.

## Goals

- add a syntax-layer stylesheet representation that is not compatibility-scoped
- keep `StylesheetParse` aligned with syntax-layer output rather than cascade
  adapter output
- preserve the current browser behavior through an explicit compatibility
  projection into `CompatStylesheet`
- document the relationship between syntax-layer output and compatibility
  projection clearly

## Non-Goals

- full selector grammar implementation
- cascade refactors beyond what is needed for the compatibility projection
- computed-style or value-parsing changes

## Expected Direction

Preferred architecture:

1. `parse_stylesheet_with_options` returns a syntax-layer stylesheet result
2. a separate conversion/projection step produces `CompatStylesheet`
3. `parse_stylesheet` may continue returning `CompatStylesheet` for migration
   convenience
4. the cascade layer continues consuming `CompatStylesheet` until its own later
   migration

## Exit Criteria

- `StylesheetParse` no longer stores `CompatStylesheet` as its primary parse
  result
- a syntax-layer representation exists for stylesheet parse output
- an explicit compatibility conversion exists for the current cascade path
- `parse_stylesheet()` still works as a migration convenience entry point
- docs explain the relationship between syntax-layer parse output and
  compatibility projection
