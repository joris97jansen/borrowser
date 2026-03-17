# H4 — Special `a` / `nobr` Recovery Paths

Status: in progress; bounded in-scope `a` / `nobr` recovery and chunk-parity evidence are landed, full spec-prescribed recovery remains coupled to H5
Milestone: H — Active formatting elements + adoption agency algorithm

## Goal

Implement the HTML5 `In body` special recovery behavior for start tags `a` and
`nobr`, integrated with Borrowser's active formatting elements (AFE),
reconstruction logic, stack of open elements (SOE), and deterministic
`DomPatch` emission.

## Why This Issue Exists

Milestone H already supports generic formatting-element insertion plus
reconstruction of missing active formatting elements. However, `a` and `nobr`
are not generic formatting-element start-tag paths in the HTML5 tree-builder.
They require dedicated recovery logic that consults existing AFE / SOE state
before inserting a new formatting element.

This issue exists to close that spec gap before the broader formatting recovery
pipeline is treated as complete.

The generic formatting start-tag path and marker insertion groundwork are
tracked separately in
[`H4a — Generic Formatting Start-Tag Handling And AFE/Marker Insertion In The In body Mode`](H4a-generic-formatting-start-tag-handling.md).

## Current Boundary

Current reconstruction work is landed, and the simple in-scope start-tag
recovery cases are now handled through explicit `a` / `nobr` branches:

- repeated `a` insertion removes the earlier active `a` from AFE and closes the
  current in-scope `a` before inserting the new `a`
- repeated in-scope `nobr` insertion closes the current `nobr`, reconstructs as
  needed, and then inserts the new `nobr`
- explicit session-level whole-input vs chunked-input parity tests exist for
  the repeated `a` and repeated `nobr` recovery scenarios

However, the full spec-prescribed recovery remains incomplete until H5 lands:

- complex cases that should flow through adoption-agency recovery still use the
  current bounded non-AAA closure path
- the special start-tag branches no longer delegate blindly to the generic
  formatting helper, but they are not yet validated against the final AAA-aware
  formatting pipeline

## Required Behavior

- `a`:
  - reconstruct active formatting elements where the HTML5 `In body` algorithm
    requires it
  - search AFE after the last marker for an active `a`; searches must not
    cross the most recent marker boundary
  - if found, run the prescribed recovery path before inserting the new `a`
  - preserve deterministic AFE ordering, SOE invariants, and patch-key
    stability for unaffected nodes
- `nobr`:
  - reconstruct active formatting elements where required
  - if `nobr` is already present on the stack of open elements in scope, run
    the prescribed recovery path before reinserting `nobr`
  - keep recovery deterministic and marker-bounded

## Acceptance Criteria

- `a` and `nobr` start tags no longer flow through the generic formatting
  start-tag path
- targeted tests cover repeated `a` insertion and in-scope `nobr` reinsertion
- AFE/SOE state remains deterministic across whole-input and chunked-input runs
- golden DOM/patch fixtures demonstrate the intended recovery behavior
- unaffected node identities remain stable; any recreated nodes receive fresh
  keys in deterministic order
- final close of this issue still requires revalidation of the special paths
  once the H5 adoption-agency path replaces the bounded non-AAA closure logic

## Evidence Expectations

- unit/integration tests for AFE interactions specific to `a`
- unit/integration tests for in-scope `nobr` recovery
- golden fixtures for DOM shape and patch ordering
- explicit chunk-parity coverage for at least one `a` and one `nobr` recovery
  scenario
- representative cases added here should also be reflected in the Milestone H
  WPT/policy tracking as coverage expands

## Dependencies

- builds on [`H3 — Reconstruct Active Formatting Elements`](H3-reconstruct-active-formatting-elements.md)
- builds on [`H4a — Generic Formatting Start-Tag Handling And AFE/Marker Insertion In The In body Mode`](H4a-generic-formatting-start-tag-handling.md)
- should land before treating the Milestone H reconstruction surface as fully
  integrated
- remains orthogonal to the full Adoption Agency Algorithm work tracked by
  [`H5`](H5-adoption-agency-algorithm.md)
