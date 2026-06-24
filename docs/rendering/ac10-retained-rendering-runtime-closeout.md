# AC10: Retained Rendering Runtime Closeout

Last updated: 2026-06-24
Status: implemented documentation closeout for Milestone AC issue 10

This document closes Milestone AC by consolidating Borrowser's retained
rendering runtime contracts into one browser-engine subsystem reference.

AC10 does not introduce new runtime behavior, Rust API surfaces, data
structures, tests, compositor concepts, dirty-region rendering, selector
dependency invalidation, CSS containment, HTML parser conformance,
JavaScript/event-loop-driven mutation invalidation, or broad WPT integration.
It documents what AC1 through AC9 already implemented, where the ownership
boundaries live, which conservative fallbacks remain intentional, and which
future milestones can safely extend the retained rendering foundation.

Related code:

- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/rendering/lifecycle.rs`
- `crates/browser/src/rendering/identity.rs`
- `crates/browser/src/rendering/invalidation.rs`
- `crates/browser/src/rendering/work_plan.rs`
- `crates/browser/src/rendering/debug.rs`
- `crates/browser/src/rendering/frame.rs`
- `crates/browser/src/rendering/contracts.rs`
- `crates/browser/src/page/style_cache.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/page/restyle.rs`
- `crates/layout/src/retained.rs`
- `crates/gfx/src/viewport.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/css/src/computed/document/reuse.rs`

Related documents:

- `docs/rendering/ac1-retained-render-state-runtime-contract.md`
- `docs/rendering/ac2-retained-render-identities.md`
- `docs/rendering/ac3-explicit-dirty-state-tracking.md`
- `docs/rendering/ac4-deterministic-render-work-plans.md`
- `docs/rendering/ac5-retained-style-artifact-reuse.md`
- `docs/rendering/ac6-retained-layout-artifact-foundation.md`
- `docs/rendering/ac7-retained-paint-artifact-reuse-repaint-planning.md`
- `docs/rendering/ac8-incremental-rendering-debug-snapshots.md`
- `docs/rendering/ac9-incremental-rendering-performance-guardrails.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/rendering/ab5-structured-paint-invalidation-model.md`
- `docs/rendering/ab8-stacking-compositing-invalidation-closeout.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`
- `docs/engine-feature-gap-tracker.md`

## Purpose And Scope

Milestone AC converts Borrowser's rendering pipeline into a
browser/runtime-owned retained rendering foundation:

```text
PageState
  -> RetainedRenderState
  -> retained render identities
  -> retained dirty state
  -> deterministic RenderWorkPlan
  -> conservative style/layout/paint artifact reuse
  -> deterministic internal debug and guardrail surfaces
```

The retained rendering runtime answers, for each render update:

- what retained state exists;
- which subsystem owns that state's lifetime and semantics;
- which style, layout, and paint work is dirty;
- why it is dirty;
- which work the runtime plans before execution;
- which retained artifacts were reused, recomputed, discarded, or rejected;
- which conservative fallbacks were selected;
- why the result is deterministic.

The scope is a retained-runtime foundation. It is not a compositor, not a
dirty-region renderer, not a selector dependency engine, not a CSS containment
implementation, not a full HTML parser conformance layer, not a JavaScript or
event-loop invalidation system, and not broad WPT integration.

## Subsystem Ownership Matrix

| subsystem | owns | may consume | must not own or infer |
| --- | --- | --- | --- |
| Browser/runtime | retained state lifetime, `RenderEpoch`, retained render identity allocation, dirty-state aggregation, invalidation entry points, work-plan derivation, artifact keys, reuse/recompute/discard counters, frame orchestration, deterministic internal debug summaries | CSS-owned style artifacts and impact facts; Layout-owned retained layout artifacts and materialization results; Paint-owned semantic paint artifacts | CSS parsing, selector matching, cascade semantics, computed value meaning, layout geometry, formatting behavior, paint ordering, stacking semantics, backend/GPU resources |
| CSS | authored CSS parsing, selector matching, cascade, computed style, `ResolvedDocumentStyle`, `ComputedDocumentStyle`, `StyledNode` construction, CSS-owned style impact facts | browser/runtime invalidation context and DOM/style inputs | layout geometry, paint ordering, viewport scheduling, retained artifact lifetime |
| Layout | box-tree construction, formatting behavior, geometry, layout metadata, retained layout artifact construction and materialization | computed style, explicit layout environment inputs, retained layout key inputs from runtime | CSS selector/cascade behavior, paint primitive semantics, compositor ownership, browser-retained identity allocation |
| Paint | semantic paint artifact construction, paint primitives, stacking contexts, paint ordering, semantic layers, paint-owned debug meaning | current-frame layout output, runtime paint inputs, retained paint lifetime decisions made by browser/runtime | layout geometry, CSS property meaning, browser-owned retained keys, retained render ID allocation, backend command caching as a semantic paint contract |

The browser/runtime may retain and invalidate artifacts from CSS, Layout, and
Paint only through explicit contracts. Retention never transfers the semantic
ownership of those artifacts into browser/runtime code.

## Retained Versus Rebuilt State

| artifact or state | semantic owner | retained owner | lifetime in AC | notes |
| --- | --- | --- | --- | --- |
| DOM | Browser/runtime | Browser/runtime | retained across updates | DOM IDs are provenance anchors, not retained render IDs. |
| Stylesheet set | Browser/runtime | Browser/runtime | retained across updates | Style input and stylesheet generations participate in style keys. |
| `ResolvedDocumentStyle` | CSS | Browser/runtime | retained across updates | Reused only through the AC5 style artifact key and clean style dirtiness. |
| `ComputedDocumentStyle` | CSS | Browser/runtime | retained across updates | CSS owns computed value meaning and style-tree construction. |
| `StyledNode` / styled tree view | CSS | none | borrow-backed rebuilt on demand | Rebuilding this view is not retained style artifact reuse. |
| retained render identities | Browser/runtime | Browser/runtime | retained within the active retained identity domain | Separate from DOM IDs and all frame-local IDs. |
| retained dirty state | Browser/runtime | Browser/runtime | retained until consumed and cleared by recorded frame results | Stores typed phase, reason, and scope entries. |
| `RenderWorkPlan` | Browser/runtime | none | derived before frame execution | Planning output, not retained cache state and not proof of reuse. |
| `RetainedLayoutArtifact` | Layout | Browser/runtime | retained across updates | Layout owns geometry and materialization semantics. Runtime owns lifetime and keys. |
| `LayoutPhaseOutput` | Layout | none | current-frame materialized output | May be produced by relayout or materialized from a retained layout artifact. |
| paint semantic artifact | Paint | Browser/runtime | retained across updates | Paint owns semantic contents. Runtime owns key, lifetime, and counters. |
| `PaintPhaseInput` | Paint/runtime handoff | none | current-frame handoff | Rebuilt as a typed handoff. |
| backend paint commands | Paint/renderer backend | none | immediate frame output | AC does not retain backend commands, GPU resources, or compositor layers. |
| resource and input state | Browser/runtime | Browser/runtime | retained runtime input | Retained outside page-local render caches. |

## Retained State Lifetime

| state | created or reset | advanced or invalidated by | reused when | explicit non-meaning |
| --- | --- | --- | --- | --- |
| `RenderEpoch` | `PageState::new()` and navigation reset start at `0` | page-owned retained render-state changes | no-op updates that only materialize from already-retained artifacts preserve it | not a frame count, pass count, cache-hit proof, or layout/paint identity |
| retained identity domain | new page state or navigation reset | full document replacement starts a new domain | same-document reconciliation can keep live anchors | not cross-document continuity proof |
| style generations | page style lifecycle | DOM/style inputs and stylesheet set changes | keys match and style is clean | not CSS semantic ownership |
| layout generations | retained layout input lifecycle | layout-affecting DOM/style/resource/viewport inputs | key matches and layout is clean | not a proof of subtree relayout support |
| paint generations | retained paint input lifecycle | paint-relevant style, input, resource, or layout changes | key matches and paint is clean | not dirty-region or compositor invalidation |
| retained dirty entries | page/runtime invalidation | explicit invalidation entry points and propagation | cleared only through successful recorded artifact results | not a dependency graph |

Navigation and full document replacement are hard retained-state boundaries.
Matching numeric DOM IDs in a newly parsed document do not prove retained
render identity, style, layout, or paint continuity.

## Retained Identity Invariants

| identity domain | current AC meaning | retained? | allowed as retained cache key? |
| --- | --- | --- | --- |
| `RetainedRenderId` | browser/runtime-owned retained render identity for representable DOM-backed render artifacts | yes, within one retained identity domain | yes, through explicit retained identity contracts |
| DOM `html::internal::Id` | live DOM node provenance anchor | no | no, by itself |
| layout `BoxId` | frame-local layout-owned box identity | no | no |
| paint operation index/order | frame-local paint debug/order concept | no | no |
| `StackingContextId` | frame-local paint-owned stacking identity | no | no |
| traversal or source-order index | one current serialized or generated ordering | no | no |
| compositor/GPU/backend resource handle | not introduced by AC | no | no |

Retained render IDs are allocated and reconciled by browser/runtime. Removed
anchors are pruned before allocation, retained IDs are not recycled within the
active document identity lifetime, and full document replacement starts a new
retained identity domain.

## Dirty-State Propagation

Dirty state is typed as:

```text
DirtyEntry { phase: DirtyPhase, reason: DirtyReason, scope: DirtyScope }
```

| entry point | direct dirtiness | propagated dirtiness | current scope behavior |
| --- | --- | --- | --- |
| `DocumentReplaced` | style/document, reason `DocumentReplaced` | layout from style, paint from layout | document |
| `DomStructureChanged` | style/document, reason `DomContentChanged` | layout from style, paint from layout | document |
| `DomAttributesChanged` | style/document, reason `StyleInputChanged` | layout from style, paint from layout | document unless a narrower supported style invalidation path is safely available |
| `DomTextChanged` | layout/document, reason `TextContentChanged` | paint from layout | document |
| `StylesheetSetChanged` | style/document, reason `StylesheetChanged` | layout from style, paint from layout | document |
| `ViewportChanged` | layout/viewport, reason `ViewportChanged` | paint from layout | viewport |
| `ResourceStateChanged` | layout/document and paint/document, reason `ResourceStateChanged` | none | document |
| `InputStateChanged` | paint/viewport, reason `RuntimeInputState` | none | viewport |
| unknown impact | style, layout, and paint, reason `ConservativeUnknownImpact` | explicit full fallback | document |

`DirtyScope` supports `None`, retained-node/artifact/subtree forms,
`Viewport`, and `Document`. Current production dirty requests use `Document`
and `Viewport` unless safe retained dependency data exists. Conflicting or
unproven narrower scopes widen conservatively to `Document`.

Viewport changes do not imply restyle by default. Text mutation does not dirty
style in the current supported selector/property model. Future generated
content, text-sensitive selectors, viewport-dependent style, container queries,
or media/environment dependencies must extend CSS-owned dependency facts or
widen the dirty rules.

## Deterministic Render Work Plans

`RenderWorkPlan` is derived before style, layout, and paint execute:

```text
retained dirty state
  + pending render work
  + retained artifact states
  -> canonical dirty state
  -> restyle / relayout / repaint decisions
  -> relayout and repaint execution strategies
```

| planned field | examples | meaning | not a proof of |
| --- | --- | --- | --- |
| `restyle` | `ReuseRetainedStyle`, `Restyle`, `ConservativeFallback` | intended style work from dirty state and retained style artifact state | actual style reuse after execution |
| `relayout` | `ReuseRetainedLayout`, `Relayout`, `ConservativeFallback` | intended layout work from dirty state and retained layout artifact state | successful materialization |
| `relayout_execution` | `ReuseRetained`, `FullDocument`, `ConservativeDocumentFallback` | executable layout strategy for the requested scope | true minimal/subtree relayout |
| `repaint` | `ReuseRetainedPaint`, `Repaint`, `ConservativeFallback` | intended paint work from dirty state and retained paint artifact state | retained display-list or dirty-region behavior |
| `repaint_execution` | `ReuseRetained`, `FullDocument`, `Viewport`, conservative fallbacks | executable repaint strategy | partial raster or compositor invalidation |
| `conservative_fallback` | `ConservativeUnknownImpact`, `TargetedRelayoutNotExecutable` | visible reason a narrow or unknown path widened | hidden optimization |

Plans are deterministic and read-only. They do not clear dirty state, mutate
retained artifacts, or prove that reuse occurred. Actual execution outcomes
are recorded by retained artifact lifecycle state and
`RenderFrameExecutionTrace`.

## Artifact Reuse Rules

### Style

| case | retained style outcome | reason |
| --- | --- | --- |
| no-op update, clean style, matching `RetainedStyleArtifactKey` | reuse retained `ResolvedDocumentStyle` and `ComputedDocumentStyle` | style input and stylesheet generations still match |
| viewport-only update in current supported CSS model | reuse style | viewport changes do not dirty style by default |
| stylesheet set change | discard/recompute | stylesheet generation changes and style dirtiness is document-scoped |
| document replacement | discard/recompute in a new identity domain | retained continuity cannot cross full document replacement |
| text-only mutation | style remains clean | current selector/property model does not make text content style-relevant |
| unknown style impact | conservative recompute | CSS has not supplied a safe narrower fact |

Browser/runtime owns retained style artifact lifetime and keys. CSS owns
parsing, selector matching, cascade, computed values, and any future selector
or property dependency facts.

### Layout

| case | retained layout outcome | reason |
| --- | --- | --- |
| no-op update with matching `RetainedLayoutKey` and clean layout dirtiness | reuse retained layout artifact | retained layout key and dirty state permit materialization |
| paint-only style update with CSS-owned paint-only impact | reuse retained layout if key still matches | layout-affecting style generation does not change |
| viewport width change | recompute or conservative document fallback | viewport width is part of the layout key |
| text, structure, resource, layout-affecting style, or unknown style impact | recompute or fallback | layout input or layout style generation changes, or layout dirtiness is present |
| retained layout materialization failure | conservative document relayout | current-frame references cannot be safely reattached |

Layout owns geometry, formatting, retained layout artifact construction, and
materialization. Browser/runtime owns retained layout lifetime, keys, counters,
and fallback reporting.

### Paint

| case | retained paint outcome | reason |
| --- | --- | --- |
| no-op update with matching `RetainedPaintArtifactKey`, clean paint dirtiness, and retained layout reuse | reuse retained paint artifact | paint inputs, layout key, and paint state still match |
| paint-only style update | recompute paint without relayout when CSS-owned impact classification permits | paint style generation changes while layout can remain valid |
| layout dirty or retained layout reuse fails | recompute paint | paint consumes current layout output |
| input state change | repaint viewport | runtime input state affects paint |
| resource state change | conservative document layout/paint work | replaced metadata or paint resources may affect output |
| key mismatch, missing artifact, or dirty paint state | recompute or fallback | retained paint continuity is not proven |

The retained paint artifact is paint-owned semantic data wrapped by a
browser/runtime-owned retained entry. It is not a backend command cache,
display-list architecture, retained scene graph, compositor layer, GPU
resource, dirty-region graph, or partial raster invalidation system.

## Conservative Fallback Behavior

| fallback | selected when | visible where | required interpretation |
| --- | --- | --- | --- |
| `ConservativeUnknownImpact` | dirty input cannot be safely classified | work-plan fallback and dirty-state debug output | document-scoped style/layout/paint work, not an optimization |
| full style recompute fallback | style continuity cannot be proven | retained style artifact action/debug counters | CSS semantics remain CSS-owned |
| `TargetedRelayoutNotExecutable` | requested relayout scope is narrower than what Layout can safely execute | `RelayoutExecution::ConservativeDocumentFallback` | actual document relayout, not subtree relayout |
| retained layout materialization failure | retained artifact cannot attach current-frame references | retained layout artifact action/debug counters | recompute layout rather than using stale geometry |
| conservative document repaint | paint input is document-scoped or paint reuse cannot be proven | repaint execution and retained paint artifact action | full document repaint semantics |
| conservative viewport repaint | paint input is viewport-scoped and executable as viewport repaint | repaint execution and retained paint artifact action | viewport-scoped repaint, not dirty rectangles |
| missing or key-mismatched retained artifact | no valid retained artifact exists for the current key | artifact state and action summaries | recompute, not implicit cache creation |

Fallbacks are part of the contract. They must stay explicit in debug output
and documentation so later milestones can narrow them deliberately.

## Debug Snapshot Surfaces

These surfaces are deterministic internal debug/regression contracts. They are
not public stable APIs.

| surface | owner | documents | key AC source |
| --- | --- | --- | --- |
| `PageState::retained_render_state_debug_snapshot()` | Browser/runtime | retained epoch, artifact states, dirty entries, style invalidation, generations, retained identities, style/layout/paint artifact counters and actions | AC1, AC2, AC3, AC5, AC6, AC7, AC8 |
| `RenderWorkPlan::to_debug_snapshot()` | Browser/runtime | canonical dirty state, entry points, restyle/relayout/repaint decisions, execution strategies, fallback reasons | AC4, AC6, AC7, AC8 |
| `RenderFrameExecutionTrace::to_debug_snapshot()` | Browser/runtime | executed frame requests, materialized-from-retained artifacts, repaint execution scope, phase order | AC8, AC9 |
| `render_phase_boundary_debug_snapshot(...)` | Browser/runtime | style/layout/paint/orchestration phase boundaries | V6, V7 |
| paint invalidation debug snapshot | Browser/runtime/Paint contract boundary | paint invalidation reasons and scopes derived from runtime entry points | AB5, AB8 |
| layout and paint owned snapshots | Layout/Paint | geometry, formatting, stacking, ordering, operation semantics | W/X/Y/Z/AA/AB milestone contracts |

Debug snapshots observe state and execution. They must not drive rendering
behavior, and retained-state snapshots must not present frame-local layout,
paint, stacking, traversal, source-order, backend, or memory identities as
retained identities.

## Regression Test Surfaces

| file | validates | AC role |
| --- | --- | --- |
| `crates/browser/src/rendering/tests/contracts.rs` | rendering contract tables and ownership alignment | keeps architecture tables executable |
| `crates/browser/src/rendering/tests/runtime_state.rs` | retained state, epoch, identity, artifact lifecycle, debug summaries | AC1, AC2, AC5, AC6, AC7, AC8 |
| `crates/browser/src/rendering/tests/invalidation.rs` | invalidation entry points, dirty requests, propagation, scopes | AC3 and AB5 alignment |
| `crates/browser/src/rendering/tests/work_plan.rs` | deterministic work-plan decisions and fallback reasons | AC4, AC6, AC7 |
| `crates/browser/src/rendering/tests/frame_trace.rs` | executed frame trace distinctions | AC8 |
| `crates/browser/src/rendering/tests/phase_boundaries.rs` | deterministic phase boundary snapshots | V6/V7 foundation consumed by AC |
| `crates/browser/src/rendering/tests/perf_guards.rs` | retained behavior guardrails for representative update scenarios | AC9 |
| `crates/browser/src/tab/tests/style_cache.rs` | retained style cache behavior through tab/page paths | AC5 integration |

AC10 adds no new test surface because it is a documentation/contracts closeout.
Behavioral changes to retained state, planning, reuse, or debug output must add
or update focused tests in the relevant existing surface.

## Benchmark And Performance Guardrails

AC9 provides CI-safe performance and resource guardrails through deterministic
counters and state assertions instead of wall-clock thresholds.

| guardrail scenario | expected proof shape |
| --- | --- |
| initial render | retained style/layout/paint recompute counters establish a baseline |
| no-op repeated render | style/layout/paint recompute counters do not grow and retained reuse is visible |
| repeated viewport resize | style recomputation does not grow by default; layout/paint work remains bounded by update count |
| text/content update | style remains clean in the current model; layout/paint work is bounded |
| paint-only style update | relayout is avoided when CSS-owned impact classification says paint-only |
| layout-affecting style update | layout and paint recompute visibly |
| stylesheet/global style update | style and downstream work recompute conservatively |
| representative in-repo page update | retained identities and dirty entries do not grow without bound |

Heap-byte allocation assertions are not part of the AC default browser
rendering proof. Future allocation guardrails should be isolated, deterministic
and crate-local rather than a broad wall-clock or global allocator shortcut.

## Known Limitations And Non-Goals

| limitation or non-goal | current AC status | future direction |
| --- | --- | --- |
| compositor or GPU layers | not implemented | requires explicit compositor/layer ownership contract |
| full dirty-region rendering | not implemented | requires region/dependency model, not just repaint scopes |
| partial raster invalidation | not implemented | requires backend-aware raster invalidation contract |
| retained backend draw commands | not implemented | requires paint/backend command ownership boundary |
| full display-list or retained scene architecture | not implemented | may extend paint-owned semantic artifacts deliberately |
| full selector dependency invalidation | not implemented | CSS must own dependency facts before runtime can consume them |
| broad browser-owned CSS property-impact table | deliberately excluded | CSS-owned impact classification only |
| CSS containment | not implemented | requires CSS/layout containment contracts |
| true minimal/subtree relayout execution | not implemented | Layout needs dependency graphs and executable subtree relayout boundaries |
| layout dependency graphs | not implemented | future Layout-owned model |
| paint dependency graphs | not implemented | future Paint-owned model |
| full HTML parser conformance | not part of AC | belongs to HTML/parser milestones |
| JavaScript and event loop | not implemented by AC | future runtime, DOM, and event-loop invalidation contracts |
| DOM mutation APIs beyond current runtime paths | incomplete | future DOM/runtime milestones |
| broad WPT integration | not introduced by AC | future test/tooling milestones |
| debug snapshots as public APIs | deliberately excluded | snapshots remain internal deterministic regression/debug contracts |

Completing AC means Borrowser has an explicit retained rendering runtime
foundation. It does not mean the renderer is complete.

## Future Extension Points

| future milestone area | likely owner | must build on | must preserve |
| --- | --- | --- | --- |
| selector dependency invalidation | CSS with Browser/runtime consumption | AC3 dirty state, AC4 work plans, AC5 style keys | CSS ownership of selector/cascade semantics |
| viewport/environment style dependencies | CSS | AC5 style key and generation model | no browser-owned CSS property semantics |
| CSS containment | CSS and Layout | dirty scopes, layout boundaries, retained identity model | explicit containment semantics before narrower invalidation |
| true subtree relayout | Layout with Browser/runtime orchestration | AC6 requested-scope and fallback model | Layout ownership of geometry and executable relayout rules |
| layout dependency graphs | Layout | retained layout artifact keys and materialization | no frame-local `BoxId` as retained identity |
| paint dependency graphs | Paint with Browser/runtime orchestration | AC7 repaint planning and AB5 paint invalidation | Paint ownership of stacking, ordering, primitives |
| dirty-region rendering | Paint/renderer with Browser/runtime orchestration | repaint scopes, paint dependency facts, retained paint artifacts | no fake dirty rectangles without region proof |
| retained display lists or scene graphs | Paint | AC7 semantic paint artifact ownership | no browser-owned paint ordering interpretation |
| compositor/GPU layers | GFX/renderer plus Paint/Browser contracts | paint scene/display-list foundations | compositor IDs must be distinct from retained render IDs |
| JavaScript/event-loop mutation invalidation | Browser/runtime, DOM, JS, CSS/Layout/Paint consumers | explicit invalidation entry points and dirty propagation | deterministic invalidation and ownership boundaries |
| HTML parser conformance expansion | HTML/parser | DOM identity and runtime replacement boundaries | parser ownership of tree construction semantics |
| WPT integration | Tests/tooling | deterministic engine contracts and fixture harnesses | WPT results must not replace focused engine regression tests |
| allocation guardrails | Tests/tooling and Browser/runtime | AC9 counter guardrails | deterministic, bounded, CI-safe measurements |

Future work should narrow conservative fallbacks only when the owning subsystem
can prove the narrower behavior with typed data and deterministic regression
coverage.

## Milestone AC Completion Rule

Milestone AC is complete at the retained rendering runtime foundation scope
while these conditions hold:

- Browser/runtime explicitly owns retained rendering state across frames.
- Retained render identities are distinct from DOM and frame-local IDs.
- Style, layout, and paint dirtiness are typed, scoped, propagated, and
  deterministic.
- Render work is planned before execution through `RenderWorkPlan`.
- Style, layout, and paint artifacts are reused only through documented keys,
  dirty-state checks, ownership boundaries, and conservative fallback rules.
- Debug snapshots expose retained state, dirty state, planned work, execution
  traces, artifact lifecycle, reuse/recompute/discard counters, and fallback
  reasons as internal regression/debug contracts.
- Performance guardrails prove representative incremental behavior through
  deterministic counters and retained-state cleanup checks.
- Known limitations remain visible in the engine feature gap tracker and are
  not presented as completed AC behavior.

Later milestones must extend these contracts deliberately rather than relying
on accidental object lifetime, eager rebuild behavior, or ad hoc special cases.
