# H4a — Generic Formatting Start-Tag Handling And AFE/Marker Insertion In The `In body` Mode

Status: landed  
Milestone: H — Active formatting elements + adoption agency algorithm

## Goal

Wire the generic `In body` formatting-element start-tag path through active
formatting reconstruction, element insertion, AFE push behavior, and
marker-producing tag boundaries, without yet implementing the special `a` /
`nobr` recovery behavior.

## Why This Issue Exists

Milestone H needs a stable generic formatting-element insertion path before the
special `a` / `nobr` start-tag recovery and the adoption agency algorithm can
land cleanly.

This issue isolates the common start-tag behavior for:

- generic formatting tags that follow the normal formatting insertion flow
- marker-producing tags that must push an AFE marker boundary
- transitional malformed sequences that should remain recoverable and
  panic-free while the special recovery paths are still deferred

## Landed Behavior

- generic formatting start tags in `In body` reconstruct active formatting
  elements before insertion
- non-self-closing formatting start tags insert an element and push a matching
  AFE entry
- self-closing formatting start tags do not remain on SOE and do not enter AFE
- marker-producing tags reconstruct first, insert normally, then push an AFE
  marker when not self-closing
- `a` and `nobr` now have explicit dedicated branches in `handle_in_body`, but
  they still delegate to the generic formatting start-tag helper until the
  special recovery behavior lands

## Acceptance Evidence

- targeted tree-builder tests verify AFE ordering for generic formatting tags
- targeted tree-builder tests verify marker boundaries are inserted into AFE
- targeted tree-builder tests verify self-closing formatting tags do not enter
  AFE
- malformed nested-`a` transitional sequences remain recoverable and
  panic-free

## Current Boundary

This issue intentionally does not implement:

- special `a` recovery when an active `a` exists after the last marker
- special `nobr` recovery when `nobr` is already present on the stack of open
  elements in scope
- adoption agency handling for formatting end tags

Those remain tracked by:

- [`H4 — Special a / nobr recovery paths`](H4-special-a-nobr-recovery.md)
- [`H5 — Adoption Agency Algorithm for supported formatting end tags`](H5-adoption-agency-algorithm.md)
