# R4: Define Rule Origin And Priority Model For The Current CSS Scope

Last updated: 2026-04-16  
Status: contract and code implemented

This document is the source-of-truth contract for Milestone R issue 4: the
explicit rule origin/priority model Borrowser uses in the current CSS scope and
how that model feeds winner resolution.

Related code:
- `crates/css/src/cascade/contract.rs`
- `crates/css/src/cascade.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/r1-cascade-architecture-style-resolution-contract.md`
- `docs/css/r2-structured-cascade-inputs-candidate-model.md`
- `docs/css/r3-core-cascade-winner-resolution.md`

## Implemented Result

R4 makes the current rule origin/priority model explicit in code through:

- `CascadeOrigin`
- `CascadeImportance`
- `CurrentScopeCascadePriorityBand`
- `CascadeOriginBand`

The key distinction is:

- `CurrentScopeCascadePriorityBand` is the ordered priority model actually
  emitted by Borrowser's current CSS scope
- `CascadeOriginBand` remains the broader long-term precedence surface that
  also reserves future animation and transition levels

This keeps the hot path honest about current behavior without blocking future
CSS origin/layer expansion.

## Current Scope Model

The current emitted priority bands are:

1. `UserAgentNormal`
2. `UserNormal`
3. `AuthorNormal`
4. `AuthorImportant`
5. `UserImportant`
6. `UserAgentImportant`

They are derived directly from the current cross-product of:

- `CascadeOrigin`
- `CascadeImportance`

This derivation now lives in
`CurrentScopeCascadePriorityBand::from_origin_and_importance(...)`.

## Integration Into Comparison

The current-scope priority model participates in winner comparison through
`CascadeRuleContext::priority_for_declaration(...)`.

That path now does three explicit things:

1. combine rule origin with declaration importance into
   `CurrentScopeCascadePriorityBand`
2. map that current-scope band into the broader `CascadeOriginBand`
3. construct the final `CascadePriority`

This means:

- the current emitted model is explicit and testable on its own
- the final comparison key remains compatible with future reserved precedence
  bands
- inspection helpers such as `CascadePriority::current_scope_band()` are
  intentionally partial and return `None` for reserved future precedence levels

## Why The Model Is Split

Borrowser needs two different concepts here:

- the priority model emitted by the current engine scope
- the broader precedence space later milestones may emit

Those are related, but they are not the same thing.

If the current hot path used only the future-looking enum directly, the code
would blur current behavior and reserved behavior together. R4 avoids that by
making the current emitted model its own explicit type.

## Future Extension Path

Future work may add precedence levels that are not representable as an
origin-plus-importance pair in the current engine path, such as:

- animation
- transition
- later origin/layer expansions if Borrowser adopts them

Those future levels remain represented in `CascadeOriginBand`, but they are not
part of `CurrentScopeCascadePriorityBand`.

That keeps unsupported future semantics out of the current rule-priority hot
path while leaving the comparison model extensible.

The debug labels for `CurrentScopeCascadePriorityBand` intentionally mirror the
matching current-scope `CascadeOriginBand` labels. When a debug surface needs
to distinguish the emitted current model from the broader precedence space, it
should do so via type/context rather than by expecting different label strings.

## Determinism Requirements

R4 establishes these invariants:

- the current origin/priority model is ordered explicitly
- the mapping from `(CascadeOrigin, CascadeImportance)` to current-scope band is
  deterministic
- the mapping from current-scope band to `CascadeOriginBand` is deterministic
- winner resolution consumes that resulting priority metadata without
  re-deriving origin semantics

## Representative Interactions Covered By Tests

The test surface now covers:

- explicit mapping of origin/importance combinations into current-scope bands
- author normal beating user normal and user-agent normal
- important priority bands outranking normal bands
- user-important beating author-important
- preservation of the broader future precedence ordering in
  `CascadeOriginBand`

## Non-Goals

R4 does not:

- implement cascade layers
- emit animation or transition declarations
- redefine inline-style specificity behavior
- merge priority modeling with inheritance/default fill
- merge priority modeling with computed-value parsing

## Exit Condition For This Issue

This issue is complete when Borrowser can answer, in code:

- what rule origin/priority combinations exist in the current engine scope
- how those combinations participate in cascade comparison
- and how later extension remains possible without redesigning the hot path

That contract now exists and is covered by unit tests.
