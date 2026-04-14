# Q3: Implement Simple Selector Matching

Last updated: 2026-04-14  
Status: implemented

This document is the source-of-truth contract for Milestone Q issue 3:
implementing the first real selector-to-DOM evaluation layer for the currently
supported element-local selector subset.

Historical note:

- this document records the Q3 landing scope
- Q5 later extended the same matcher entrypoint to full complex-selector
  traversal for the supported selector IR

Related code:
- `crates/css/src/selectors/matching.rs`

Related documents:
- `docs/css/q1-selector-matching-architecture.md`
- `docs/css/q2-selector-matching-context.md`
- `docs/css/p2-selector-ir-data-structures.md`

## Implemented Result

Q3 adds real element-local selector evaluation through:

- `SelectorMatchingContext::matches_compound_selector(...)`
- `SelectorMatchingContext::match_selector_list(...)`

The matcher now evaluates these supported selector classes against individual
elements:

- universal selectors
- named type selectors
- id selectors
- class selectors
- attribute selectors in the Milestone P supported subset

Compound selectors built from those simple selector classes are also supported
as long as matching remains element-local.

## Evaluator Boundary

Q3 still does not implement combinator traversal.

The active matcher subset is therefore:

- parsed selector lists whose `ComplexSelector` entries have empty `tail()`
- compound selectors composed only of currently supported simple selector
  classes

Parsed selector lists that require combinator traversal are handled
conservatively:

- they return `SelectorListMatchOutcome::unsupported()`
- they are not partially reinterpreted by matching only their head compound
- mixed lists are likewise treated as unsupported for this evaluator stage

This is deliberate. It preserves deterministic behavior and avoids introducing
temporary partial-matching semantics that later full selector evaluation would
have to undo.

Historical staging note:

- at Q3 landing time there were two `Unsupported` origins at match time:
  - parser-level unsupported selector input
  - parsed selector input outside the active evaluator subset
- Q5 later removed the second category by implementing real complex-selector
  traversal for the supported selector IR

## Matching Semantics

`matches_compound_selector(...)` evaluates:

- the optional type selector first
- subclass selectors in source order
- with short-circuit failure on the first non-match

This keeps compound matching deterministic and aligned with the selector IR.

`match_selector_list(...)` evaluates one selector list against one target
element:

- `Invalid` parse results produce `Invalid` match outcomes
- parser-level `Unsupported` selector lists produce `Unsupported` match outcomes
- parsed selector lists outside the current evaluator subset also produce
  `Unsupported` match outcomes
- supported parsed selector lists produce `Parsed` outcomes with matched
  selectors recorded in source order and with IR-derived specificity

This meant the Q3 match outcome surface was already stable enough for engine
integration, while still allowing later matcher milestones to reduce the
conservative fallback without changing the entry point or result structure.

## Determinism Requirements

Q3 matching is deterministic by contract:

- type/id/class/attribute semantics are inherited from the centralized Q2
  query layer
- matched selectors are recorded by selector-list source order
- specificity is taken from selector IR rather than recomputed ad hoc
- parsed selectors outside the active evaluator subset are rejected
  consistently instead of partially matched

## Test Surface

Q3 adds unit coverage for:

- compound selector matching against a single element
- selector-list matching over supported element-local selectors
- supported selector lists that produce no matches
- propagation of invalid and unsupported parse states into match outcomes
- conservative rejection of parsed selector lists that require combinator
  traversal

## Non-Goals

Q3 does not:

- implement descendant, child, adjacent-sibling, or general-sibling traversal
- evaluate full complex selectors
- integrate matching into final cascade winner resolution
- add caching or invalidation

Those remain later Milestone Q work.
