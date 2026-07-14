# HTML5 DOM And Patch Invariants

Last updated: 2026-03-27  
Scope: `crates/html/src/html5/tree_builder` (`feature = "html5"`)

This document is the K1 contract for structural DOM-state checks and
`DomPatch` batch checks used by tests, fuzz targets, and strict integration
drivers.

Related contracts:
- [`docs/html5/dompatch-contract.md`](dompatch-contract.md)
- [`docs/html5/node-identity-contract.md`](node-identity-contract.md)
- [`docs/html5/ae2-parser-created-dom-node-model.md`](ae2-parser-created-dom-node-model.md)

## Goals

- Keep HTML5 tree-builder output panic-free and deterministic under malformed input.
- Make DOM shape assumptions explicit and machine-checkable.
- Validate patch batches against a concrete pre-batch DOM state.
- Reuse the same checks in tests and future fuzz targets.

## DOM Invariants

These are checked by `check_dom_invariants(dom)` over
`html::html5::tree_builder::DomInvariantState`.

Allowed baseline:
- an empty state is valid before the first `CreateDocument`
- otherwise the state must be rooted

Required invariants:
- the tree is acyclic
- there is at most one document node, and it is the declared root
- the root, when present, is a document node and has no parent
- every non-root node has exactly one parent
- every parent/child edge is bidirectionally consistent
- child lists contain no duplicate node references
- every referenced parent and child exists
- sibling order is explicit and preserved by the stored child vector
- only document and element nodes may have children
- doctype, text, and comment nodes are leaves
- a doctype node, when present, is a direct child of the document
- at most one doctype child is present
- the doctype child appears before the first document element child

Operational interpretation:
- "children order is stable" means the DOM state carries one concrete child
  order per parent and the checker rejects duplicate or contradictory sibling
  references that would make that order ambiguous
- detached non-root nodes are invalid final DOM state

## Patch Batch Invariants

These are checked by `check_patch_invariants(patches, dom_state)`.

Baseline rules:
- `dom_state` itself must already satisfy `check_dom_invariants`
- patch validation is batch-scoped and order-sensitive

Required invariants:
- `PatchKey::INVALID` must never appear
- `Create*` operations must introduce a fresh key before any later reference
- `Clear` may only appear as the first patch in a batch
- a `Clear` batch must re-establish a rooted document by the end of the batch
- `AppendChild` / `InsertBefore` must reference existing nodes
- structural parents must be container nodes (`Document` or `Element`)
- `InsertBefore.before` must already be a child of the specified parent
- move/reparent operations must not move a document node
- move/reparent operations must not move the document root element
- move/reparent operations must not create ancestor cycles
- `RemoveNode` must target a live attached node or the root
- `SetAttributes` only targets element nodes
- `SetText` and `AppendText` only target text nodes
- the final post-batch DOM state must satisfy the DOM invariants above

## API Surface

Current checker entrypoints:
- `check_dom_invariants(&DomInvariantState) -> Result<(), DomInvariantError>`
- `check_patch_invariants(&[DomPatch], &DomInvariantState) -> Result<DomInvariantState, PatchInvariantError>`
- `Html5TreeBuilder::dom_invariant_state() -> DomInvariantState`

Recommended usage:
1. capture the builder state before a batch
2. run the parser step and collect emitted patches
3. call `check_patch_invariants` with the pre-batch state
4. compare the returned post-batch state to `builder.dom_invariant_state()`

This keeps the emitted patch stream and the builder's internal live tree under
the same structural contract.

## Internal Live-Tree Boundary

`LiveTree` inside the HTML5 tree builder is an internal structural mirror, not a
public typed validator.

Contract:
- it mirrors structural edits that the tree builder already considers authoritative
- it is assertion-based by design
- a `LiveTree` panic indicates a Borrowser tree-builder bug, not malformed HTML
- fuzzers and invariant-aware tests should rely on `check_dom_invariants` and
  `check_patch_invariants` for typed failure reporting

## AE8 Table Parser-State Invariants

AE8 adds parser-state invariants for the supported table tree-construction
subset. These invariants are internal regression contracts, not public runtime
APIs.

Pending table-character state:

- `InTableText` owns one pending table-text state containing both the
  table-character buffer and recorded original table-family insertion mode.
- the recorded mode inside that state must be one of `InTable`, `InTableBody`,
  or `InRow`;
- entering `InTableText` while pending table-text state already exists is an
  internal invariant violation and must not replace or discard the active state;
- an `InTableText` mode without pending table-text state is an internal
  invariant violation, not a recoverable HTML parse error;
- leaving `InTableText` through a non-character token or EOF must flush the
  buffer and take the return mode before reprocessing that token;
- the pending buffer and recorded return mode must both be clear together after
  the flush;
- parser finalization through EOF must not leave pending table-character state.

Foster-parenting state:

- foster parenting resolves an `InsertionLocation` before text or element
  insertion;
- the location is either append-to-parent or insert-before-anchor;
- when the relevant table has a live parent, the emitted structural patch for a
  newly created foster-parented node must use that parent and the table as the
  `InsertBefore.before` anchor;
- when the relevant table has no live parent, the fallback parent is the stack
  entry immediately above the table;
- foster parenting must not use `RemoveNode`, DOM rebuilding, post-parse
  normalization, runtime repair, or layout ancestry.

Table stack operations:

- table-scope checks are read-only stack probes;
- table-context, table-body-context, and table-row-context clearing pop parser
  stack entries only;
- stack clearing does not remove, detach, or reorder DOM nodes;
- implied table wrappers are real parser-created DOM nodes with fresh
  `PatchKey` identities.

Table cell and AFE interaction:

- entering `td` or `th` pushes an AFE marker after inserting the cell;
- closing a cell, explicit or implied, clears AFE entries back to the last
  marker;
- cell close recovery must not expand into unrelated adoption-agency behavior.

## AE9a Form And Void-Insertion Invariants

- A form pointer is absent or identifies one successfully parser-created form
  `PatchKey`; clearing it never removes a DOM node.
- Form end processing clears the pointer before scope validation and exact stack
  removal; a failed validation leaves it clear.
- Exact-key stack removal preserves other entry order, counts, caches, and DOM.
- Pending textarea initial-LF state is valid only for the active textarea RCDATA
  entry and is cleared by first text consumption, non-text handling, or text-mode exit.
- A void insertion restores retained stack length/order after one real push/pop;
  high-water records the transient observed depth.
- AE9 start-tag dispatch finalizes every original self-closing flag exactly once:
  an unacknowledged AE9 non-void flag records its trailing-solidus error after
  the tag-specific recovery error, including a recoverably ignored token.
- The frozen deprecated insertion helper retains pre-AE9 skip-stack behavior;
  only AE9 semantic void insertion changes stack-transition observability.
