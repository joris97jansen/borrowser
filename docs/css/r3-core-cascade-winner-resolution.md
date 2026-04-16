# R3: Implement Core Cascade Winner Resolution

Last updated: 2026-04-15  
Status: contract and code implemented

This document is the source-of-truth contract for Milestone R issue 3: the
deterministic algorithm Borrowser uses to select the authored winning
declaration for each supported property from the R2 candidate set.

Related code:
- `crates/css/src/cascade/contract.rs`
- `crates/css/src/cascade.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/r1-cascade-architecture-style-resolution-contract.md`
- `docs/css/r2-structured-cascade-inputs-candidate-model.md`

## Implemented Result

R3 adds the core winner-resolution layer between declaration candidates and
resolved-style fill:

- `resolve_cascade_winners(...)`
- `resolve_cascade_winners_from_rule_inputs(...)`
- `CascadeWinnerEntry`
- `CascadeWinnerSet`

This layer is intentionally sparse. It resolves only authored winners. It does
not yet fill inherited or initial/default entries.

## Why This Exists

R2 defined the post-match rule inputs and comparable declaration candidates.
That still left one central question open:

- given the full candidate set, which declaration wins for each property?

R3 answers that question directly and keeps it separate from later inheritance
and initial/default fill work.

## Resolution Model

Winner resolution consumes `CascadeDeclarationCandidate` values.

Each candidate already carries:

- one supported property id
- one authored declaration source
- one fully materialized `CascadePriority`
- one authored specified value

R3 resolves winners in three explicit steps:

1. sort candidates by `CascadeDeclarationCandidateKey`
2. group adjacent candidates by `CascadePropertyId`
3. take the last candidate in each property group as the winner

Because the candidate sort is stable, equal-key behavior is also explicit:

- if two candidates have the same property and the same precedence key,
  the later candidate in the input slice wins

That is a degenerate tie rule, not a meaningful CSS semantic level, but the
behavior is deterministic and tested.

## Ordering Semantics

Winner comparison remains grounded in the R1 cascade precedence contract:

1. origin/importance band
2. specificity
3. rule order
4. declaration order

R3 does not recompute any of those facts. It only consumes the already
materialized candidate key.

## Sparse Winner Output

The output of R3 is `CascadeWinnerSet`.

That set:

- contains only properties with authored winners
- stores entries in canonical property order
- is property-addressable
- has a stable debug snapshot surface

This is intentionally not `ResolvedStyle` yet.

Inheritance/default fill remains a downstream cascade step, so the engine keeps
the staircase explicit:

- `CascadeRuleInput`
- `CascadeDeclarationCandidate`
- `CascadeWinnerSet`
- inheritance/default fill
- `ResolvedStyle`

## Determinism Requirements

R3 winner resolution is deterministic by contract:

- candidate comparison is defined only by `CascadeDeclarationCandidateKey`
- unsupported/custom/invalid declarations never participate because they do not
  produce candidates
- canonical property ordering is independent of candidate discovery order
- equal candidate keys use the documented degenerate tie rule: stable input
  order is preserved and the later input candidate wins

## Non-Goals

R3 does not:

- fill inherited entries
- fill initial/default entries
- compute typed values
- optimize candidate resolution for performance work
- alter the legacy DOM-attached style bridge

## Exit Condition For This Issue

This issue is complete when the cascade engine can take a deterministic
candidate set and answer, in code:

- which authored declaration wins for each supported property
- why that declaration outranked competing declarations
- and do so without relying on incidental parser or DOM vector ordering

That contract now exists and is covered by unit tests.
