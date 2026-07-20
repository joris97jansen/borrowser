# HTML5 DomPatch Contract (Core v0)

Last updated: 2026-03-24  
Scope: `crates/html/src/html5` (`feature = "html5"`)

This document is the normative contract for patch emission from the HTML5 tokenizer/tree-builder/session pipeline.

Related identity contract:
- [`docs/html5/node-identity-contract.md`](node-identity-contract.md)

Related ownership contract:
- [`docs/html5/ae1-html-parser-dom-ownership-contract.md`](ae1-html-parser-dom-ownership-contract.md)
- [`docs/html5/ae2-parser-created-dom-node-model.md`](ae2-parser-created-dom-node-model.md)

## Goals

- Make `DomPatch` the first-class parser output.
- Keep patch ordering deterministic and replayable.
- Define atomic batch and version transition rules for runtime consumers.
- Keep Core v0 behavior explicit while leaving room for future patch variants.

## Ownership Boundary

`DomPatch` is the documented parser output protocol between the HTML tree
builder and runtime consumers. It carries parser-created document/node
construction effects, but it does not expose tokenizer states, insertion modes,
SOE/AFE internals, parse-error recovery decisions, or parser debug counters as
runtime, CSS, Layout, or Paint semantics.

Runtime materializers such as `DomStore` may validate and apply patch batches
atomically. They must not reinterpret malformed-markup recovery, choose
document mode, or treat `PatchKey` identity as retained render identity.

## Patch Types (Core v0 Contract Surface)

Core v0 patch protocol is defined by [`DomPatch`](../../crates/html/src/dom_patch.rs):

- Node creation:
  - `CreateDocument`
  - `CreateDocumentType`
  - `CreateElement`
  - `CreateText`
  - `CreateComment`
  - `CreateTemplateContents { host, contents }`
    - validates an existing canonical `template` host and a fresh contents key,
      creates the typed parser-created fragment, and associates both endpoints
      atomically; it is not an ordinary child edge
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

1. `CreateDocument` appears before any child node emission.
2. An accepted initial doctype emits `CreateDocumentType` followed by
   `AppendChild` to the document before the first document element.
3. Element/text/comment insertion emits `Create*` followed by `AppendChild`.
4. Coalesced adjacent text runs emit `AppendText` on the previously created text key.
5. End-tag SOE pops do not directly emit close/remove patches.
6. An accepted template start emits `CreateElement`, then
   `CreateTemplateContents`, then the host's ordinary structural insertion;
   descendants attach beneath the contents key.

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
- template contents roots cannot be attached or moved with `AppendChild` or
  `InsertBefore`; direct removal while hosted is illegal.

### AE10 Association Lifecycle

There is no detached-fragment create opcode or independent association setter.
Duplicate/re-association and wrong endpoint kinds are rejected. Removing a
template host, or an ordinary ancestor containing it, recursively removes the
associated fragment subgraph. `Clear` removes all associations. Association
replacement beneath a surviving host is represented by reset/rebuild, not live
re-association.

Every independently stored patch/live/runtime/test arena records the fragment
kind explicitly together with its host key. `TemplateContents` is validated at
association creation and again during materialization; a generic fragment name
must not imply template semantics.

Generic `DomPatch` validation owns these structural rules and may validate the
canonical host name. It does not require every manually created element named
`template` to have an association. The HTML5 parser-output validator separately
requires an association for every AE10-accepted template and checks coordinated
parser state before a patch batch can drain.

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

HTML patch validation and parser-owned materialization MUST fail
deterministically for parser-created DOM protocol violations. Runtime appliers
may also reject these shapes, but they do not own HTML semantics such as doctype
placement or malformed-markup recovery. At minimum, the following error classes
are contractual at the HTML validation/materialization boundary:

- unknown/missing node references (`AppendChild`, `InsertBefore`, `RemoveNode`, `Set*`, `AppendText`),
- invalid keys (`PatchKey::INVALID`),
- duplicate key creation within one baseline,
- invalid structural references (`InsertBefore` where `before` is not a child of `parent`),
- invalid parent kind for child attachment (non-container parent),
- invalid child kind for container-only operations (`DocumentType`, `Text`, and
  `Comment` are leaves),
- invalid document-child doctype shape (doctype not directly under document,
  duplicate doctype children, or doctype appearing after the first document
  element),
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

## AE11 namespace-preserving element and attribute transport

`CreateElement` requires an explicit `ExpandedElementName`; no constructor,
validator, decoder, applier, or materializer supplies an implicit HTML
namespace. `SetAttributes` and element creation carry the ordered
`Vec<ParserCreatedAttribute>` unchanged. Each attribute has a valid structured
qualified name and string value. Patch validation and Browser `DomStore`
preserve exact namespace, local-name case, prefix shape, value, and list order.
Changing an element namespace or the ordered structural attribute list is an
observable structural difference even when numeric identity is unchanged.
