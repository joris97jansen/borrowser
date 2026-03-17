# HTML5 Node Identity Contract (F10)

Last updated: 2026-03-04  
Scope: HTML5 tree builder + runtime_parse integration path

This document defines how node identity maps across:

- internal parser/tree-builder state,
- patch protocol identities,
- runtime document handles and versioning.

## Identity Domains

### `DomHandle` (runtime document identity)

- Allocated by `runtime_parse` on `ParseHtmlStart`.
- Stable for the lifetime of one parse session (`tab_id`, `request_id` pair).
- Patch updates for one parse session MUST use exactly one handle.
- A new parse session MUST allocate a new handle.

### `DomVersion` (update sequence identity)

- Scoped to one `DomHandle`.
- Monotonic and contiguous for non-empty updates:
  - each update is `from -> to` where `to = from.next()`.
- Empty drains MUST NOT emit updates and MUST NOT advance version.

### `PatchKey` (node identity in patch streams)

- Non-zero only (`PatchKey::INVALID` is forbidden in emitted patches).
- References in non-create patches MUST point to live/known nodes.
- `Create*` introduces a key before first use by structure/content patches.
- Parenting invariants:
  - a node has at most one parent,
  - cycles are forbidden,
  - identity-preserving move/reattach is represented by `AppendChild` /
    `InsertBefore` under the HTML5 move-semantics contract,
  - document/document-root moves remain illegal.

## Lifetime and Stability Rules

### HTML5 tree builder (`crates/html/src/html5/tree_builder`)

- Keys are allocated by builder-owned monotonic allocator.
- Keys are stable and never reused within one builder instance.
- Emission order is deterministic and source-ordered.

### Runtime applier (`crates/browser/src/dom_store.rs`)

- Applies patch batches atomically: all-or-none.
- Rejects unknown/missing keys deterministically.
- `Clear` resets DOM contents and key-allocation domain for that handle baseline.
- Legal structural moves preserve the moved node's `PatchKey`.
- Key reuse policy in strict applier:
  - keys are non-reusable until `Clear`,
  - keys MAY be reused after `Clear`.

### Legacy diff path (`runtime_parse` test diff helpers)

- Maps internal `Node::id()` to `PatchKey` via `PatchState::id_to_key`.
- Mapping is stable for a node while present.
- Reset path (`Clear`) rebuilds baseline and resets id-to-key map state.

## Integration Guarantees

For the feature-gated HTML5 runtime path (`runtime_parse/html5`):

- Emitted patch updates MUST satisfy handle/version continuity.
- Emitted patch batches MUST be materializable without unknown-node references.
- Contract enforcement is test-backed in:
  - `runtime_html5_mode_updates_are_well_formed_and_materializable_if_any`
  - `runtime_html5_mode_emits_updates_for_simple_document_when_strict_enabled`
    (gated by `runtime_parse/html5-strict-integration-tests`)
  - HTML5 patch golden harness materialization checks (including per-batch incremental checks).

## Non-Goals (Current Milestone)

- Global cross-session key uniqueness.
- Persisted identity across handle replacement strategies.
