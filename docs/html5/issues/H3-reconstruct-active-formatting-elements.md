# H3 — Reconstruct Active Formatting Elements

Status: in progress; reconstruction core and evidence are strong, closure is blocked by special formatting-path and AAA integration
Milestone: H — Active formatting elements + adoption agency algorithm

## Goal

Implement the HTML5 "reconstruct the active formatting elements" step for the
Milestone H formatting-element scope, with deterministic DOM/patch behavior and
chunk-equivalent outcomes.

## Current Status

The reconstruction core is landed:

- AFE stores owned formatting-element snapshots (`PatchKey`, `AtomId`,
  attributes).
- reconstruction start-point selection is marker-bounded and identity-based
  against SOE membership
- missing formatting elements are recreated oldest-missing to newest-missing
- recreated nodes receive fresh keys and replace the corresponding AFE entries
  in place
- reconstruction emits direct `DomPatch` element creation and structural
  insertion patches
- golden DOM/patch fixtures now cover reconstruction after normal generic
  ancestor-pop recovery, including a multi-element recreation-order case
- explicit whole-input vs chunked-input parity coverage exists at the session
  level for reconstruction scenarios

## Current Boundary

H3 is not closed yet because surrounding parser integration is still
transitional:

- supported formatting end tags still flow through the generic `In body`
  scope-pop path until AAA lands
- `a` / `nobr` special recovery paths remain deferred
- table-family and template insertion-mode reconstruction call sites remain out
  of scope

This means H3 currently proves the reconstruction algorithm core plus limited
`In body` integration. It does not yet claim full spec-complete formatting
recovery for the entire Milestone H surface. The remaining blockers are tracked
as follow-up issues:

- [`H4 — Special a / nobr recovery paths`](H4-special-a-nobr-recovery.md)
- [`H5 — Adoption Agency Algorithm for supported formatting end tags`](H5-adoption-agency-algorithm.md)

## Evidence Required Before Close

- no regressions in AFE/SOE identity invariants while reconstructed nodes
  replace stale AFE keys
- special `a` / `nobr` recovery paths integrated with reconstruction in the
  supported `In body` flow
- reconstruction behavior validated against the eventual AAA-driven formatting
  end-tag pipeline, not only the current transitional generic scope-pop path

## Remaining Work

- keep the reconstruction golden fixtures and chunk-parity tests active as
  formatting integration expands
- implement the special `a` / `nobr` formatting-element recovery paths
  tracked by [`H4`](H4-special-a-nobr-recovery.md)
- finish AAA and special formatting-element end-tag recovery tracked by
  [`H5`](H5-adoption-agency-algorithm.md) before calling the overall formatting
  recovery pipeline complete
