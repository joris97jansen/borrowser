# R5: Implement Inheritance Behavior For Supported Properties

Last updated: 2026-04-16  
Status: contract and code implemented

This document is the source-of-truth contract for Milestone R issue 5: the
explicit inheritance/default-fill step that turns authored cascade winners into
a total `ResolvedStyle` for Borrowser's current supported property subset.

AF7 reconciliation (2026-08-24): R5's `Option<&ResolvedStyle>` wording is a
historical compatibility API description, not the authoritative architecture.
AF7 source classification receives only typed immediate-parent presence and
inherits the effective value later from the parent `ComputedStyle`. See
`docs/css/af7-specified-value-defaulting-source-resolution.md`.

Related code:
- `crates/css/src/cascade/contract.rs`
- `crates/css/src/cascade.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/r1-cascade-architecture-style-resolution-contract.md`
- `docs/css/r3-core-cascade-winner-resolution.md`
- `docs/css/r4-rule-origin-priority-model-current-scope.md`
- `docs/css/r6-initial-default-value-handling.md`

## Implemented Result

R5 adds the explicit inheritance/default-fill layer through:

- `resolve_cascade_style(...)`
- `resolve_cascade_style_from_rule_inputs(...)`

This layer consumes:

- authored winners in `CascadeWinnerSet`
- typed immediate-parent presence (with the historical public wrapper adapting
  optional parent `ResolvedStyle` to presence only)

and produces:

- a total `ResolvedStyle` over `CascadePropertyId::ALL`

## Inheritance Rules

The supported inherited properties remain:

- `color`
- `font-size`

All other currently supported properties are non-inherited in the cascade
contract.

That policy remains owned by `CascadePropertyId::metadata()`.

## Resolution Algorithm

For each supported property, R5 now resolves in this order:

1. if a local authored winner exists, record `ResolvedValueSource::Winner`
2. otherwise, if the property inherits and an immediate parent exists,
   record `ResolvedValueSource::Inherited`
3. otherwise, record `ResolvedValueSource::Initial(...)`

This means:

- explicit declarations override inherited behavior
- non-inherited properties never inherit accidentally
- inherited properties at the root fall back to their initial value rather than
  recording `Inherited`

## Why Parent Style Matters

Inheritance is no longer an accidental downstream behavior.

Immediate-parent presence is an explicit input to cascade-style construction,
but parent resolved contents are not. Winner selection remains
parent-independent, and computed materialization later reads the parent
computed value.

That preserves the intended staircase:

- `CascadeRuleInput`
- `CascadeDeclarationCandidate`
- `CascadeWinnerSet`
- `ResolvedStyle`

## Determinism Requirements

R5 establishes these invariants:

- inheritance/default fill is total over the supported property subset
- inherited behavior depends only on property metadata, local winners, and
  parent-style presence
- local authored winners always outrank inherited/default behavior
- root-level inherited properties resolve deterministically to their initial
  values
- non-inherited properties always resolve to winner-or-initial, never to
  `Inherited`

## Representative Interactions Covered By Tests

The test surface now covers:

- inherited properties resolving through `Inherited` only when an immediate
  parent is present
- inherited properties at the root falling back to initial values
- explicit winners overriding parent inheritance
- non-inherited properties staying initial even when the parent has a winner
- rule-input -> winner-set -> resolved-style flow without re-deriving priority
  semantics

## Non-Goals

R5 does not:

- compute typed inherited values
- store copied parent authored values in child `ResolvedStyle`
- merge inheritance/default fill into winner selection
- change the legacy DOM-attached style bridge

## Exit Condition For This Issue

This issue is complete when Borrowser can answer, in code:

- which supported properties inherit
- when immediate-parent presence affects child source resolution
- and when the engine must fall back to explicit initial/default values instead

That contract now exists and is covered by unit tests.
