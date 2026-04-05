# N8: Expand Selector Syntax Structure Beyond Compatibility Projection

Last updated: 2026-04-05  
Status: queued after N7

Tracker note:
- this issue was originally queued in-repo under `N5`
- it was renumbered to `N6` once deterministic parse recovery became the
  canonical implemented `N5`
- it was then renumbered to `N7` once stable syntax-layer debug/serialization
  output became the implemented `N6`
- it was then renumbered to `N8` once resource limits and parser invariants
  became the implemented `N7`

## Issue

Introduce a real structured selector syntax layer so stylesheet parsing no
longer depends on projecting qualified-rule preludes directly into the limited
`CompatSelector` model used by the current cascade path.

N4 established the stylesheet AST, structured declaration handling, and an
explicit compatibility projection. The largest remaining syntax-side gap is now
selector structure: qualified-rule preludes are preserved as generic component
values, but selector syntax is not yet represented explicitly inside the
syntax-layer AST.

## Why This Exists

The current parser architecture is now strong on the rule/block/value side:

- tokenization is real and deterministic
- stylesheet parsing is AST-oriented
- compatibility projection is explicit and no longer the parser contract

However, selector handling still relies on a narrow compatibility adapter:

- `CompatSelector` only models universal, type, id, and class selectors
- selector parsing for the cascade path is projection logic, not syntax-layer
  AST structure
- later selector milestones will need explicit selector syntax nodes rather
  than generic prelude preservation alone

This issue exists to make selector syntax first-class in the syntax layer
without dragging cascade semantics into the parser.

## Goals

- introduce syntax-layer selector structures separate from `CompatSelector`
- parse qualified-rule preludes into explicit selector syntax representations
  where supported
- preserve unsupported selector syntax deterministically for recovery and later
  expansion
- keep selector parsing lexical/syntactic rather than DOM-matching or
  specificity-evaluation oriented
- maintain an explicit compatibility projection into `CompatSelector` for the
  current cascade path

## Non-Goals

- selector matching against DOM nodes
- full Selectors Level 4 coverage
- cascade or specificity redesign beyond the compatibility projection boundary
- computed-style or value-parsing work

## Preferred Direction

Preferred architecture:

1. qualified rules continue to own preserved prelude/component-value structure
2. the syntax layer adds explicit selector-list and selector-node structures
3. supported selector syntax projects into those structures during parsing
4. unsupported selector syntax remains recoverable and deterministic
5. compatibility projection into `CompatSelector` remains separate and
   migration-scoped

## Exit Criteria

- syntax-layer selector structures exist in code
- qualified-rule parsing exposes selector syntax more explicitly than raw
  generic prelude values alone
- compatibility projection no longer acts as the de facto selector parser
- selector parsing remains deterministic, bounded, and testable
- docs clearly describe the boundary between selector syntax parsing and later
  selector semantics/matching
