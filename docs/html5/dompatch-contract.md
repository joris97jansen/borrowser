# HTML5 DomPatch Contract (Core v0)

Last updated: 2026-03-24  
Scope: `crates/html/src/html5` (`feature = "html5"`)

This document is the normative contract for patch emission from the HTML5 tokenizer/tree-builder/session pipeline.

Related identity contract:
- [`docs/html5/node-identity-contract.md`](node-identity-contract.md)

## Goals

- Make `DomPatch` the first-class parser output.
- Keep patch ordering deterministic and replayable.
- Define atomic batch and version transition rules for runtime consumers.
- Keep Core v0 behavior explicit while leaving room for future patch variants.

## Patch Types (Core v0 Contract Surface)

Core v0 patch protocol is defined by [`DomPatch`](../../crates/html/src/dom_patch.rs):

- Node creation:
  - `CreateDocument`
  - `CreateElement`
  - `CreateText`
  - `CreateComment`
- Tree shape / ordering:
  - `AppendChild`
    - defined as identity-preserving implicit reparenting when `child` is
      already parented
  - `InsertBefore`
    - defined as identity-preserving implicit reparenting/reordering when
      `child` is already parented
  - `RemoveNode` (defined; currently not emitted by HTML5 tree builder for end-tag closure)
- Content mutation:
  - `SetAttributes`
  - `SetText`
  - `AppendText`
- Reset:
  - `Clear`
  - Core v0 strict consumers reject `Clear` batches that do not re-establish a rooted document.

`Close` is not a patch in Core v0; end-tag closure is represented by SOE state transitions and subsequent structural emissions.

### Move Semantics Confirmation (I8)

Milestone I confirms that the existing structural patch model is sufficient for
table-driven foster parenting and complex reparenting. No new `MoveNode`,
`ReparentNode`, or detach-only patch opcode is added.

Normative consequence:

- `AppendChild` and `InsertBefore` with an already-created `child` are the
  minimal engine-grade move surface for Core v0
- same-parent reorder and cross-parent reparent remain one contract surface,
  not separate opcode families
- parser/runtime code must not encode temporary-detach moves with `RemoveNode`

## Ordering Rules

For all patch streams:

1. Patches are applied strictly in stream order.
2. `Create*` must appear before first reference to their key in structural/content patches.
3. Structural child ordering is explicit via `AppendChild` / `InsertBefore`; consumers must not reorder.
4. `Clear` may only appear as the first patch in a batch.
5. `PatchKey(0)` is invalid and must never appear in emitted patches.

HTML5 tree-builder Core v0 emission profile:

1. `CreateDocument` appears before any non-doctype-attached node emission.
2. Element/text/comment insertion emits `Create*` followed by `AppendChild`.
3. Coalesced adjacent text runs emit `AppendText` on the previously created text key.
4. End-tag SOE pops do not directly emit close/remove patches.

Structural move semantics for HTML5-capable appliers:

- `AppendChild { parent, child }` MAY attach a detached node or move an
  already-parented node to the end of `parent`'s child list while preserving
  `PatchKey` identity.
- `InsertBefore { parent, child, before }` MAY attach a detached node or move
  an already-parented node so it becomes the immediate previous sibling of
  `before` while preserving `PatchKey` identity.
- same-parent reordering and cross-parent reparenting use the same structural
  insertion semantics.
- `RemoveNode` is destructive subtree removal, not a temporary detach
  primitive.
- moving a document node or document root node is illegal.
- successful moves must leave no dangling parent references, dangling child
  references, duplicate sibling references, or cycles in the materialized tree.
- if a move attempt fails, atomic batch rollback means no partial detach or
  half-applied reparent is observable.

## Atomic Batch Rules

Batch type is [`DomPatchBatch`](../../crates/html/src/dom_patch.rs):

- A batch is atomic: apply all patches or none.
- Batch boundaries are flush boundaries (`take_patches` / `take_patch_batch`) and must preserve in-batch order.
- Batches for one document must not interleave with another document handle.
- Runtime appliers must reject and roll back on the first protocol violation in a batch.
- Strict runtime apply APIs reject empty patch slices (`[]`).

Strict runtime rule (Core v0):

- A batch starting with `Clear` must produce a rooted document state by the end
  of the batch (`CreateDocument` required in practice).

## Version Rules

`DomPatchBatch` carries `{ from, to, patches }` and requires:

- `to = from + 1` for every non-empty batch.
- Empty drains do not advance version.
- Version progression is monotonic per parse session/document handle.

Session API surface:

- `Html5ParseSession::take_patches()` returns raw patches (legacy compatibility).
- `Html5ParseSession::take_patch_batch()` returns atomic versioned batches.

## Sink Contract

Tree-builder sink interface lives in [`PatchSink`](../../crates/html/src/html5/tree_builder/mod.rs):

- `VecPatchSink`: caller-owned buffer sink.
- `CallbackPatchSink`: callback-based streaming sink.

Contract:

1. Sink implementation must preserve patch order.
2. Sink implementation must not mutate patch payload semantics.
3. Tree builder emits through patch-producing operations; structural emissions must flow through structural boundary helpers.

## Deterministic Materialization Failures

Patch materialization and strict runtime application MUST fail deterministically for protocol violations.
At minimum, the following error classes are contractual:

- unknown/missing node references (`AppendChild`, `InsertBefore`, `RemoveNode`, `Set*`, `AppendText`),
- invalid keys (`PatchKey::INVALID`),
- duplicate key creation within one baseline,
- invalid structural references (`InsertBefore` where `before` is not a child of `parent`),
- invalid parent kind for child attachment (non-container parent),
- cycle/self-attachment attempts,
- illegal move attempts (for example document/document-root moves, cycle
  creation, or move support disabled in a non-HTML5-capable applier)
  (`MoveNotSupported`),
- wrong node kind for content operations (`SetAttributes` on non-element, `SetText`/`AppendText` on non-text),
- batch protocol violations (for example `Clear` not first, clear-only batch in strict appliers, rootless state where disallowed).

## Test Contract

Core v0 patch correctness uses patch-level golden tests:

- [`html5_golden_tree_builder_patches.rs`](../../crates/html/tests/html5_golden_tree_builder_patches.rs)
- fixtures in `crates/html/tests/fixtures/html5/tree_builder_patches/`

These tests validate deterministic ordering, batching equivalence (whole vs chunked), and patch protocol stability without relying on DOM snapshot diffing for acceptance.

Move-specific evidence for Core v0 / Milestone I includes:

- runtime/applier unit tests for same-parent reorder and cross-parent reparent
- HTML5 patch goldens that explicitly cover:
  - `AppendChild`-encoded existing-node moves
  - `InsertBefore`-encoded foster-parent moves
  - deterministic ordering under whole vs chunked execution
