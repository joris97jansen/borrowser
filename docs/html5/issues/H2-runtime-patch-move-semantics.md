# H2 — Runtime Patch Move Semantics Contract For AAA-Compatible Structural Reparenting

Status: follow-up to Milestone H contract foundation  
Milestone: H — Active formatting elements + adoption agency algorithm

## Goal

Define and lock down how Borrowser runtime patch application materializes the
identity-preserving structural moves required by the HTML5 adoption agency
algorithm (AAA).

This issue exists to remove ambiguity before production AAA code lands.

## Current Boundary

The Milestone H AFE/AAA contract requires identity-preserving reparenting for
some recovery paths, but intentionally does not yet choose one canonical patch
encoding for those moves.

Current strict runtime behavior still rejects reattaching an already-parented
node (`MoveNotSupported`), so AAA-compatible move semantics are not yet fully
contracted end-to-end.

## Follow-Up Question

Borrowser must choose and document one of these canonical runtime models:

- implicit reparenting via `AppendChild` / `InsertBefore` on an already-parented
  node
- explicit detach-then-insert move sequences

Either model is acceptable only if it preserves node identity, deterministic
ordering, and chunk-equivalent patch semantics.

## Required Contract Decisions

- Define the canonical move encoding used by HTML5 tree-builder output.
- Define whether non-canonical move encodings are forbidden or merely
  unsupported in strict appliers.
- Define legality rules for moving an already-parented node.
- Define explicit rejection rules for moving root or document nodes.
- Define whether same-parent reordering uses the same canonical move semantics
  as cross-parent reparenting.
- Define ordering guarantees for sibling insertion during moves.
- Define deterministic sibling-index results for both same-parent reordering
  and cross-parent reparenting.
- Define `PatchKey` stability guarantees across moves.
- Define how move semantics interact with batch atomicity and rollback.
- Define whether strict appliers treat move support as mandatory for HTML5 mode.

## Acceptance Criteria

- `DomPatch` / runtime contract explicitly documents canonical move semantics.
- Runtime apply path can materialize AAA-required structural moves while
  preserving `PatchKey` identity.
- Tests prove moved nodes keep identity while parent/child ordering remains
  deterministic.
- Tests cover both same-parent reordering and cross-parent reparenting.
- Patch semantics remain chunk-equivalent for representative move-heavy AAA
  cases.
- Any remaining unsupported move modes are explicitly rejected and documented.

## Evidence Expectations

- targeted runtime/applier tests for identity-preserving reparenting
- targeted runtime/applier tests for deterministic same-parent reordering
- HTML5 patch-level tests that exercise AAA move patterns
- whole-input vs chunked-input parity tests showing stable patch semantics
