# U8: Runtime Integration Contracts And Future Extension Points

Last updated: 2026-04-30
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
- `crates/css/src/computed/style_tree.rs`
- `crates/css/tests/representative_pages.rs`
- `crates/css/benches/css_bench.rs`

Related documents:

- `docs/css/u1-runtime-integration-architecture-css-pipeline-ownership.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`
- `docs/security/css-hardening.md`
- `docs/architecture/ARCHITECTURE.md`

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
  stylesheet load state, and redraw requests.
- `browser::PageState` owns the active DOM, document stylesheet set,
  style/layout dirty state, style generations, and page-local style cache.
- `browser::DocumentStyleSet` owns document-order stylesheet slots and exposes
  loaded `StylesheetParse` artifacts in cascade order.
- `crates/css` owns parsing semantics, selector matching, cascade, computed
  style materialization, diagnostics, and hardening limits.
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
3. `Tab::on_core_event(...)` filters every event by `(tab_id, request_id)`.
   Stale events cannot mutate active page state.
4. DOM snapshots use `RestyleHint::document_replaced()`.
5. DOM patch batches are applied atomically by `DomStore`; empty patch batches
   are no-ops for restyle; non-empty batches produce a `RestyleHint`.
6. `PageState::replace_dom(...)` installs the materialized DOM and marks DOM,
   style-input, or layout state according to the hint.
7. Head metadata, visible text, form-control state, image discovery, and
   stylesheet discovery are updated from the active DOM.
8. `PageState::reconcile_document_stylesheets()` updates `DocumentStyleSet`.
   Any slot-set or loaded-sheet change increments the stylesheet generation and
   marks style dirty with full invalidation.
9. External stylesheet arrivals call `PageState::apply_css_block(...)`.
   A successful install marks the stylesheet generation dirty. The tab/event
   layer observes the changed page state and requests redraw.
10. `browser::view::content(...)` calls `PageState::build_style_tree()`.
11. `PageState` either reuses a valid `ComputedDocumentStyle` cache, performs
    an incremental suffix recompute, or performs full style resolution.
12. `build_style_tree_from_computed_styles(...)` rebuilds a borrow-backed
    `StyledNode<'_>` view from the current DOM and cached computed styles.
13. Layout/gfx consume the `StyledNode` tree.

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

## Restyle Trigger Contract

DOM patch batches are classified by `RestyleTrigger` with this severity order:

```text
DocumentReplaced > TreeMutated > AttributesChanged > TextMutated
```

Current trigger behavior:

| trigger | examples | style effect | layout effect |
| --- | --- | --- | --- |
| `DocumentReplaced` | navigation snapshot, `Clear`, `CreateDocument` | full style-input invalidation | dirty |
| `TreeMutated` | create, append, insert, remove, reparent | full style-input invalidation | dirty |
| `AttributesChanged` | `SetAttributes` | partial suffix invalidation when cache proof exists; full fallback | dirty |
| `TextMutated` | `SetText`, `AppendText` | no style-input invalidation by itself in the current supported selector/property model | dirty |
| stylesheet reconciliation | `<style>` text change, `<link>` add/remove/order change | stylesheet generation invalidation, full style invalidation | dirty |
| external stylesheet install/fail/abort/state change | `CssDecodedBlock`, load completion, error, abort | stylesheet generation invalidation, full style invalidation when the active stylesheet set/state changes | dirty |

Text-only DOM changes do not invalidate computed style in the current selector
and property model. This contract must widen if future selector or generated
content support makes text content style-relevant, for example through `:empty`
or `:has(...)`. If the changed text belongs to a `<style>` element, stylesheet
reconciliation independently invalidates the stylesheet generation.

Empty DOM patch batches are no-ops for DOM/style generations and dirty state.

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

`PageState` caches:

```text
PageStyleCache {
  style_input_generation,
  stylesheet_generation,
  resolved: ResolvedDocumentStyle,
  computed: ComputedDocumentStyle,
}
```

Cache reuse is allowed only when:

```text
style_dirty == false
and cache.style_input_generation == generations.style_inputs
and cache.stylesheet_generation == generations.stylesheets
```

The cache stores owned resolved/computed artifacts, not `StyledNode<'_>`.
`StyledNode` remains a borrow-backed view and is rebuilt from the current DOM
and cached computed styles. This avoids a self-referential `PageState`.

### Incremental Suffix Restyle

The only U partial restyle mechanism is `AttributeSuffix`.

For attribute mutations with materialized dirty node IDs and a valid previous
cache:

```text
reuse resolved/computed prefix before earliest dirty element
recompute dirty element and document-order suffix
fallback to full recompute if proof fails
```

This is conservative for the current selector model because:

- sibling selectors can affect following siblings
- inheritance can affect descendants
- attribute/class/id/style changes affect the target element
- no supported selector lets a later or descendant element affect an earlier
  ancestor or sibling

If support is added for selectors such as `:has()`, this proof must be widened
to full invalidation or replaced with selector-aware invalidation dependencies.

Pending partial invalidations merge. A pending `Full` invalidation cannot be
narrowed by a later partial invalidation. Multiple attribute suffix scopes merge
and recompute from the earliest dirty node.

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
  `PageState::build_style_tree()`.
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

Performance smoke and allocation guards are regression tripwires, not final
browser performance targets. Criterion results are the timing source of truth
for performance-sensitive changes.

## Future Extension Points

Future systems should extend the integration through explicit objects rather
than ad hoc call-site policy.

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

The current partial restyle path can use patch keys for attribute suffix
invalidation. A more complete dynamic DOM engine should pass materialized
stable node IDs directly in restyle hints and reconcile stylesheet slots by
DOM/style-node identity first, with URL/text fallback only for snapshot mode.

### Selector-Aware Invalidation

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

Milestone U is complete only while these invariants hold:

- Runtime style resolution uses the structured CSS pipeline by default.
- `PageState` owns style lifecycle state, generations, dirty state, and cache
  lifetime.
- `DocumentStyleSet` owns stylesheet ordering and slot identity.
- `runtime_css` emits complete decoded stylesheet text and owns no CSS
  semantics.
- DOM mutations map to explicit restyle triggers.
- Text-only mutations dirty layout without invalidating computed style.
- Attribute mutations can use conservative suffix restyle with full fallback.
- Stylesheet changes invalidate stylesheet generation and style cache.
- `ComputedDocumentStyle` is cached; `StyledNode<'_>` is rebuilt as a
  borrow-backed view.
- Layout/gfx consume typed computed style only.
- Representative page, perf, and allocation regression lanes exist and pass.
