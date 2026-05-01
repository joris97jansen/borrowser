# U1: Runtime Integration Architecture And CSS Pipeline Ownership

Last updated: 2026-04-30
Status: architecture contract implemented; final Milestone U close-out lives in U8

This document is the source-of-truth contract for Milestone U issue 1. It
defines how Borrowser's rebuilt CSS engine integrates with the browser runtime,
where style resolution lives, which subsystem owns each phase, how page-load
and DOM-mutation restyles are triggered, and which integration boundaries are
in scope for Milestone U. The implemented post-U close-out contract is recorded
in `docs/css/u8-runtime-integration-contracts-extension-points.md`.

Related code:
- `crates/browser/src/page.rs`
- `crates/browser/src/tab/html.rs`
- `crates/browser/src/tab/css.rs`
- `crates/browser/src/tab/events.rs`
- `crates/browser/src/tab/discovery.rs`
- `crates/browser/src/view.rs`
- `crates/browser/src/dom_store`
- `crates/runtime_css/src/lib.rs`
- `crates/css/src/lib.rs`
- `crates/css/src/cascade.rs`
- `crates/css/src/computed.rs`

Related documents:
- `docs/css/u8-runtime-integration-contracts-extension-points.md`
- `docs/architecture/ARCHITECTURE.md`
- `docs/html5/dompatch-contract.md`
- `docs/html5/node-identity-contract.md`
- `docs/css/o1-rule-value-model-architecture.md`
- `docs/css/r1-cascade-architecture-style-resolution-contract.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/security/css-hardening.md`

## Purpose

Milestones N through S rebuilt the CSS subsystem as structured syntax, rule
model, selector matching, cascade, and computed-style layers. Milestone T
defined hardening expectations. Milestone U integrates those layers into real
runtime behavior.

The runtime integration contract is:

```text
network/html/css events
  -> Tab event routing
  -> PageState DOM + DocumentStyleSet ownership
  -> structured style resolution
  -> StyledNode tree
  -> layout/paint/view consumption
```

The browser runtime owns scheduling and invalidation. The CSS engine owns CSS
semantics. No runtime, layout, or paint layer may reimplement CSS parsing,
selector specificity, cascade priority, inheritance/default policy, computed
normalization, or supported-property metadata.

## Runtime Entry Points

The runtime integration entry points are intentionally narrow. Dirty marking,
generation tracking, cache reuse, and partial restyle attach at these boundaries
rather than being scattered through layout or paint code.

| entry point | owning layer | responsibility |
| --- | --- | --- |
| `Tab::on_dom_update(...)` | browser runtime | accept parsed DOM snapshots or materialized patch results, update page metadata, discover subresources, and act as the style-affecting DOM dirty boundary |
| `Tab::on_css_decoded_block(...)` | browser runtime | accept decoded stylesheet text from `runtime_css`, parse it through structured CSS entry points, and act as the stylesheet-set dirty boundary |
| `Tab::on_css_sheet_done(...)` | browser runtime | retire pending stylesheet load state and request redraw |
| `PageState::reconcile_document_stylesheets(...)` | page state | reconcile document `<style>` blocks and stylesheet links into document-order stylesheet slots during DOM updates |
| `PageState::apply_css_block(...)` | page state | parse an external stylesheet into `StylesheetParse` and install it into the pre-registered document-order stylesheet slot for the current navigation/request |
| `PageState::build_style_phase_output(...)` | page state | reuse or recompute page-owned resolved/computed style cache and rebuild the runtime `StylePhaseOutput` / `StyledNode` view |
| `build_style_tree_from_computed_styles(...)` | CSS engine | validate the current DOM against cached computed styles and build a borrow-backed `StyledNode` tree |
| `browser::view::content(...)` | browser view | request style-phase output construction and pass `StylePhaseOutput` into viewport/layout/paint orchestration |
| `DomStore::apply(...)` | browser DOM patch runtime | apply parser patch batches atomically before the style subsystem sees the materialized DOM |

`runtime_css` is not a style-resolution owner. It owns stylesheet byte
buffering, incremental UTF-8 assembly, abort handling, and decoded-block event
emission. It must not tokenize, parse, match, cascade, compute, cache computed
styles, or inspect DOM.

A future parse-worker model may execute `crates/css` parsing work off the main
thread, but that is an execution-host decision only. `runtime_css` may
transport and assemble CSS input; `crates/css` owns parsing semantics; and
browser/page state owns stylesheet attachment order and lifetime.

## Stylesheet Ordering Contract

Stylesheet cascade order is document/source order, not network completion order
or decoded-block arrival order.

When the runtime discovers an inline `<style>` block or stylesheet `<link>`, it
must register a page-owned stylesheet slot with a stable document-order position
for the current navigation/document generation.

External stylesheet slots may be pending, loaded, failed, or aborted. A pending
or failed sheet does not contribute declarations to cascade, but its slot
preserves the source-order position so that a later decoded stylesheet result is
installed into the correct cascade position.

Stylesheet slot identity is distinct from stylesheet resource/cache identity.
Two `<link rel="stylesheet">` elements with the same resolved URL still create
two document-order stylesheet slots. The runtime may reuse a parsed stylesheet
artifact internally when the accepted response identity is equivalent, but cache
reuse must not collapse stylesheet slots or change cascade order.

`CssDecodedBlock` handling must resolve the decoded block back to its existing
stylesheet slot by stylesheet-slot identity plus current navigation/request
identity and install or replace the parsed `StylesheetParse` there. URL alone
is not a valid attachment key. It must not append stylesheets according to
network completion order.

Inline style blocks and external stylesheets enter the same ordered stylesheet
set before cascade. Repeated DOM snapshots or patch materializations must
reconcile inline stylesheet slots instead of duplicating equivalent inline
stylesheet inputs.

## Ownership Boundaries

### Browser Runtime

The browser runtime owns:

- tab isolation and navigation-generation filtering
- document load state and pending stylesheet load state
- DOM snapshot or patch materialization before style resolution
- stylesheet discovery from parsed DOM
- document/source-ordered stylesheet attachment to `PageState`
- dirty-state tracking and restyle scheduling
- deciding when layout/paint must be invalidated after style changes
- surfacing style failures as page/view errors, not silent fallback

The browser runtime must not:

- parse CSS declaration values outside the CSS crate
- attach normative style results to `html::Node::style`
- compute selector specificity
- choose cascade winners
- invent inheritance/default behavior
- duplicate `PropertyId` metadata

### CSS Engine

The CSS engine owns:

- CSS syntax parsing and deterministic parse recovery
- engine-facing stylesheet/rule/declaration/value model construction
- selector parsing, validation, specificity, and matching
- cascade candidate construction and winner resolution
- inheritance/default source selection for supported properties
- specified-value validation and computed-value normalization
- `ResolvedDocumentStyle`, `ComputedDocumentStyle`, `ComputedStyle`, and
  `StyledNode` construction
- CSS hardening limits, diagnostics, and deterministic error reporting

The CSS engine must not:

- fetch resources
- own tab, navigation, or document lifecycle
- mutate browser DOM as part of structured style resolution
- schedule paints or layouts
- assume DOM identity remains stable across snapshots unless the runtime
  explicitly provides a stable mutation/invalidation contract

### Layout, Gfx, And View

Layout, gfx, and view code consume `StyledNode` or `ComputedStyle`.

They may:

- read typed computed values through accessors
- perform geometry, text measurement, hit-testing, and paint decisions
- use resource state, viewport state, and input state to render controls

They must not:

- parse authored CSS
- inspect cascade winners or resolved-style provenance
- recover from invalid supported CSS declarations
- synthesize supported-property initial/default values
- depend on the legacy `Node::style` compatibility projection for new runtime
  behavior

## Page-Load Style Lifecycle

The page-load lifecycle for styles is:

1. Navigation starts.
   `PageState::start_nav(...)` clears DOM, head metadata, visible text,
   form-control index, document stylesheet slots, style generations, dirty
   state, and style cache.
2. HTML bytes stream into `runtime_parse`.
   Parser events are routed through `Tab::on_core_event(...)` and gated by
   `(tab_id, request_id)`.
3. DOM output arrives.
   `Tab::on_dom_update(...)` installs the DOM snapshot or materialized patch
   result, extracts head metadata, reconciles inline `<style>` blocks into
   document-order stylesheet slots, seeds form control state, updates visible
   text, discovers stylesheets and images, and requests redraw.
4. Stylesheet links are discovered.
   `Tab::discover_stylesheets(...)` resolves each stylesheet URL against the
   document base URL, registers a document-order stylesheet slot in
   `PageState`, and sends `FetchStream { kind: Css }`.
5. CSS bytes stream into `runtime_css`.
   `runtime_css` buffers bytes per `(tab_id, request_id, stylesheet_slot_id)`,
   assembles UTF-8, and emits one complete-body `CssDecodedBlock` followed by
   `CssSheetDone`.
6. Stylesheets attach to page state.
   `Tab::on_css_decoded_block(...)` calls `PageState::apply_css_block(...)`,
   which parses through the structured CSS model path and installs the result
   into the pre-registered document-order slot.
7. View construction asks for styles.
   `browser::view::content(...)` calls `PageState::build_style_phase_output()`.
8. CSS engine resolves styles.
   `PageState` either reuses a valid `ComputedDocumentStyle` cache, performs
   an incremental suffix recompute, or runs full selector matching, cascade,
   and computed-style assembly through the CSS crate without mutating the DOM.
9. Layout and paint consume the style-phase output.
   `gfx::viewport::page_viewport(...)` receives `StylePhaseOutput`, builds
   `LayoutPhaseOutput`, and passes that structured handoff to paint.

The default runtime path for new work is the structured path:

```text
DOM + StylesheetParse[]
  -> resolve_document_styles(...)
  -> compute_document_styles_from_resolved_styles_with_reuse_stats(...)
  -> build_style_tree_from_computed_styles(...)
  -> StylePhaseOutput
```

Legacy APIs such as `attach_styles(...)`, `compute_style(...)`, and
`build_style_tree(...)` are compatibility surfaces only. They must not become
the browser runtime's normative style path again.

## Implemented Baseline After Milestone U

Milestone U moved the browser from view-requested style recomputation toward
page-owned style lifecycle management. The browser view now calls
`PageState::build_style_phase_output()`, and page state decides whether style
artifacts can be reused, partially recomputed, or fully recomputed.

The implemented baseline is:

- `DocumentStyleSet` owns document-order stylesheet slots so network completion
  order cannot affect cascade order.
- Inline `<style>` blocks and external stylesheets share one ordered author
  stylesheet set.
- `PageStyleGenerations` separates DOM generation, style-input generation, and
  stylesheet generation.
- `PageStyleCache` stores owned `ResolvedDocumentStyle` and
  `ComputedDocumentStyle` artifacts, never a self-referential `StyledNode`.
- Attribute mutations may use conservative suffix recomputation when a previous
  cache proves reusable; all uncertain cases fall back to full recomputation.
- Text-only DOM mutations dirty layout without invalidating computed style,
  unless stylesheet reconciliation changes the stylesheet set.
- Representative page tests, performance smoke guards, Criterion benchmarks,
  and opt-in allocation guards cover the structured path.

The remaining extension work is outside U: layout generation caching, paint
invalidation, selector-aware invalidation, richer stylesheet-link semantics,
UA/user origins, media/container query invalidation, and environment-sensitive
computed values.

## DOM Mutation And Restyle Triggers

Any runtime change that can affect selector matching, cascade inputs,
inheritance, or computed values is style-affecting.

Milestone U defines these trigger classes:

| trigger | examples | minimum invalidation for Milestone U |
| --- | --- | --- |
| document replacement | navigation, full `DomUpdate` snapshot, `Clear` patch batch | whole-document style dirty |
| tree structure change | node create, append, insert, remove, reparent | affected subtree dirty; ancestor/sibling selectors may escalate to whole-document |
| element attribute change | `class`, `id`, `style`, presentation-relevant attributes, link rel/href | target element and descendants dirty; stylesheet discovery may run |
| inline style block change | `<style>` text insertion/update/removal | stylesheet set dirty and whole-document style dirty |
| external stylesheet arrival | `CssDecodedBlock` | stylesheet set dirty and whole-document style dirty |
| stylesheet load error/abort | network error, unsupported content type | pending-load state dirty; style tree unchanged unless the failed sheet previously contributed |
| text change | `SetText`, `AppendText` | layout dirty; style dirty only when the text belongs to a `<style>` element or future selector support depends on text state |
| pseudo/input state change | future hover/focus/active/visited hooks | target-dependent style dirty once pseudo-class matching is supported |

The current parser patch path applies patch batches through `DomStore` before
materializing the DOM. Patch batches are classified before materialization;
empty batches are no-ops, structural mutations conservatively invalidate the
whole style input set, and attribute mutations can produce an
`AttributeSuffix` invalidation scope when patch identity maps to materialized
node identity.

### Mutation To Style Work Mapping

The runtime must normalize mutations into style work before layout work:

```text
DOM mutation or stylesheet mutation
  -> style dirty marking
  -> style resolution or style-cache reuse
  -> layout dirty marking
  -> paint invalidation
```

Layout must not run against a DOM version newer than the `StyledNode` tree.
Paint must not consume geometry produced from stale styles after a style-affecting
mutation.

## Incrementality And Cache Boundaries

Milestone U introduced basic incremental mechanisms with correctness before
narrow invalidation.

Implemented first-stage mechanisms:

- page-owned DOM, style-input, and stylesheet generation counters
- page-owned resolved/computed style cache keyed by style-input and stylesheet
  generations
- pass-local computed-style reuse inside the CSS crate
- conservative attribute suffix recomputation with full fallback
- whole-document restyle fallback whenever invalidation precision is uncertain

Required safety rules:

- cache hits must be invalidated by any DOM mutation that can change selector
  matching, inheritance, inline styles, or document order
- stylesheet-order changes invalidate the document style generation even if the
  stylesheet text is identical
- cache keys must include navigation generation or an equivalent document
  identity to avoid cross-navigation contamination
- caches must not expose mutable `ComputedStyle` or `StyledNode` state to
  consumers
- caches must respect Rust ownership boundaries: if `StyledNode` remains a
  borrow-backed DOM view, it must be rebuilt from owned DOM and computed-style
  artifacts or stored in an arena/owned representation that avoids
  self-referential `PageState` layouts
- cache reuse must preserve CSS crate hardening limits and diagnostics
- partial restyle must have a whole-document fallback for unsupported selector
  features, unknown patch effects, or identity mismatches

Parsed stylesheet attachment and page-level style-cache lifetime are
browser/page concerns. Semantic style computation and pass-local memoization of
computed style materialization are CSS-engine concerns.

## Runtime Invariants

The following invariants remain mandatory for Milestone U implementation:

- A `Tab` owns exactly one active `PageState` for its current navigation
  generation.
- Runtime events from stale `(tab_id, request_id)` pairs must not mutate the
  active page or stylesheet set.
- `PageState::css_stylesheets()` exposes loaded stylesheets in document/source
  order for the active navigation. Network arrival order must not affect
  cascade order.
- Inline `<style>` blocks and external stylesheets must enter the same
  structured stylesheet model before cascade.
- Structured style resolution must not mutate `html::Node`.
- `Node::style` is a legacy projection and is not a contract for new layout,
  view, or runtime code.
- `StyledNode` must correspond to the same DOM snapshot/materialized DOM that
  was passed into style resolution.
- A style-resolution error must be observable by the browser view; it must not
  silently render with an empty or partially guessed style tree.
- Layout and paint may assume `ComputedStyle` is total for the supported
  property subset.
- DOM patch batches must be applied atomically before style invalidation sees
  their effects.
- Any future partial invalidation must be conservative when selectors,
  attributes, sibling order, or inherited properties can affect elements
  outside the immediate mutation target.

## Milestone U Scope

In scope for Milestone U:

- make the structured CSS pipeline the default browser runtime path
- establish dirty-state and restyle hooks for document, stylesheet, and
  relevant DOM mutations
- remove or isolate remaining runtime dependencies on legacy DOM-attached style
  projection
- introduce safe caching where generation keys make correctness obvious
- add representative runtime page regressions
- add performance and allocation guards for style resolution and redraw paths
- document contracts required by later layout integration work

Out of scope for Milestone U:

- full browser-grade selector invalidation
- cascade layers, user origin, UA stylesheet completeness, animations, and
  transitions beyond existing reserved model slots
- Shadow DOM, scoped styles, adopted stylesheets, media queries, container
  queries, and dynamic viewport-query invalidation
- JavaScript-driven arbitrary DOM mutation APIs beyond the runtime mutation
  hooks needed to keep future integration coherent
- layout-dependent computed value resolution such as percentage sizing or font
  metric-dependent values unless explicitly introduced by a later issue
- cross-tab shared style caches
- speculative parallel style calculation that complicates tab/document
  ownership before correctness and measurement are established

## Future Extension Points

Future work should extend this contract by adding explicit objects rather than
spreading policy across call sites.

Likely extension points:

- `DocumentStyleSet`: page-owned ordered author stylesheet collection,
  including inline style blocks and external sheets, with stable
  document-order slots whose states can be pending, loaded, failed, or aborted
- `StyleInvalidationScope` or future `StyleInvalidationSet`: conservative
  description of DOM/style mutations that must be restyled
- `StyleResolver` or `StyleContext`: CSS-engine entry point for resolving a DOM
  plus stylesheet set with limits, diagnostics, and optional internal caches
- `StyleGeneration`: browser-owned generation keys for DOM, stylesheets,
  computed styles, layout, and paint
- `RuntimeStyleCache`: page-owned cache of parsed stylesheets and/or derived
  style artifacts keyed by navigation, DOM, stylesheet, and future
  style-environment generations

Any new object must preserve the ownership split:

- browser/page objects schedule work and own document lifetime
- CSS-engine objects own CSS semantics and diagnostics
- layout/gfx objects consume typed style output only
