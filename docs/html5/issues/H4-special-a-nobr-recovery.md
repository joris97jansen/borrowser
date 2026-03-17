# H4 — Special `a` / `nobr` Recovery Paths

Status: ready  
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

## Current Boundary

Current reconstruction work is landed, but these special paths are still
deferred:

- start tag `a` does not yet run the prescribed recovery when an active `a`
  exists after the last marker
- start tag `nobr` does not yet run the prescribed recovery when `nobr` is
  already present on SOE in scope

Generic formatting-element insertion is not sufficient for either case.

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
- should land before treating the Milestone H reconstruction surface as fully
  integrated
- remains orthogonal to the full Adoption Agency Algorithm work tracked by
  [`H5`](H5-adoption-agency-algorithm.md)
