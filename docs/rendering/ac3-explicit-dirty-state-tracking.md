# AC3: Explicit Dirty-State Tracking

Last updated: 2026-06-19
Status: implemented explicit retained dirty-state tracking for Milestone AC issue 3

This document defines Borrowser's browser/runtime-owned dirty-state model for
retained rendering. AC3 makes style, layout, and paint dirtiness explicit,
typed, scoped, deterministic, inspectable, and conservatively propagated.

AC3 does not introduce retained layout caches, retained paint caches, retained
paint scenes, display lists, dirty-region rendering, targeted relayout,
compositor layers, GPU layers, or broad CSS property-impact classification.

Related code:

- `crates/browser/src/rendering/types.rs`
- `crates/browser/src/rendering/invalidation.rs`
- `crates/browser/src/rendering/lifecycle.rs`
- `crates/browser/src/rendering/debug.rs`
- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/page/mod.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/rendering/tests/invalidation.rs`
- `crates/browser/src/rendering/tests/runtime_state.rs`

Related documents:

- `docs/rendering/ac1-retained-render-state-runtime-contract.md`
- `docs/rendering/ac2-retained-render-identities.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/ab5-structured-paint-invalidation-model.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`

## Purpose

Before AC3, retained runtime state exposed style and layout dirty placeholders
and AB5 exposed paint-specific invalidation. AC3 introduces the shared retained
dirty vocabulary that future retained work planning and conservative artifact
reuse can consume:

```text
RenderInvalidationEntryPoint
  -> RenderDirtyRequest
  -> DirtyEntry { phase, reason, scope }
  -> RenderDirtyState
  -> deterministic dirty-state debug output
```

The model explains which phase is dirty, why it is dirty, and how broad the
current conservative invalidation scope is. It does not prove that any
artifact was reused or recomputed.

## Ownership

Browser/runtime owns:

- dirty-state lifetime in `RetainedRenderState`;
- dirty request derivation from runtime invalidation entry points;
- deterministic dirty propagation between style, layout, and paint;
- conservative dirty-scope merging;
- dirty-state debug output.

CSS owns:

- authored CSS parsing;
- selector matching;
- cascade and computed-style semantics;
- property metadata and any future property-impact classification.

Layout owns:

- box-tree generation;
- layout geometry;
- layout-specific dependency knowledge.

Paint owns:

- paint ordering;
- stacking-context semantics;
- paint primitives and paint artifact semantics.

Browser/runtime must not duplicate CSS property semantics with an ad hoc
property-impact table. If CSS does not expose a safe distinction between
paint-only and layout-affecting style changes, runtime dirty propagation uses
an explicit conservative fallback.

## Dirty Vocabulary

AC3 introduces these runtime types:

- `DirtyPhase`: `Style`, `Layout`, `Paint`
- `DirtyReason`: typed source or propagation reason
- `DirtyScope`: `None`, retained-node/artifact/subtree forms, `Viewport`,
  and `Document`
- `DirtyEntry`: one `{ phase, reason, scope }` record
- `RenderDirtyRequest`: dirty entries derived from one runtime invalidation
  entry point
- `RenderDirtyState`: deterministic aggregate dirty state
- `DirtyPropagationResult`: direct and propagated dirty entries for tests and
  future planning
- `DirtyStateDebugSnapshot`: stable debug representation of retained dirty
  state

The retained-ID dirty scopes are part of the typed vocabulary, but AC3 does
not emit node, artifact, or subtree scopes unless the runtime has safe retained
dependency data. Current production dirty requests use `Document` and
`Viewport` scopes.

## Propagation Rules

For current runtime entry points:

| entry point | direct dirty entries | propagated entries |
| --- | --- | --- |
| `DocumentReplaced` | style/document, reason `DocumentReplaced` | layout/document from style, paint/document from layout |
| `DomStructureChanged` | style/document, reason `DomContentChanged` | layout/document from style, paint/document from layout |
| `DomAttributesChanged` | style/document, reason `StyleInputChanged` | layout/document from style, paint/document from layout |
| `DomTextChanged` | layout/document, reason `TextContentChanged` | paint/document from layout |
| `StylesheetSetChanged` | style/document, reason `StylesheetChanged` | layout/document from style, paint/document from layout |
| `ViewportChanged` | layout/viewport, reason `ViewportChanged` | paint/viewport from layout |
| `ResourceStateChanged` | layout/document and paint/document, reason `ResourceStateChanged` | none |
| `InputStateChanged` | paint/viewport, reason `RuntimeInputState` | none |

Pure text mutation does not restyle in the current supported CSS model. Text
content affects layout and paint; stylesheet text changes are handled by
stylesheet reconciliation and dirty style separately. Future generated content
or text-sensitive selector support must widen this rule or add CSS-owned
dependency classification.

Viewport changes do not imply style dirtiness by default.

Input-state changes are currently a safely representable paint-only runtime
invalidation. AC3 does not classify arbitrary CSS style changes as paint-only.

Unknown impact falls back to document-scoped style, layout, and paint
dirtiness with `ConservativeUnknownImpact`.

## Merge And Ordering

`RenderDirtyState` stores dirty entries in a deterministic vector. Inserts are
deduplicated by phase and reason, and merged by conservative scope.

Scope merging is conservative:

- `Document` wins over all other scopes.
- `Viewport` remains viewport-scoped only when merged with viewport or none.
- conflicting retained-node/artifact/subtree scopes widen to `Document`.
- `None` does not create a dirty entry.

Debug output uses stable phase, reason, and scope ordering. It does not depend
on hash-map or hash-set iteration.

## Debug Surface

`PageState::retained_render_state_debug_snapshot()` includes
`DirtyStateDebugSnapshot`.

The stable string form contains:

```text
dirty-state:
  entries: 3
    entry[0]: phase=style reason=document-replaced scope=document
    entry[1]: phase=layout reason=cascaded-from-style scope=document
    entry[2]: phase=paint reason=cascaded-from-layout scope=document
  style-dirty: true
  layout-dirty: true
  paint-dirty: true
```

The boolean phase fields are convenience summaries derived from typed dirty
entries. The typed entries are the normative retained dirty-state contract.

## Relationship To AB5 Paint Invalidation

AB5 remains the paint-specific invalidation and repaint planning contract.
AC3 does not replace it. AC3 exposes paint dirtiness as part of the shared
style/layout/paint retained dirty model, while AB5 continues to define paint
invalidation reasons, paint scopes, pending paint invalidation merging, and
repaint execution scope.

The two models derive from the same runtime invalidation entry points and must
remain consistent.

## Invariants

For a fixed sequence of runtime invalidations:

- dirty-state creation is deterministic;
- dirty entries are typed, not string metadata;
- every dirty entry names a phase, reason, and scope;
- style dirtiness conservatively propagates to layout and paint unless a safe
  CSS-owned classification narrows it in a future issue;
- layout dirtiness propagates to paint when geometry or visual output may
  change;
- viewport changes do not imply restyle by default;
- pure text mutations dirty layout and paint, not style, in the current model;
- paint-only runtime invalidation can avoid layout only when safely
  classifiable from existing runtime state;
- conservative full-document fallback is explicit and visible;
- retained render IDs are not confused with DOM IDs, layout `BoxId`, paint
  operation indices, stacking IDs, or traversal/source-order IDs.

## Deliberate Exclusions

AC3 deliberately excludes:

- retained layout caches;
- retained paint caches;
- retained display lists or scenes;
- dirty-region rendering;
- targeted relayout;
- compositor layers or GPU concepts;
- backend partial-raster behavior;
- broad browser-owned CSS property-impact classification;
- dependency graphs from DOM/style/layout nodes to retained paint artifacts;
- use of frame-local layout, paint, stacking, or traversal IDs as dirty keys;
- deterministic render work planning beyond dirty-state derivation.

AC6 adds retained layout artifact reuse and explicit relayout execution
fallbacks on top of this dirty-state vocabulary. True minimal/subtree relayout
remains future work until Layout can execute those scopes safely.
