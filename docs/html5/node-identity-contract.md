# HTML5 Node Identity Contract (F10)

Last updated: 2026-03-04  
Scope: HTML5 tree builder + runtime_parse integration path

This document defines how node identity maps across:

- internal parser/tree-builder state,
- patch protocol identities,
- runtime document handles and versioning.

Related contracts:

- [`docs/html5/ae1-html-parser-dom-ownership-contract.md`](ae1-html-parser-dom-ownership-contract.md)
- [`docs/html5/ae2-parser-created-dom-node-model.md`](ae2-parser-created-dom-node-model.md)
- [`docs/rendering/ac2-retained-render-identities.md`](../rendering/ac2-retained-render-identities.md)

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
- Parser-created `DocumentType` nodes receive ordinary `PatchKey` identity in
  the patch stream.
- Parser-created processing-instruction nodes receive ordinary `PatchKey`
  identity while retaining exact target/data payloads separately.
- Parser-created template hosts and template-contents roots receive separate,
  stable `PatchKey` identities. The host/contents association is typed and is
  not a parenting edge.
- Parenting invariants:
  - a node has at most one parent,
  - cycles are forbidden,
  - identity-preserving move/reattach is represented by `AppendChild` /
    `InsertBefore` under the HTML5 move-semantics contract,
  - document/document-root moves remain illegal.

### `html::internal::Id` (materialized DOM identity)

- Exposed by materialized `html::Node` values consumed by browser/runtime, CSS,
  Layout, and Paint-facing handoffs.
- `DocumentType` is part of this materialized DOM identity domain when present.
- Processing instructions are part of this materialized DOM identity domain
  and remain typed leaves.
- A typed template-contents fragment and all fragment descendants participate
  in this identity domain even though ordinary traversal does not enter them.
- Today, browser `DomStore` materialization maps live `PatchKey(n)` to
  `Id(n)`.
- That numeric bridge is owned by DOM materialization. It is not a license for
  CSS, Layout, Paint, or retained-rendering code to depend on patch-layer
  allocation policy.
- Matching numeric IDs across separate parser runs or full document
  replacement do not prove DOM continuity or retained render continuity.

### `RetainedRenderId` (browser/runtime render identity)

- Owned by browser/runtime retained rendering, not by HTML/parser or
  `DomStore`.
- Anchored to live materialized DOM provenance where currently representable,
  but separate from `PatchKey` and `html::internal::Id`.
- Non-rendering parser-created nodes such as `DocumentType` and
  `ProcessingInstruction` do not create retained render identity anchors.
- A typed template host and its contents root/descendants create no retained
  render identity anchors.
- Full document replacement starts a new retained render identity domain even
  when fresh parser output produces matching numeric patch keys or DOM IDs.

## Lifetime and Stability Rules

### HTML5 tree builder (`crates/html/src/html5/tree_builder`)

- Keys are allocated by builder-owned monotonic allocator.
- Keys are stable and never reused within one builder instance.
- Emission order is deterministic and source-ordered.
- The AE13b2.2a template child-storage proof performs its explicitly fallible
  reservation before logical insertion, patch emission, or patch-key
  advancement. A failure therefore exposes no identity from the failed
  operation. This narrow guarantee does not yet make other tree-state or patch
  allocations fallible.

### Runtime applier (`crates/browser/src/dom_store.rs`)

- Applies patch batches atomically: all-or-none.
- Rejects unknown/missing keys deterministically.
- `Clear` resets DOM contents and the strict applier's baseline-local
  duplicate-key tracking for that document handle.
- Legal structural moves preserve the moved node's `PatchKey`.
- Key reuse policy in strict applier:
  - keys are non-reusable until `Clear`,
  - keys MAY be reused after `Clear` in a new runtime baseline.

AF4e resolves selector-relevant mutation targets only inside `DomStore`, after
candidate patch application and materialization but before publication commit.
An allocated live key yields a surviving materialized DOM identity; an
allocated non-live key is a valid historical/transient target; and a
never-allocated key is a typed error. A live key without a materialized
identity is also a typed invariant failure. Callers do not duplicate or expose
the internal numeric `PatchKey`/`Id` bridge. Resolution failure preserves the
entire previously committed publication and retained-render state.

AF9 queries committed and staged arena records by `PatchKey`, then uses the
DOM-owned mapping only to produce resolved materialized IDs as neutral
old/final mutation anchors. Direct text-parent identity comes from the arena
parent back-reference through that same mapping; no materialized-tree target
walk or numeric equality inference occurs outside DOM ownership. Browser may
retain surviving IDs, historical-target counts, exact qualified
attributes/text, and a direct text parent identity; it does not turn
them into selector identities or infer dependency meaning. The retained CSS
dependency artifact is separately keyed by `RetainedRenderIdentityDomain` and
the authoritative stylesheet generation. AF9 still executes structural
changes as full Style work; stable source-ID binding of individual retained
style entries remains Milestone AG rather than an implied property of numeric
`PatchKey`/`Id` equality.

The production parser has a stricter, separate session-history contract:
its allocator never reuses a `PatchKey` in the same parser session, including
after `Clear`. AE13 retained-prefix validation enforces that parser-history
rule without changing runtime baseline semantics.

AE13 canonical patch labels are a separate snapshot-local display identity.
They are assigned by first semantic operand appearance, remain monotonic across
`Clear`, and never expose the numeric `PatchKey`.

### Legacy diff path (`runtime_parse` test diff helpers)

- Maps internal `Node::id()` to `PatchKey` via `PatchState::id_to_key`.
- Mapping is stable for a node while present.
- Reset path (`Clear`) rebuilds baseline and resets id-to-key map state.

### AE10 full-model identity

The recursive materialized fragment stores no host ID; physical ownership by
the template element's opaque `ElementNode` is authoritative. Receiver-only
`Node::set_id()` therefore cannot stale an association, and ordinary callers
cannot reach the private association slot. Crate-owned fragment ID mutation
changes only the fragment identity and is used through a controlled
test-harness whole-model transformation where cross-crate legacy tests require
it. Full-model missing-ID assignment, lookup, snapshot traversal
and diff collection use deterministic preorder: host, associated contents root
and its descendants, then ordinary host children. Duplicate detection covers
all ordinary and fragment identities, including nested templates. Ordinary-
tree lookup intentionally does not cross the association.

## Integration Guarantees

AF4a keeps parser-local patch-batch sequencing separate from Browser-visible
`DomVersion`. A mode-bearing publication carries exactly one runtime generation
transition alongside its `DomHandle`; CSS does not receive either identity as
part of its semantic environment.

For the HTML5 runtime path:

- Emitted patch updates MUST satisfy handle/version continuity.
- Emitted patch batches MUST be materializable without unknown-node references.
- A parser fatal failure discards the runtime's unpublished buffer without
  advancing its pending accounting. Previously published batches and their
  identities remain valid and are not rolled back.
- Contract enforcement is test-backed in:
  - `runtime_updates_are_well_formed_and_materializable_if_any`
  - `runtime_emits_updates_for_simple_document_when_strict_enabled`
    (gated by the `runtime_parse` `html5-strict-integration-tests` feature)
  - HTML5 patch golden harness materialization checks (including per-batch incremental checks).

## Non-Goals (Current Milestone)

- Global cross-session key uniqueness.
- Persisted identity across handle replacement strategies.
- Treating `PatchKey`, `html::internal::Id`, or `RetainedRenderId` as
  interchangeable identity domains.

## AE11 semantic-name separation

Namespace-aware parsing does not change identity allocation. `PatchKey` and
materialized `Id` remain stable numeric identities independent of an element's
`ExpandedElementName`. Stack, scope, special-element, selector, and HTML
semantic classification use expanded names; identity-preserving moves retain
the same numeric key. A namespace/name change in diff input requires the
existing structural replacement/reset policy and never a synthesized identity
derived from a namespace URI or local name.
