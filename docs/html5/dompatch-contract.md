# HTML5 DomPatch Contract (Core v0)

Last updated: 2026-03-02  
Scope: `crates/html/src/html5` (`feature = "html5"`)

This document is the normative contract for patch emission from the HTML5 tokenizer/tree-builder/session pipeline.

## Goals

- Make `DomPatch` the first-class parser output.
- Keep patch ordering deterministic and replayable.
- Define atomic batch and version transition rules for runtime consumers.
- Keep Core v0 behavior explicit while leaving room for future patch variants.

## Patch Types (Core v0 Contract Surface)

Core v0 patch protocol is defined by [`DomPatch`](/Users/jorisjansen/personal/code/borrowser/crates/html/src/dom_patch.rs):

- Node creation:
  - `CreateDocument`
  - `CreateElement`
  - `CreateText`
  - `CreateComment`
- Tree shape / ordering:
  - `AppendChild`
  - `InsertBefore` (defined; currently not emitted by HTML5 tree builder)
  - `RemoveNode` (defined; currently not emitted by HTML5 tree builder for end-tag closure)
- Content mutation:
  - `SetAttributes`
  - `SetText`
  - `AppendText`
- Reset:
  - `Clear`
  - Core v0 strict consumers reject `Clear` batches that do not re-establish a rooted document.

`Close` is not a patch in Core v0; end-tag closure is represented by SOE state transitions and subsequent structural emissions.

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

## Atomic Batch Rules

Batch type is [`DomPatchBatch`](/Users/jorisjansen/personal/code/borrowser/crates/html/src/dom_patch.rs):

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

Tree-builder sink interface lives in [`PatchSink`](/Users/jorisjansen/personal/code/borrowser/crates/html/src/html5/tree_builder/mod.rs):

- `VecPatchSink`: caller-owned buffer sink.
- `CallbackPatchSink`: callback-based streaming sink.

Contract:

1. Sink implementation must preserve patch order.
2. Sink implementation must not mutate patch payload semantics.
3. Tree builder emits through patch-producing operations; structural emissions must flow through structural boundary helpers.

## Test Contract

Core v0 patch correctness uses patch-level golden tests:

- [`html5_golden_tree_builder_patches.rs`](/Users/jorisjansen/personal/code/borrowser/crates/html/tests/html5_golden_tree_builder_patches.rs)
- fixtures in `crates/html/tests/fixtures/html5/tree_builder_patches/`

These tests validate deterministic ordering, batching equivalence (whole vs chunked), and patch protocol stability without relying on DOM snapshot diffing for acceptance.
