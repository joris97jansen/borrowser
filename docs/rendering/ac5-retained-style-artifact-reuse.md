# AC5: Retained Style Artifact Reuse

Status: implemented conservative retained style artifact reuse for Milestone AC
issue 5

This document defines Borrowser's first retained style artifact reuse path.
AC5 makes browser/runtime-owned style artifact lifetime, cache keys,
reuse/recompute/discard accounting, and deterministic debug output explicit.

AC5 does not introduce selector dependency tracking, style sharing beyond the
existing CSS-owned computed-style paths, retained paint caches, retained
display lists, retained scenes, compositor layers, GPU concepts, or
browser-owned CSS property impact tables. AC6 later adds retained layout
artifact reuse and CSS-owned computed-style layout-impact classification.

Related code:

- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/page/style_cache.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/page/restyle.rs`
- `crates/browser/src/page/stylesheets.rs`
- `crates/browser/src/rendering/lifecycle.rs`
- `crates/browser/src/rendering/work_plan.rs`
- `crates/browser/src/rendering/tests/runtime_state.rs`
- `crates/browser/src/tab/tests/style_cache.rs`

Related documents:

- `docs/rendering/ac1-retained-render-state-runtime-contract.md`
- `docs/rendering/ac2-retained-render-identities.md`
- `docs/rendering/ac3-explicit-dirty-state-tracking.md`
- `docs/rendering/ac4-deterministic-render-work-plans.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`

## Retained Style Artifacts

The only retained style artifacts introduced by AC5 are:

- `ResolvedDocumentStyle`
- `ComputedDocumentStyle`

Browser/runtime stores these artifacts in `PageState` through
`RetainedRenderState` and `PageStyleCache`. CSS remains the semantic owner of
the artifacts. The browser/runtime owns only their retained lifetime,
validity key, invalidation state, and debug accounting.

The following are deliberately not retained style artifacts:

- `StyledNode`
- `StylePhaseOutput`
- layout boxes or layout IDs
- paint commands or paint operation IDs
- stacking context IDs
- traversal or source-order IDs
- display lists, retained scenes, compositor layers, or GPU resources

`StyledNode` remains a borrow-backed view rebuilt from the current DOM and the
retained computed style artifact. Rebuilding that view is not counted as
retained style artifact reuse.

## Cache Key

Retained style artifacts are keyed by:

```text
RetainedStyleArtifactKey {
  identity_domain: RetainedRenderIdentityDomain,
  style_input_generation: u64,
  stylesheet_generation: u64,
}
```

The key represents the current conservative validity boundary:

- `identity_domain` prevents reuse across full document replacement, even when
  the new document contains matching numeric DOM IDs;
- `style_input_generation` changes when DOM/style inputs can affect selector
  matching, inline style attributes, inheritance, or element order;
- `stylesheet_generation` changes when the active stylesheet set or loaded
  stylesheet contribution changes.

`RenderEpoch` is not a retained style cache key. It remains the broader
browser/runtime retained-state generation described by AC1 and is not cache-hit
proof.

## Reuse And Invalidation

Browser/runtime may reuse retained style artifacts only when:

```text
style is not dirty
and PageStyleCache.key == current RetainedStyleArtifactKey
```

No-op updates reuse retained `ResolvedDocumentStyle` and
`ComputedDocumentStyle`. The runtime still rebuilds the borrow-backed
`StyledNode` view for the current frame.

Viewport-only updates do not dirty style by default for the currently
supported CSS feature set. If future CSS features add viewport-dependent style
semantics, such as media queries, viewport units, container queries, font or
device state, or similar environment inputs, CSS-owned dependency facts or a
style-environment generation must participate in the retained style key.

Stylesheet changes conservatively invalidate style and discard retained style
artifacts.

Document replacement and structural DOM changes conservatively invalidate style
with a full scope. Full document replacement also advances the retained
identity domain so matching numeric DOM IDs cannot prove retained style
continuity.

Class, attribute, and inline style changes use the currently supported
`AttributeSuffix` scope only when the runtime receives materialized dirty node
IDs from the DOM mutation path. This is a conservative current-contract scope,
not selector-aware dependency tracking. If the smaller scope is unavailable or
proof fails, the runtime falls back to full style recompute and exposes the
fallback in debug output.

Text-only DOM mutation does not dirty style in the current supported selector
and property model. It dirties layout and paint. Future text-sensitive selector
or generated-content support must widen this rule or introduce CSS-owned
dependency classification.

## Debug Surface

`RetainedRenderStateDebugSnapshot` includes a `style-artifacts` block:

```text
style-artifacts:
  key: identity-domain=1 style-input-generation=1 stylesheet-generation=1
  state: retained-fresh
  last-action: reused
  reuse-count: 1
  recompute-count: 1
  discard-count: 0
```

The counters track retained style artifact lifecycle decisions:

- `reuse-count`: retained resolved/computed artifacts were reused;
- `recompute-count`: retained resolved/computed artifacts were recomputed;
- `discard-count`: retained resolved/computed artifacts were discarded due to
  full invalidation.

`last-action` makes the most recent lifecycle decision visible:

- `none`
- `initial-compute`
- `reused`
- `full-recompute`
- `incremental-suffix-recompute`
- `discarded-for-full-invalidation`
- `fallback-full-recompute`

The debug surface is deterministic and internal to regression tests. It is not
a public API.

## Ownership Invariants

Browser/runtime owns:

- retained style artifact lifetime;
- retained style artifact key construction;
- reuse/recompute/discard accounting;
- conservative invalidation state;
- deterministic debug reporting.

CSS owns:

- CSS parsing;
- selector matching;
- cascade behavior;
- inheritance and defaulting;
- computed values and property meaning;
- style-tree construction;
- any future selector/property/environment dependency facts.

Browser/runtime must not inspect selectors, infer selector dependencies,
classify CSS properties by impact, or duplicate CSS property metadata.

## Deliberate Exclusions

AC5 deliberately excludes:

- fake selector dependency tracking;
- browser-owned CSS property impact tables;
- retained paint commands, display lists, or scenes;
- dirty-region rendering;
- compositor or GPU concepts;
- frame-local layout, paint, stacking, traversal, or source-order IDs as
  retained keys;
- treating `StyledNode` rebuilding as retained style artifact reuse;
- performance/allocation guardrails beyond deterministic lifecycle tests.
