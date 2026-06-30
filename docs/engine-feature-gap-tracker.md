# Engine Feature Gap Tracker

Status: living tracker

This is a small, non-normative checklist for visible engine gaps. Detailed
contracts still live in the milestone docs under `docs/css/`, `docs/rendering/`,
and `docs/html5/`.

Use this file to answer: "what major browser/CSS features are still missing?"

## Flexbox

Current supported subset:

- `display: flex`
- block-level flex containers
- direct generated in-flow children as flex items
- row-only, single-line layout
- internal default grow/shrink/basis behavior
- default cross-axis stretch behavior for auto-height items

Missing:

- `inline-flex`
- authored `flex-direction`, including column and reverse directions
- authored `flex-wrap` and multi-line layout
- `flex-flow`
- `justify-content`
- authored `align-items`, `align-self`, and `align-content`
- `gap`, `row-gap`, and `column-gap`
- `order`
- authored `flex`, `flex-grow`, `flex-shrink`, and `flex-basis`
- baseline alignment
- full min-content/max-content flexbox behavior
- flex-specific paint behavior

Notes:

- Paint does not implement flexbox. Layout computes flex geometry; paint only
  consumes final layout rectangles.
- Unsupported flex behavior is tracked in
  `docs/rendering/z6-flex-unsupported-feature-handling.md`.

## CSS Property Coverage

Current supported property set is intentionally small. Milestone AD is complete
only as a CSS value/property foundation milestone. It does not mean broad CSS
property coverage, selector conformance, cascade conformance, media queries,
custom properties, animations, layout behavior, paint behavior, or WPT-backed
CSS compatibility are complete.

Architecture and foundation status:

- AD1 defines the CSS-owned value/property architecture and subsystem
  boundaries for future property work; see
  `docs/css/ad1-css-value-property-ownership-architecture.md`.
- AD1 is architecture-only. It does not add broad property coverage, CSS-wide
  keyword support, shorthand support, registry-backed invalidation impact
  metadata, or new Browser/runtime impact APIs.
- AD2 adds the CSS-owned typed core value model foundation for the current
  supported property parsing subset; see
  `docs/css/ad2-typed-core-css-value-model.md`.
- AD2 does not add broad CSS Values and Units coverage, new property families,
  URL/resource loading, broad function support, CSS-wide keywords, shorthands,
  or invalidation impact classification.
- AD3 adds shared CSS-wide keyword handling for supported properties; see
  `docs/css/ad3-css-wide-keyword-handling.md`. `initial`, `inherit`, and
  `unset` are supported through cascade winner selection and CSS-owned
  resolved-style materialization. `revert` and `revert-layer` are recognized
  but unsupported until cascade origin/layer support exists.
- AD4 formalizes the CSS-owned supported longhand registry and metadata model;
  see `docs/css/ad4-css-property-registry-longhand-metadata.md`. Supported
  longhands now expose explicit registry metadata and deterministic metadata
  debug output. AD4 does not add broad property coverage, shorthand support,
  or the full AD7 invalidation taxonomy.
- AD5 defines the specified-value and computed-value boundaries for every
  currently supported longhand; see
  `docs/css/ad5-specified-computed-value-boundaries.md`. The boundary
  inventory is registry-derived and deterministic, and representative tests
  cover specified-to-computed conversion, initial values, and inherited values.
  AD5 does not add used-value resolution, actual-value resolution,
  layout-dependent percentage resolution, shorthand expansion, or broader CSS
  property coverage.
- AD6 adds a CSS-owned shorthand expansion foundation and the narrow
  `outline` shorthand subset; see
  `docs/css/ad6-shorthand-expansion-foundation.md`. Expanded longhands flow
  through the existing longhand registry, specified-value parser, CSS-wide
  keyword handling, computed-value normalization, and invalidation metadata.
  Broad shorthand coverage remains missing.
- AD7 replaces the old two-bucket longhand impact hook with CSS-owned
  composable invalidation impact flags for currently supported longhands; see
  `docs/css/ad7-css-owned-invalidation-impact-classification.md`.
  Browser/runtime consumes a narrow CSS-owned projection for retained dirty
  state and work planning without owning CSS property semantics.
- AD8 adds deterministic declaration-list parsing and diagnostics coverage for
  declaration classification; see
  `docs/css/ad8-deterministic-declaration-parsing-diagnostics.md`. Inline
  style attributes now use the model declaration-list parser instead of a
  synthetic stylesheet wrapper. Unsupported standard properties remain generic
  unsupported declarations unless future issues add real supported property or
  shorthand contracts.
- AD9 adds deterministic CSS-owned property coverage and metadata debug
  snapshots for the current supported subset; see
  `docs/css/ad9-css-property-coverage-metadata-debug-snapshots.md`. The
  snapshots expose current longhand coverage, registry metadata, representative
  value parsing, supported shorthand registration and expansion, and
  invalidation classification. AD9 does not add broad property coverage or new
  CSS semantics.
- AD10 closes Milestone AD as a CSS value/property foundation milestone; see
  `docs/css/ad10-css-value-property-foundation-closeout.md`. The closeout
  summarizes the value model, property registry, CSS-wide keyword support,
  specified/computed boundaries, shorthand foundation, invalidation
  classification, declaration parsing, unsupported behavior, and future
  extension points. AD10 does not add or imply broad CSS property coverage.

Major missing families remain:

- borders:
  - full `border` shorthand support
  - `border-width`, `border-style`, and `border-color` shorthands
  - additional border styles beyond the supported subset
  - border radius
  - border images
  - logical border properties
- outlines:
  - `outline-offset`
  - `auto` and additional outline styles beyond the supported subset
  - rounded outline geometry
- CSS shorthand and multi-component declaration expansion:
  - foundation exists for CSS-owned shorthand expansion
  - only the narrow `outline` shorthand subset is currently supported
  - broad shorthand parsing is not yet supported
  - current feature work should prefer explicit longhands unless shorthand
    support is part of the issue scope
- fonts: `font-family`, `font-weight`, `font-style`, line-height variants
- text:
  - `white-space`, text alignment, text transform
  - supported text-decoration subset: `text-decoration-line: none` and
    `text-decoration-line: underline`
  - full CSS Text Decoration beyond the AA5 underline subset is missing; see
    `docs/rendering/aa5-text-decoration-rendering-subset.md`
  - missing text-decoration follow-ups include `text-decoration` shorthand,
    `text-decoration-color`, `text-decoration-style`,
    `text-decoration-thickness`, `text-underline-offset`,
    `text-underline-position`, overline, line-through, blink, skip-ink, real
    font-metric/table-based underline positioning, full propagation and
    cancellation semantics, nested inline behavior beyond AA5, atomic
    inline/replaced behavior, bidi/ruby/vertical-writing-mode behavior, and UA
    stylesheet link underline behavior
- backgrounds: images, repeat, position, size, attachment, multiple backgrounds
- box effects: shadows, opacity, transforms, filters
- layout: floats, clear, grid, table layout, multi-column layout
- positioning: complete absolute/fixed/sticky geometry and full CSS
  stacking/compositing beyond the AB3/AB4 supported positioned integer
  `z-index` and stacking-order execution subset
- sizing: full intrinsic sizing keywords and browser-compatible min/max nuance
- overflow: scrollbars, scroll containers, scroll offsets, overflow-x/y split
  behavior, viewport/body overflow propagation
- selectors and media: broad selector coverage, pseudo-classes,
  pseudo-elements, selector invalidation, media queries, and container queries
- custom properties and variables
- animations and transitions
- CSS Values and Units beyond the current narrow subset
- CSS Color beyond the current narrow keyword and hex subset
- WPT-backed broad CSS conformance

## Layout

Missing or incomplete:

- CSS Grid
- table layout
- floats and clear
- full positioned layout geometry
- full CSS stacking/compositing beyond the AB3/AB4 supported positioned
  integer `z-index` and stacking-order execution subset
- writing modes and logical-axis remapping
- fragmentation and pagination
- full inline formatting behavior, including bidi and advanced line breaking
- full replaced-element and intrinsic-size compatibility
- margin/border/padding completeness across all formatting contexts and edge
  cases

## Paint / GFX

Current supported subset:

- Milestone AA's supported paint model, invariants, limitations, and future
  attachment points are closed out in
  `docs/rendering/aa9-paint-model-invariants-extension-points.md`.
- Milestone AB's supported stacking, semantic layering, paint invalidation,
  repaint execution, debug-surface, and future attachment-point model is closed
  out in
  `docs/rendering/ab8-stacking-compositing-invalidation-closeout.md`.
- deterministic AA paint ordering for box background, box border, list marker,
  overflow clip scope for contents and descendants, inline formatting content,
  child subtrees in layout order, and box outline; see
  `docs/rendering/aa7-deterministic-paint-ordering-layering-rules.md`
- deterministic Paint-owned paint-operation debug snapshots for structural visual
  regression coverage of the supported AA paint subset; see
  `docs/rendering/aa8-paint-debug-visual-regression-surface.md`
- basic paint-time overflow clipping when Layout exposes an overflow clip; see
  `docs/rendering/aa6-overflow-clipping-paint-behavior.md`
- narrow AB3/AB4 z-order and stacking-order execution supported subset:
  authored/computed
  `z-index: auto | <integer>` is supported, but affects paint ordering only for
  generated boxes where `position != static`; positioned boxes with integer
  `z-index` can create paint-owned child stacking contexts; AB4 orders those
  child contexts around the context source subtree through deterministic
  negative, zero, and positive stacking slots, with semantic order snapshots,
  operation snapshots, and immediate painting routed through
  `StackingContextTree::ordered_slots`; see
  `docs/rendering/ab3-z-order-layering-semantics.md` and
  `docs/rendering/ab4-stacking-context-paint-order.md`
- AB5 structured paint invalidation foundation: explicit repaint triggers,
  conservative `Document`/`Viewport` scopes, deterministic paint invalidation
  requests/state, and runtime-owned pending paint dirtiness; see
  `docs/rendering/ab5-structured-paint-invalidation-model.md`
- AB6 basic targeted repaint execution uses the structured AB5 invalidation
  model with conservative `Document` and `Viewport` execution scopes;
  browser/runtime derives repaint scope, and GFX consumes the selected scope;
  see `docs/rendering/ab6-basic-targeted-repaint-behavior.md`
- AB7 deterministic debug and regression surfaces expose paint-owned
  stacking/layering decisions and browser/runtime-owned invalidation/repaint
  planning through semantic, backend-independent snapshots; see
  `docs/rendering/ab7-deterministic-debug-regression-coverage.md`

Missing or incomplete:

- full border rendering beyond the supported physical solid-border subset
- full outline rendering beyond the supported rectangular longhand subset
- border radius
- background images and advanced background painting
- shadows, transforms, opacity, filters, and advanced clipping interactions
- full CSS stacking/compositing remains missing beyond the AB3/AB4 supported
  positioned integer `z-index` and stacking-order execution subset, including:
  - complete CSS painting order for all formatting contexts
  - full stacking-context creation triggers beyond positioned integer
    `z-index`
  - opacity-created stacking contexts
  - transform-created stacking contexts
  - filter/backdrop-filter-created stacking contexts
  - perspective and 3D transform stacking behavior
  - mix-blend-mode, isolation, and blending/compositing semantics
  - contain/containment and will-change-related stacking/compositing behavior
  - full positioned layout geometry for absolute/fixed/sticky elements
  - complete CSS-compatible `z-index` behavior across all stacking-context
    triggers, formatting contexts, and positioned-layout cases
  - inline, float, table, flex, grid, and pseudo-element painting-order
    interactions
  - top layer behavior such as dialogs/popovers
  - scroll containers, scrollbars, clipped descendants, and advanced overflow
    interactions
  - masks, clip-path, advanced clipping, and border-radius clipping
    interactions
  - compositor layer promotion, retained display lists/scenes, GPU compositing,
    and compositor-layer invalidation
- advanced optimized repaint remains missing: retained paint scenes/display
  lists, minimal dirty-region propagation, paint-source-scoped repaint,
  compositor-layer invalidation, GPU compositing, and backend partial
  raster/partial repaint execution are not implemented
- scrollbars and scrollable overflow painting
- selection painting outside supported text-control paths
- font fallback and advanced text shaping
- flex/grid/table-specific paint behavior where future layout data requires it
- pixel/raster visual regression infrastructure

Notes:

- Basic paint-time overflow clipping is supported when Layout exposes an
  overflow clip; Layout owns overflow semantics, and Paint consumes final layout
  clip metadata. See
  `docs/rendering/aa6-overflow-clipping-paint-behavior.md`.
- AA8 visual regression coverage is structural Paint-owned paint-operation
  snapshotting, not screenshot or pixel comparison. Browser/runtime may expose
  this debug output but does not define paint semantics.
- AB7 snapshots are semantic debug/regression surfaces for stacking and
  invalidation, not pixel/raster visual regression infrastructure.
- AB8 uses "compositing semantics" to mean the current semantic paint layering
  model and future compositor extension points. It does not mean compositor
  layers, GPU layers, retained scenes, dirty-region invalidation, or backend
  partial-raster behavior are implemented.

## Browser Runtime / Platform

Current supported subset:

- AC1 runtime-owned retained render state foundation: `PageState` owns
  `RetainedRenderState`, typed `RenderEpoch` semantics, deterministic retained
  render-state debug snapshots, no-op epoch preservation, fallible recompute
  epoch correctness, and explicit frame-local identity non-retention; see
  `docs/rendering/ac1-retained-render-state-runtime-contract.md`.
- AC2 stable retained render identities: browser/runtime owns typed retained
  render identities, uses DOM IDs only as anchors/provenance, treats full
  document replacement as a retained identity boundary, prunes removed
  artifacts before allocation, avoids retained ID recycling within the active
  document identity lifetime, exposes deterministic retained identity debug
  output, and keeps layout, paint, stacking, and traversal/source-order IDs
  explicitly frame-local; this does not add style/layout/paint caches,
  dirty-state planning, retained layout trees, retained paint scenes/display
  lists, compositor/GPU concepts, or dirty-region rendering; see
  `docs/rendering/ac2-retained-render-identities.md`.
- AC3 explicit dirty-state tracking: browser/runtime owns typed style, layout,
  and paint dirty entries with deterministic reasons, scopes, propagation,
  merge/deduplication behavior, conservative document/viewport fallbacks, and
  retained dirty-state debug output; this does not add retained layout caches,
  retained paint caches, dirty-region rendering, compositor/GPU concepts, or
  broad browser-owned CSS property-impact classification; see
  `docs/rendering/ac3-explicit-dirty-state-tracking.md`.
- AC4 deterministic render work planning: browser/runtime derives an explicit
  pre-execution `RenderWorkPlan` from retained dirty state, pending
  invalidation work, and existing retained style artifact state; plans expose
  restyle, relayout, repaint, and conservative fallback decisions through
  deterministic debug output; this does not add retained layout caches,
  retained paint caches, targeted relayout, dirty-region rendering,
  compositor/GPU concepts, retained display lists/scenes, or a browser-owned
  CSS property-impact table; see
  `docs/rendering/ac4-deterministic-render-work-plans.md`.
- AC5 retained style artifact reuse: browser/runtime keys retained
  `ResolvedDocumentStyle` and `ComputedDocumentStyle` artifacts by retained
  identity domain plus style-input and stylesheet generations, reuses them for
  no-op and viewport-only style-clean updates, conservatively discards them on
  full style invalidation such as stylesheet or document replacement changes,
  records reuse/recompute/discard lifecycle counts in deterministic debug
  output, and keeps CSS ownership of selector matching, cascade behavior,
  computed values, inheritance, property meaning, and style-tree construction;
  this does not add fake selector dependency tracking, browser-owned CSS
  property impact tables, retained layout caches, retained paint caches,
  display lists/scenes, compositor/GPU concepts, or dirty-region rendering;
  see `docs/rendering/ac5-retained-style-artifact-reuse.md`.
- AC6 retained layout artifact foundation: browser/runtime owns retained
  layout artifact lifetime, keys, counters, dirty-state integration, and debug
  accounting while Layout owns retained layout artifact materialization,
  geometry, formatting behavior, layout order, and layout metadata. No-op and
  safely classified paint-only updates can reuse retained layout. Requested
  relayout scopes are explicit, and narrower scopes conservatively execute as
  document relayout when Layout cannot safely perform true targeted relayout;
  see `docs/rendering/ac6-retained-layout-artifact-foundation.md`.
- AC7 retained paint artifact reuse and repaint planning: browser/runtime owns
  retained paint artifact lifetime, keys, counters, repaint planning,
  conservative fallbacks, and debug reporting while Paint owns the semantic
  paint artifact, stacking contexts, paint ordering, semantic layers, and
  primitives. No-op frames can reuse retained paint artifacts when runtime keys
  still match, and paint-only changes can plan repaint without relayout where
  CSS-owned impact classification supports that; see
  `docs/rendering/ac7-retained-paint-artifact-reuse-repaint-planning.md`.
- AC8 incremental rendering debug snapshots: browser/runtime exposes
  deterministic retained-state, dirty-state, render work-plan, execution-trace,
  generation, artifact lifecycle, reuse/recompute/discard, and conservative
  fallback debug output for retained rendering decisions. These snapshots are
  internal regression contracts and do not introduce new optimization behavior,
  dirty-region rendering, compositor/GPU concepts, retained display lists, or
  new artifact reuse semantics; see
  `docs/rendering/ac8-incremental-rendering-debug-snapshots.md`.
- AC9 incremental rendering performance guardrails: browser/runtime has
  deterministic CI-safe retained rendering guard tests for initial render,
  no-op repeated render, repeated viewport resize, text/content update,
  paint-only style update, layout-affecting style update, stylesheet/global
  style update, and representative in-repo page updates. These guardrails use
  existing lifecycle counters, retained dirty-state cleanup, retained artifact
  states, retained identity counts, and frame traces rather than wall-clock CI
  thresholds. Heap-byte allocation measurement is not part of the default
  browser rendering proof; see
  `docs/rendering/ac9-incremental-rendering-performance-guardrails.md`.
- Milestone AC retained rendering runtime foundation closeout: browser/runtime
  now has documented retained rendering purpose and scope, subsystem ownership
  boundaries, retained versus rebuilt state, retained state lifetime, retained
  identity invariants, dirty-state propagation, deterministic work-plan
  derivation, style/layout/paint artifact reuse rules, conservative fallback
  behavior, internal debug/regression snapshot surfaces, regression test
  surfaces, performance guardrails, known limitations, and future extension
  points. This marks Milestone AC complete only as a retained runtime
  foundation and does not claim compositor/GPU layers, full dirty-region
  rendering, selector dependency invalidation, CSS containment, HTML parser
  conformance, JavaScript/event-loop invalidation, or broad WPT integration;
  see `docs/rendering/ac10-retained-rendering-runtime-closeout.md`.

Missing or incomplete:

- true minimal/subtree relayout execution and layout dependency graphs
- compositor/GPU layer architecture and scheduling
- full dirty-region rendering and partial raster invalidation
- retained display-list or scene-graph architecture beyond AC7's semantic
  paint artifact reuse
- full selector dependency invalidation
- CSS containment
- heap-byte allocation guards for browser rendering retained-state updates
- JavaScript execution and DOM bindings
- event loop, timers, microtasks, and script-triggered invalidation
- full DOM mutation APIs
- full HTML parser conformance
- broad WPT integration for rendering/runtime retained behavior
- full form submission and navigation behavior
- focus management and keyboard navigation
- origin/security policy model
- storage, cookies, history, and session behavior
- resource loading for fonts, media, and broader image formats
- accessibility tree

## HTML / DOM

Missing or incomplete:

- full DOM API surface
- script integration
- form algorithms beyond current control-state support
- full editing/contenteditable behavior
- full custom element/template/shadow DOM behavior

## How To Update

When a feature lands:

1. Remove or narrow the item here.
2. Link to the relevant contract doc if the behavior is subtle.
3. Keep this file short; move detailed rules to subsystem docs.
