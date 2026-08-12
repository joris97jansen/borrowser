# N9: Expand Selector Syntax Structure Beyond Compatibility Projection

Last updated: 2026-08-12
Status: superseded by Milestone P and AF2

Tracker note:
- this issue was originally queued in-repo under `N5`
- it was renumbered to `N6` once deterministic parse recovery became the
  canonical implemented `N5`
- it was then renumbered to `N7` once stable syntax-layer debug/serialization
  output became the implemented `N6`
- it was then renumbered to `N8` once resource limits and parser invariants
  became the implemented `N7`
- it was then renumbered to `N9` once parser contract cutover/documentation
  became the implemented `N8`

## Historical status

This proposal is no longer an active implementation direction. Its original
motivation—to prevent `CompatSelector` from becoming the permanent selector
representation—was fulfilled by Milestone P and reconciled by AF2.

`css::selectors` now owns the permanent selector AST/parser and
`StyleRule::selectors` stores its structured parse result. `css::syntax` keeps
its generic token/component-value/recovery ownership. AF2 does not introduce a
second selector AST under `css::syntax`.

## Original issue

Introduce a real structured selector representation so stylesheet parsing no
longer depends on projecting qualified-rule preludes directly into the limited
`CompatSelector` model used by the current cascade path.

N4 established the stylesheet AST, structured declaration handling, and an
explicit compatibility projection. The largest remaining syntax-side gap is now
selector structure: qualified-rule preludes are preserved as generic component
values, but selector syntax is not yet represented explicitly inside the
syntax-layer AST.

## Original rationale

The historical parser architecture was strong on the rule/block/value side:

- tokenization is real and deterministic
- stylesheet parsing is AST-oriented
- compatibility projection is explicit and no longer the parser contract

At the time, selector handling still relied on a narrow compatibility adapter:

- `CompatSelector` only models universal, type, id, and class selectors
- selector parsing for the cascade path is projection logic, not syntax-layer
  AST structure
- later selector milestones will need explicit selector syntax nodes rather
  than generic prelude preservation alone

This issue exists to make selector syntax first-class in the syntax layer
without dragging cascade semantics into the parser.

## Original goals

- introduce selector structures separate from `CompatSelector`
- parse qualified-rule preludes into explicit selector representations where
  supported
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

## Superseded preferred direction

Preferred architecture:

1. qualified rules continue to own preserved prelude/component-value structure
2. `css::selectors` consumes those values and owns selector-list and selector
   node structures
3. supported selector syntax produces typed selector IR during model
   integration
4. unsupported selector syntax remains recoverable and deterministic
5. compatibility projection into `CompatSelector` remains separate and
   migration-scoped

## Historical exit criteria

- syntax-layer selector structures exist in code
- qualified-rule parsing exposes selector syntax more explicitly than raw
  generic prelude values alone
- compatibility projection no longer acts as the de facto selector parser
- selector parsing remains deterministic, bounded, and testable
- docs clearly describe the boundary between selector syntax parsing and later
  selector semantics/matching
