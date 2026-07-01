# AC2: Stable Retained Render Identities

Last updated: 2026-06-19
Status: implemented minimal retained render identity model for Milestone AC issue 2

This document defines Borrowser's browser/runtime-owned retained render
identity model. The model exists so future style, layout, and paint caches have
an explicit identity domain and do not accidentally key retained state by
temporary frame-local IDs.

AC2 does not introduce style, layout, or paint caches. It does not introduce
dirty-state planning, retained paint scenes, display lists, compositor layers,
GPU resources, or dirty-region rendering. AC6 later uses AC2 retained render
identity domains as part of retained layout cache keys without treating
frame-local layout IDs as retained identities.

Related code:

- `crates/browser/src/rendering/identity.rs`
- `crates/browser/src/rendering/lifecycle.rs`
- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/page/mod.rs`
- `crates/browser/src/page/debug.rs`
- `crates/browser/src/rendering/tests/runtime_state.rs`

Related documents:

- `docs/rendering/ac1-retained-render-state-runtime-contract.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/html5/node-identity-contract.md`
- `docs/html5/ae1-html-parser-dom-ownership-contract.md`

## Purpose

Before Borrowser can safely retain render artifacts across frames, it needs an
identity model for runtime continuity. That identity model must be separate
from:

- DOM identity;
- traversal or source-order indices;
- layout `BoxId` values;
- paint operation order or indices;
- paint `StackingContextId` values.

The current model is intentionally minimal:

```text
PageState
  -> RetainedRenderState
  -> RetainedRenderIdentityMap
  -> RetainedRenderIdentityDomain
  -> RetainedRenderId anchored to live DOM provenance
```

The browser/runtime owns allocation and reconciliation. CSS, layout, and paint
may consume retained identities in future issues, but they do not allocate or
own retained render IDs.

## Identity Domains

### DOM Identity

DOM identity is represented by `html::internal::Id`.

Within a live document, DOM IDs identify DOM nodes and may be used as
provenance anchors for the minimal AC2 retained identity map. DOM IDs are not
retained render IDs.

Parser-created DOM identity and materialized DOM identity are also not retained
render identity. HTML parser `PatchKey` values belong to the parser output
protocol. The browser `DomStore` may materialize live patch keys as
`html::internal::Id` values, but that materialization bridge remains separate
from `RetainedRenderId` allocation and reconciliation.

DOM identity by itself does not prove render-artifact continuity across full
document replacement or reparse. A newly parsed document may assign the same
numeric DOM IDs as a prior document, but those IDs live in a different retained
render identity domain.

### Retained Render Identity

Retained render identity is represented by `RetainedRenderId`.

`RetainedRenderId` is a browser/runtime-owned identity for render artifacts
that may survive across frame updates. It is typed and separate from DOM,
layout, paint, stacking, and traversal identity domains.

The current representable retained artifact kind is:

- `DomBackedRenderNode`

Its current provenance anchor is:

- `RetainedRenderAnchor::DomNode(html::internal::Id)`

The anchor locates the current live DOM-backed artifact. It is not the retained
identity itself.

### Retained Render Identity Domain

`RetainedRenderIdentityDomain` isolates retained render IDs across full
document replacement.

`PageState::replace_dom(...)` is treated as a full document identity boundary
in the current runtime path. It resets retained render IDs for the new document
and advances the retained render identity domain. This prevents stale retained
state from being preserved merely because newly parsed DOM nodes receive the
same numeric DOM IDs as nodes in the prior document.

`PageState::start_nav(...)` resets the identity domain to the initial empty
state before a new document is installed.

### Frame-Local Layout Identity

Layout `BoxId` values are frame-local layout-owned IDs. They are deterministic
for a fixed box tree generation, but they are not retained render identities
and must not be used as retained cache keys.

### Frame-Local Paint Identity

Paint operation order and paint operation indices are frame-local paint-owned
debug/ordering concepts. They are not retained render identities and must not
be used as retained cache keys.

### Frame-Local Stacking Identity

`StackingContextId` is a paint-owned frame-local identity assigned while
building the current `StackingContextTree`. It is not a retained render
identity, compositor layer ID, retained scene ID, or backend resource handle.

### Frame-Local Traversal And Source Order

Traversal and source-order indices describe one generated or serialized view of
the current frame/document. They are not retained render identities and must
not be used as retained cache keys.

## Reconciliation Rules

For same-document DOM mutation, browser/runtime reconciles retained identities
against the current live DOM:

1. collect currently live DOM-backed render anchors;
2. prune retained identities whose anchors are no longer live;
3. allocate new retained IDs for newly observed live anchors.

Pruning happens before allocation. Retained IDs are not recycled within the
active document identity lifetime.

For full document replacement through `PageState::replace_dom(...)`:

1. advance the retained render identity domain;
2. clear the prior identity map;
3. reset ID allocation for the new domain;
4. allocate identities from the new live document.

This means equivalent fresh input produces deterministic retained identity
debug output in each fresh document, without implying cross-document identity
continuity.

## Debug Surface

`PageState::retained_render_state_debug_snapshot()` reports retained identities
through `RetainedRenderStateDebugSnapshot`.

The stable string form includes:

```text
retained-identities:
  identity-domain: 1
  render-artifacts: 5
    - retained-render-id=4 kind=dom-backed-render-node anchor=dom-node(4)
  frame-local-layout-ids: not-retained
  frame-local-paint-ids: not-retained
  frame-local-stacking-ids: not-retained
  frame-local-traversal-source-order-ids: not-retained
```

`anchor=dom-node(...)` is provenance only. It is not a retained ID and is not a
continuity proof across document replacement.

The debug surface is deterministic and internal to regression tests. It is not
a public API.

## Invariants

For a fixed sequence of same-document mutations and full document replacements:

- browser/runtime owns retained render identity allocation;
- retained render IDs are typed and separate from DOM IDs;
- DOM IDs are anchors/provenance only;
- full document replacement starts a new retained render identity domain;
- removed anchors are pruned before new IDs are allocated;
- retained IDs are not recycled within the active document identity lifetime;
- equivalent fresh input produces deterministic identity debug output;
- layout `BoxId` values remain frame-local;
- paint operation order/indices remain frame-local;
- `StackingContextId` values remain frame-local;
- traversal/source-order indices remain frame-local;
- no cache or artifact reuse behavior is implied by identity existence.

## Future Extension Points

Future retained rendering issues may extend this model by adding:

- generated or anonymous layout artifact anchors;
- explicit style cache keys that consume retained render identities;
- explicit layout cache keys that consume retained render identities;
- explicit paint cache keys that consume retained render identities;
- mutation-driven identity tracking beyond the current live-DOM reconciliation;
- conservative fallback rules when continuity cannot be proven.

Those extensions must preserve browser/runtime ownership of retained identity
allocation and must not reinterpret frame-local layout, paint, stacking, or
traversal IDs as retained identities.
