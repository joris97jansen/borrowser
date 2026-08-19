# U8: Runtime Integration Contracts And Future Extension Points

Last updated: 2026-08-15
Status: Milestone U close-out contract

This document records the implemented runtime integration contract for the
rebuilt CSS engine after Milestone U. It supersedes the "implementation target"
parts of U1 with the current page/runtime behavior, while preserving U1's
ownership boundaries.

Related code:

- `crates/browser/src/page.rs`
- `crates/browser/src/document_style.rs`
- `crates/browser/src/tab/events.rs`
- `crates/browser/src/tab/css.rs`
- `crates/browser/src/tab/discovery.rs`
- `crates/browser/src/view.rs`
- `crates/runtime_css/src/lib.rs`
- `crates/css/src/cascade/integration.rs`
- `crates/css/src/computed/document.rs`
- `crates/css/src/computed/document/incremental.rs`
- `crates/css/src/style_invalidation.rs`
- `crates/css/src/computed/style_tree.rs`
- `crates/css/tests/representative_pages.rs`
- `crates/css/benches/css_bench.rs`

Related documents:

- `docs/css/u1-runtime-integration-architecture-css-pipeline-ownership.md`
- `docs/css/af1-selector-cascade-computed-style-architecture-contract.md`
- `docs/css/af4b-selector-dom-query-contract.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`
- `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
- `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/security/css-hardening.md`
- `docs/architecture/ARCHITECTURE.md`
- `docs/html5/ae1-html-parser-dom-ownership-contract.md`

## Integration Ownership

Milestone U establishes this runtime pipeline:

```text
network/html/css events
  -> Tab event routing and navigation filtering
  -> PageState DOM + DocumentStyleSet ownership
  -> ResolvedDocumentStyle
  -> ComputedDocumentStyle
  -> borrow-backed StyledNode tree
  -> layout/gfx/view consumers
```

Ownership is split as follows:

- `runtime_css` owns stylesheet byte buffering, UTF-8 assembly, abort handling,
  and complete decoded-block event emission.
- `browser::Tab` owns event routing, request/navigation filtering, pending
  stylesheet load state, pending render work, and runtime render-work requests.
- `browser::PageState` owns the active DOM, document stylesheet set,
  style/layout dirty state, style generations, and page-local style cache.
- `browser::DocumentStyleSet` owns document-order stylesheet slots and exposes
  loaded `StylesheetParse` artifacts in cascade order.
- `crates/css` owns parsing semantics, selector matching, cascade, computed
  style materialization, diagnostics, and hardening limits.
- `crates/css` may consume DOM element names, attributes, relationships, and
  stylesheet text exposed by the active DOM. It must not depend on HTML
  tokenizer states, tree-builder insertion modes, parse errors, or
  malformed-markup recovery internals.
- `crates/css` owns the fallible selector-DOM projection and its CSS-local
  element IDs. Browser/runtime transports source DOM IDs only for explicit
  mapping and does not reuse patch or retained-render identity as selector
  identity.
- `layout`, `gfx`, and `view` consume `StyledNode` or `ComputedStyle`; they do
  not parse CSS, inspect cascade winners, or recover from invalid declarations.

`Node::style` remains a legacy compatibility projection only. New runtime,
layout, or paint behavior must use `ComputedStyle` or `StyledNode`.

## Stylesheet Attachment Contract

Stylesheets are attached through `DocumentStyleSet`.

The active document owns an ordered list of stylesheet slots:

```text
StylesheetSlot {
  id: StylesheetSlotId,
  key: Inline(text) | External(resolved_url),
  state: Pending | Loaded(StylesheetParse) | Failed | Aborted,
}
```

The invariants are:

- Cascade order is DOM/source order, not network arrival order.
- Inline `<style>` blocks and external `<link rel="stylesheet">` sheets enter
  the same ordered author stylesheet set.
- External stylesheet slots are registered when discovered in the DOM.
- `CssDecodedBlock` installs into an existing slot by `StylesheetSlotId` plus
  current navigation/request identity.
- URL identity is not slot identity. Duplicate same-URL links are distinct
  cascade participants.
- Pending, failed, and aborted slots preserve document position but do not
  contribute declarations.
- Late decoded CSS for removed, failed, or aborted slots is ignored.
- Inline stylesheet text is exact concatenation of text-node children.
- Repeated DOM snapshots reconcile equivalent inline/external slots rather
  than appending duplicates.
- Slot IDs are checked monotonic identities within a document style set and are
  never silently wrapped.

`DocumentStyleSet::stylesheets()` exposes loaded `StylesheetParse` artifacts in
document order for the active document style set. This is correct for U because
style recomputation is controlled by stylesheet-generation invalidation rather
than by rebuilding stylesheet order at every call site. Future performance work
may replace the backing storage with `Arc<StylesheetParse>` or a slot-view
representation, but it must preserve slot identity and cascade order.

## CSS Runtime Contract

`runtime_css` emits complete stylesheet bodies only.

```text
CssChunk
  -> append bytes to per-slot UTF-8 assembly state
  -> append decoded text to pending stylesheet text
  -> emit nothing

CssDone
  -> finish UTF-8
  -> emit exactly one CssDecodedBlock with complete stylesheet text
  -> emit CssSheetDone

CssAbort
  -> discard pending UTF-8/text state
```

`runtime_css` must not tokenize, parse, match selectors, cascade, compute
styles, cache `ComputedStyle`, or inspect the DOM.

CSS response content-type policy is owned by the browser tab. Currently,
`text/css` is accepted, absent content type is accepted, and non-CSS content
types are ignored and marked failed on completion.

## Page Load Lifecycle

For a navigation:

1. `Tab::start_nav(...)` creates a new request generation and resets
   `PageState`.
2. HTML network events stream to the parse runtime and return DOM snapshots or
   DOM patch events.
   HTML/parser owns tokenizer, tree-builder, parser-created DOM, and
   parse-error semantics; browser/page state consumes the resulting document
   output through the AE1 ownership boundary.
3. `Tab::on_core_event(...)` filters every event by `(tab_id, request_id)`.
   Stale events cannot mutate active page state.
4. DOM snapshots produce neutral document-replacement facts.
5. DOM patch batches are staged and applied atomically by `DomStore`; after
   materialization, Browser resolves live versus historical mutation targets
   and constructs one composable `DomMutationFacts` publication.
6. `PageState::replace_dom(...)` installs the materialized DOM, classifies the
   complete fact set once in CSS, and independently derives intrinsic runtime
   work from the same facts.
7. Head metadata, visible text, form-control state, image discovery, and
   stylesheet discovery are updated from the active DOM.
8. `PageState::reconcile_document_stylesheets()` updates `DocumentStyleSet`.
   Any slot-set or loaded-sheet change increments the stylesheet generation and
   submits `StylesheetSetChanged` to CSS-owned invalidation planning, which is
   currently full invalidation.
9. External stylesheet arrivals call `PageState::apply_css_block(...)`.
   A successful install marks the stylesheet generation dirty. The tab/event
   layer observes the changed page state and requests redraw.
10. `browser::view::content(...)` calls `PageState::build_style_phase_output()`.
11. `PageState` either reuses a valid `ComputedDocumentStyle` cache, performs
    an incremental suffix recompute, or performs full style resolution.
12. `build_style_tree_from_computed_styles(...)` rebuilds a borrow-backed
    `StyledNode<'_>` view from the current DOM and cached computed styles.
13. Layout/gfx consume the resulting `StylePhaseOutput` and downstream typed
    layout/paint handoffs.

The normative runtime style path is:

```text
DOM + DocumentStyleSet::stylesheets()
  -> resolve_document_styles(...)
  -> compute_document_styles_from_resolved_styles_with_reuse_stats(...)
  -> ComputedDocumentStyle cache
  -> build_style_tree_from_computed_styles(...)
```

Compatibility APIs such as `attach_styles(...)`, `compute_style(...)`, and
legacy `build_style_tree(...)` are not the browser runtime path.

The runtime path constructs only an explicit document projection. Nested
documents, ambiguous direct document elements, canonical-name violations,
selector-ID exhaustion, and reported projection capacity/reservation failures
remain typed CSS errors through page style/cache/frame/debug callers. A cache
or incremental resolver may report genuine unavailability only after bounded
selector-DOM preflight has validated the projection input; it must not
translate a projection build error into that state. The no-retained-artifact
incremental branch performs preflight only and does not materialize and discard
a complete selector index before Browser's deterministic full fallback.

## DOM Publication Fact Contract

DOM patch batches produce composable neutral facts; there is no winning
trigger or severity order. Current behavior is:

| fact | examples | style effect | intrinsic effect |
| --- | --- | --- | --- |
| `DocumentReplaced` | navigation snapshot, `Clear`, `CreateDocument` | full style-input invalidation | dirty |
| ordinary allocation | create document type/element/text/comment/PI | none by itself | none by itself |
| topology/order | append, insert, remove, reparent | full style-input invalidation | Layout dirty |
| template association | `CreateTemplateContents` | none by itself | none by itself |
| attribute targets | `SetAttributes` | CSS-owned suffix eligibility for surviving identities; full fallback otherwise | no intrinsic work |
| text targets | `SetText`, `AppendText` | AF4e full-document plan because `:empty` is text-sensitive | direct Layout work |
| unclassified patch | future `DomPatch` understood by `DomStore` | conservative full-document plan | conservative direct Layout work |
| stylesheet reconciliation | `<style>` text change, `<link>` add/remove/order change | stylesheet generation invalidation, full style invalidation | dirty |
| external stylesheet install/fail/abort/state change | `CssDecodedBlock`, load completion, error, abort | stylesheet generation invalidation, full style invalidation when the active stylesheet set/state changes | dirty |

Text-only DOM changes are conservatively full-restyled under AF4e. A future
selector dependency index may prove a particular fact style-neutral by
returning `None`; Browser must then preserve style generation and cache-key
eligibility. If the changed text belongs to a `<style>` element, stylesheet
reconciliation independently detects changed stylesheet input and submits a
full CSS-owned plan.

Empty DOM patch batches are no-ops for DOM/style generations and dirty state.

Target resolution occurs against the post-application staged store. Allocated
live keys resolve to canonical materialized IDs, allocated non-live keys are
valid historical targets, and never-allocated keys are typed failures. Any
genuine failure occurs before publication state or pending work is committed.

## Generation And Dirty-State Contract

`PageState` tracks:

```text
PageStyleGenerations {
  dom: u64,
  style_inputs: u64,
  stylesheets: u64,
}
```

The generation meanings are:

- `dom`: increments on every non-empty DOM replacement or DOM patch mutation.
- `style_inputs`: increments when DOM changes can affect selector matching,
  inline style attributes, inheritance, or document element order.
- `stylesheets`: increments when the document stylesheet set or loaded
  stylesheet contribution changes.

`style_dirty` means the cached `ComputedDocumentStyle` may not match current
style inputs. `layout_dirty` means layout/paint must not assume previous
geometry remains valid. U currently tracks layout dirty state, but full layout
generation caching is future work.

## Style Cache And Incrementality Contract

`PageState` retains this cache inside its `RetainedRenderState` owner:

```text
PageStyleCache {
  key: RetainedStyleArtifactKey,
  resolved: ResolvedDocumentStyle,
  computed: ComputedDocumentStyle,
}
```

Cache reuse is allowed only when:

```text
style_dirty == false
and cache.key == RetainedStyleArtifactKey {
  identity_domain,
  style_input_generation,
  stylesheet_generation
}
and CSS confirms that the retained resolved/computed artifacts were produced
under the current SelectorMatchingEnvironment
```

The cache stores owned resolved/computed artifacts, not `StyledNode<'_>`.
`StyledNode` remains a borrow-backed view and is rebuilt from the current DOM
and cached computed styles. This avoids a self-referential `PageState`.

The identity-domain component prevents reuse across full document replacement,
even if a newly materialized document has matching numeric DOM IDs. AC5 records
retained style artifact reuse, recompute, discard, and fallback decisions in
the retained render-state debug surface. These diagnostics count only retained
`ResolvedDocumentStyle` and `ComputedDocumentStyle` artifacts; rebuilding
borrow-backed `StyledNode` views is not retained artifact reuse.

### Incremental Suffix Restyle

The current AF1 partial restyle mechanism is an opaque CSS-owned document-order
suffix plan.

For attribute mutations with materialized dirty node IDs and a valid previous
cache:

```text
reuse resolved/computed prefix before earliest dirty element
recompute dirty element and document-order suffix
fallback to full recompute if proof fails
```

The suffix plan expresses semantic eligibility, not a guarantee that the
incremental algorithm runs. If no compatible previous style artifacts exist,
CSS reports incremental-unavailable and Browser performs a deterministic full
recompute; the retained-render fallback action does not claim that an
incremental computation was invoked.

This is conservative for the current selector model because:

- sibling selectors can affect following siblings
- inheritance can affect descendants
- attribute/class/id/style changes affect the target element
- no supported selector lets a later or descendant element affect an earlier
  ancestor or sibling

If support is added for selectors such as `:has()`, CSS must widen this proof
to full invalidation or replace it with CSS-owned selector-aware invalidation
dependencies. Browser must not gain a selector-specific branch.

Pending plans merge through the CSS-owned plan API. A pending full plan cannot
be narrowed by a later suffix plan. Multiple suffix plans are canonicalized,
deduplicated, and recompute from the earliest dirty node.

Patch-derived dirty IDs currently rely on the `DomStore` contract that
materialized `Node::id() == Id(PatchKey.0)`. If that identity mapping changes,
dirty IDs must be resolved by the DOM patch/materialization layer before
reaching `PageState`.

## CSS Engine Reuse Contract

The CSS crate performs pass-local computed-style reuse during document computed
style materialization.

Reuse key:

```text
ResolvedStyle + Option<ComputedStyle parent>
```

The cache is pass-local and cannot survive DOM, stylesheet, navigation, or
environment changes. Reuse remains valid only while computed style is a pure
function of resolved style plus parent computed style.

Future computed-value dependencies such as viewport units, font metrics,
writing mode, visited-link privacy state, container queries, media/device
state, or layout-dependent percentages must either be added to the cache key or
disable reuse for affected properties.

## Failure And Diagnostics Contract

Style failures are observable:

- CSS parse diagnostics stay in `StylesheetParse`.
- Style-resolution limit and computed-style errors propagate out of
  `PageState::build_style_phase_output()`.
- `browser::view::content(...)` renders a visible style-computation failure
  message rather than silently falling back to guessed styles.

Runtime code must not hide style errors by mutating `Node::style`, injecting
empty stylesheets, or bypassing supported-property validation.

## Regression, Performance, And Allocation Coverage

Milestone U coverage includes:

- page-load stylesheet attachment and initial computed style tests
- DOM mutation restyle tests for document replacement, attributes, tree
  mutation, text mutation, empty patches, and stylesheet text mutation
- duplicate same-URL stylesheet slot tests
- out-of-order external stylesheet arrival tests
- stale removed/failed/aborted stylesheet result tests
- complete-body CSS chunk assembly tests
- incremental suffix and cache-reuse tests
- pass-local computed-style reuse tests
- CSS Criterion benchmarks for parsing, selector matching, and style resolution
- deterministic smoke/heavy perf guards
- opt-in allocation guards
- representative HTML+CSS page snapshots in
  `crates/css/tests/fixtures/representative_pages`
- deterministic render phase-boundary fixtures through
  `browser::rendering::render_phase_boundary_debug_snapshot(...)`

Performance smoke and allocation guards are regression tripwires, not final
browser performance targets. Criterion results are the timing source of truth
for performance-sensitive changes.

## Future Extension Points

Future systems should extend the integration through explicit objects rather
than ad hoc call-site policy.

Rendering-side follow-on work must now integrate through the named rendering
hooks recorded in `browser::rendering::render_extension_hook_contracts()`
rather than bypassing the Milestone V ownership and invalidation contracts from
layout/paint call sites.

### Layout Generation

Add page-owned layout generation and layout cache keys:

```text
style generation changed
or viewport/layout environment changed
or layout-affecting DOM text changed
  -> layout dirty
  -> rebuild or reuse layout tree
```

Layout cache keys must include DOM identity, computed style generation,
viewport dimensions, font metrics, and any future layout environment inputs.

### Paint Generation

Paint invalidation should derive from layout generation, resource/image state,
input/pseudo state, and visual-only style changes. Paint must not consume
geometry from stale layout or styles from stale computed artifacts.

### Style Environment Generation

Add a style-environment generation when computed style starts depending on:

- viewport units
- media queries
- container queries
- font loading and font metrics
- writing modes
- visited-link privacy state
- UA/user/preferred color-scheme state

That generation must participate in page style cache keys and CSS reuse keys
where relevant.

### Stable DOM Identity

The current partial-restyle path resolves attribute and text patch targets
inside the staged `DomStore` and passes canonical materialized DOM IDs through
neutral publication facts. Valid historical targets remain explicit counts;
patch keys do not escape as selector identities. A more complete dynamic DOM
engine may enrich CSS-owned dependencies while preserving this identity
boundary and should reconcile stylesheet slots by DOM/style-node identity
first, with URL/text fallback only for snapshot mode.

### Selector-Aware Invalidation

AF4e refinement: exact text participates in `:empty`, so an aggregate DOM
publication containing text currently receives a CSS-owned full-document plan.
A neutral mutation fact
does not advance the Browser style-input generation by itself; only CSS
returning `Some(plan)` authorizes that transition and generic Style dirtiness.
`None` preserves cache-key eligibility and any pending plan through CSS-owned
merge. Browser applies this result once and emits one separate
`DomPublicationStyleInvalidated` request. Intrinsic entry-point effects remain
independent; `DomTextChanged` separately remains direct Layout input. CSS's
aggregate authorization is never copied onto an intrinsic mutation request.

Future selector invalidation should be introduced only with:

- explicit dependency extraction from parsed selector IR
- conservative fallback for unsupported selectors
- tests for sibling, ancestor, descendant, and future pseudo-class effects
- preservation of whole-document fallback on proof failure

### Stylesheet Cache

Parsed stylesheet resource reuse may be added across slots or requests only if
it preserves:

- document-order slot identity
- duplicate same-URL slot participation
- navigation/request filtering
- response identity and content policy
- parse diagnostics and hardening limits

Shared cache identity must never collapse `DocumentStyleSet` slots.

## U8 Close-Out Invariants

AF4a artifact validity: resolved and computed CSS artifacts retain the matching
environment used to produce them. Incremental or prefix reuse compares that
environment before reuse; Browser retained-style keys remain lifecycle
eligibility checks only.

Milestone U is complete only while these invariants hold:

- Runtime style resolution uses the structured CSS pipeline by default.
- `PageState` owns style lifecycle state, generations, dirty state, and cache
  lifetime.
- `DocumentStyleSet` owns stylesheet ordering and slot identity.
- `runtime_css` emits complete decoded stylesheet text and owns no CSS
  semantics.
- DOM mutations map to explicit restyle triggers.
- Text-only mutations dirty layout without invalidating computed style.
- CSS can authorize conservative suffix restyle with full fallback; Browser
  does not decide suffix safety.
- Stylesheet changes invalidate stylesheet generation and style cache.
- `ComputedDocumentStyle` is cached; `StyledNode<'_>` is rebuilt as a
  borrow-backed view.
- Layout/gfx consume typed computed style only.
- Representative page, perf, and allocation regression lanes exist and pass.
