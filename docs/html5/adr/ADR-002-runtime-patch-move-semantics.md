# ADR-002 — Runtime Patch Move Semantics For AAA-Compatible Structural Reparenting

Status: accepted; core runtime/applier move support and move-heavy HTML5 evidence are landed
Milestone: H — Active formatting elements + adoption agency algorithm

## Decision Scope

Define and lock down how Borrowser runtime patch application materializes the
identity-preserving structural moves required by the HTML5 adoption agency
algorithm (AAA).

This ADR removes ambiguity before production AAA code lands.

## Current Implementation Boundary

The Milestone H AFE/AAA contracts now select implicit reparenting via
`AppendChild` / `InsertBefore` as the canonical external move encoding for
identity-preserving structural moves.

Core strict patch appliers/materializers in `crates/html` and
`crates/browser` now implement identity-preserving reparenting/reordering for
legal `AppendChild` / `InsertBefore` moves, including deterministic same-parent
reordering, explicit document/document-root move rejection, and atomic
rollback-on-failure behavior.

This ADR remains implementation-relevant because future parser/runtime changes
must preserve the landed end-to-end HTML5 evidence for move-heavy AAA cases.

## Decision

Borrowser will use implicit reparenting as the canonical patch-stream model for
Milestone H:

- `AppendChild` and `InsertBefore` are defined to accept an already-parented
  `child` and to perform an identity-preserving move
- `RemoveNode` remains destructive subtree removal and is not used as a
  temporary detach step for live-node moves
- explicit detach-then-insert may still be used as an internal runtime
  implementation strategy, but it is not the canonical patch-stream encoding

This selects one stable external contract while still allowing appliers to
implement the move internally however they want, provided externally observable
semantics are identical.

## Normative Decision Details

- Canonical move encoding:
  - HTML5 tree-builder output uses `AppendChild` / `InsertBefore` as the only
    canonical encoding for identity-preserving moves
  - no `MoveNode` or detach-only patch is introduced for Milestone H
- Non-canonical encodings:
  - strict HTML5 appliers may support internal detach-then-insert mechanics,
    but external patch traces remain canonicalized to `AppendChild` /
    `InsertBefore`
  - `RemoveNode` is not a legal substitute for temporary detachment because it
    invalidates the subtree keys
- Legality rules:
  - moving an already-parented element, text, or comment node is legal if the
    target parent is a valid container and the move does not create a cycle
  - moving a document node is illegal
  - moving the document root node is illegal
  - self-attachment and ancestor-cycle creation remain illegal
- Same-parent vs cross-parent behavior:
  - same-parent reordering uses the same canonical move semantics as
    cross-parent reparenting
  - both are modeled by one structural insertion operation on the existing node
- Ordering guarantees:
  - `AppendChild { parent, child }` moves `child` to the end of `parent`'s
    child list after detaching it from any existing parent
  - `InsertBefore { parent, child, before }` moves `child` so it becomes the
    immediate previous sibling of `before` under `parent` after detaching it
    from any existing parent
  - if `child` is already in the requested position, the operation is a
    deterministic structural no-op that preserves identity and sibling order
- `PatchKey` stability:
  - a moved node keeps the same `PatchKey`
  - unaffected nodes keep both key identity and relative order
  - only recreated/replacement nodes receive fresh keys
- Batch/rollback rules:
  - move semantics remain subject to atomic batch application
  - if any patch in the batch fails, the runtime rolls back the whole batch and
    no partial move is observable
- Strict HTML5 requirement:
  - move support is mandatory for strict HTML5-capable appliers
  - appliers that still reject legal reparenting/reordering remain
    non-compliant with the Milestone H HTML5 contract until upgraded

## Acceptance Criteria

- `DomPatch` / runtime contract explicitly documents implicit-reparenting move
  semantics for `AppendChild` / `InsertBefore`.
- Runtime apply path can materialize AAA-required structural moves while
  preserving `PatchKey` identity.
- Tests prove moved nodes keep identity while parent/child ordering remains
  deterministic.
- Tests cover both same-parent reordering and cross-parent reparenting.
- Patch semantics remain chunk-equivalent for representative move-heavy AAA
  cases.
- Illegal move attempts remain explicitly rejected and documented.

## Evidence Expectations

- targeted runtime/applier tests for identity-preserving reparenting
- targeted runtime/applier tests for deterministic same-parent reordering
- HTML5 patch-level tests that exercise AAA move patterns
- whole-input vs chunked-input parity tests showing stable patch semantics
- representative move-heavy cases added here should also be reflected in the
  Milestone H WPT/policy tracking as coverage expands
