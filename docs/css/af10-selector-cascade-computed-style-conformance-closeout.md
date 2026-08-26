# AF10: Selector, Cascade, And Computed-Style Conformance Closeout

Last updated: 2026-08-26
Status: closeout contract for Milestone AF issue 10

This document closes Milestone AF as a browser-shaped selector, specificity,
cascade, inheritance, computed-style, and style-dependency foundation for the
explicitly supported subset. It does not claim full CSS conformance, broad
selector or property coverage, media-query evaluation, custom properties,
animations, transitions, CSSOM, JavaScript-facing style APIs, or broad WPT
coverage.

AF10 adds no selector, cascade, inheritance, computed-value, invalidation,
Layout, Paint, or Browser/runtime semantics. It consolidates the deterministic
evidence implemented by AF1 through AF9, records the final ownership and
extension boundaries, and keeps unsupported behavior explicit.

## Related code

- `crates/css/src/selectors/`
- `crates/css/src/document_selector_matching.rs`
- `crates/css/src/cascade/`
- `crates/css/src/specified/`
- `crates/css/src/properties/`
- `crates/css/src/computed/`
- `crates/css/src/style_invalidation.rs`
- `crates/css/src/style_invalidation/dependencies.rs`
- `crates/browser/src/document_style.rs`
- `crates/browser/src/page/style_cache.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/page/debug.rs`
- `crates/layout/src/`
- `crates/gfx/src/paint/`

## Related contracts

Current Milestone AF authority:

- `docs/css/af1-selector-cascade-computed-style-architecture-contract.md`
- `docs/css/af2-selector-ast-and-parser.md`
- `docs/css/af3-selector-specificity.md`
- `docs/css/af4a-document-matching-environment.md`
- `docs/css/af4b-selector-dom-query-contract.md`
- `docs/css/af4c-html-host-language-selector-comparison.md`
- `docs/css/af4d-tree-structural-pseudo-class-matching.md`
- `docs/css/af4e-selector-invalidation-parser-conformance-closeout.md`
- `docs/css/af5-stylesheet-rule-collection-source-order-contract.md`
- `docs/css/af6-cascade-ordering-winner-selection-contract.md`
- `docs/css/af7-specified-value-defaulting-source-resolution.md`
- `docs/css/af8-computed-style-document-artifact-contract.md`
- `docs/css/af9-selector-cascade-invalidation-dependencies.md`

Foundation and downstream contracts:

- `docs/css/ad3-css-wide-keyword-handling.md`
- `docs/css/ad4-css-property-registry-longhand-metadata.md`
- `docs/css/ad5-specified-computed-value-boundaries.md`
- `docs/css/ad7-css-owned-invalidation-impact-classification.md`
- `docs/css/ad10-css-value-property-foundation-closeout.md`
- `docs/html5/ae1-html-parser-dom-ownership-contract.md`
- `docs/html5/ae2-parser-created-dom-node-model.md`
- `docs/html5/ae14-html-parser-foundation-closeout.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`
- `docs/rendering/ac5-retained-style-artifact-reuse.md`
- `docs/rendering/ac10-retained-rendering-runtime-closeout.md`
- `docs/engine-feature-gap-tracker.md`

Historical selector, matching, cascade, and computed-style contracts remain
useful with their reconciliation notices: Milestones P1-P8, Q1-Q8, R1-R9,
S1-S9, and U1-U8. AF6 supersedes historical R3/R4 winner, tie, and inline
priority details. AF7 supersedes historical parent-`ResolvedStyle` defaulting
wording. AF10 does not restore superseded algorithms.

## Completed scope

Milestone AF completes the supported foundation for:

- typed CSS-owned selector parse results and selector AST;
- deterministic parsing of the current selector subset;
- deterministic matching against an explicit projection of AE parser-created
  DOM;
- typed A/B/C specificity derived from selector IR;
- immutable, fallible stylesheet rule collection with typed active, inactive,
  skipped, invalid, and unsupported states;
- deterministic current-scope cascade ordering and invariant-safe sparse
  winner selection;
- author stylesheet, inline style, and the current Browser UA stylesheet
  integration;
- total registry-backed specified-source resolution, inheritance, initial
  fallback, and supported CSS-wide keywords;
- CSS-owned document-level computed styles as the authoritative input to the
  styled-tree, Layout, and Paint pipeline;
- an owned CSS selector/cascade dependency artifact and opaque invalidation
  decisions consumed by Browser/runtime;
- stable semantic assertions, versioned snapshots, bounded diagnostics,
  parser-backed document fixtures, fuzz corpora, and runtime handoff tests.

Completion means the architecture and behavior of this subset are explicit,
deterministic, inspectable, and extensible. It does not mean the missing CSS
families listed below are complete.

## Final production pipeline

The authoritative supported path is:

```text
CSS source
  -> syntax/model parsing
  -> typed SelectorListParseResult on each style rule

AE html::parse_document
  -> parser-created Node tree + parser-selected DocumentMode
  -> CSS SelectorDomIndex projection
  -> SelectorMatchingEnvironment

stylesheet collection inputs
  -> one fallible immutable RuleCollection per style execution
  -> supported selector matching
  -> matched selector result + effective matched specificity
  -> validated declaration candidates
  -> origin / importance / attachment / specificity / source order /
     declaration order
  -> sparse winners
  -> total resolved sources
  -> top-down computed values
  -> ComputedDocumentStyle
  -> StylePhaseOutput
  -> Layout used values and geometry
  -> Paint visual output
```

The invalidation path shares the same active collection and selector meaning:

```text
AF5 active rules with supported declaration candidates
  -> CSS-owned StyleDependencyArtifact
  -> neutral Browser mutation facts returned to CSS
  -> CSS-owned no-op / suffix / full invalidation decision
  -> Browser retained-artifact validation and execution orchestration
  -> AD7 impact classification after recomputation
```

No fixture, debug serializer, Browser branch, Layout branch, or Paint branch
reimplements selector matching, cascade ordering, inheritance, property
meaning, or computed-value normalization.

## Ownership boundaries

| Subsystem | Owns | Must not own |
| --- | --- | --- |
| CSS syntax/model/selectors | CSS tokens and rules, selector AST, parser outcomes, selector matching, host-language selector comparison, specificity, selector diagnostics | HTML recovery, DOM mutation lifecycle, Layout geometry, Paint output |
| CSS cascade/specified/computed | rule collection, candidate admission, priority, winners, CSS-wide keywords, inheritance/defaulting sources, computed values, style dependencies and invalidation meaning | Browser scheduling, used-value geometry, paint ordering |
| HTML/parser | parser-created DOM, namespaces, attributes, document mode, template-content boundaries, patch production | selector or cascade semantics |
| Browser/runtime | stylesheet discovery and lifetime, retained artifacts, neutral mutation facts, dirty state, scheduling, execution counters | selector AST inspection, specificity, cascade ordering, inheritance, property meaning |
| Layout | computed-style consumption, used values, box/layout structures, geometry | declarations, selector matching, cascade winners |
| Paint/GFX | visual structures, paint ordering, primitives, backend output | selector matching, cascade ordering, property parsing |
| Tests/tooling | fixture loading, semantic assertions, snapshot comparison, fuzz replay | alternate engine behavior or fixture-driven production decisions |

## Executable supported behavior

### Selector parsing and matching

The author-selector subset includes:

- selector lists;
- universal and named type selectors;
- ID and class selectors;
- unqualified attribute existence selectors;
- unqualified attribute exact, includes, dash-match, prefix, suffix, and
  substring value selectors;
- compound selectors;
- descendant, child, adjacent-sibling, and subsequent-sibling combinators;
- the static tree-structural pseudo-classes `:root`, `:empty`,
  `:first-child`, `:last-child`, and `:only-child`.

Matching consumes neutral DOM facts through `SelectorMatchDom` and the
production `SelectorDomIndex`. It uses the parser-selected document mode,
preserves HTML/SVG/MathML namespace boundaries, excludes template-associated
fragment descendants from ordinary child axes, skips non-elements for sibling
axes, and uses exact direct text facts for `:empty`.

HTML host-language comparison is CSS-owned. Supported HTML type and
unqualified attribute names use selector-side ASCII normalization; foreign
names remain exact. Quirks-mode ID and class value comparison and the supported
HTML attribute-value comparison inventory use the AF4c policy. Selector IDs
remain CSS-local projection identities and are not DOM patch or retained
render identities.

### Specificity

`Specificity` is the sole typed selector specificity model. It is an A/B/C
tuple with lexicographic ordering and saturating component arithmetic. Type
selectors contribute C, ID selectors contribute A, class/attribute/supported
pseudo-class selectors contribute B, and combinators/universal selectors
contribute zero.

Selector-list matching retains the specificity of each actual match. The
effective rule specificity used by cascade is the highest specificity among
selectors that matched the target element, never an unmatched selector's
specificity. Inline style is a distinct element-attachment precedence step,
not a fabricated selector specificity.

### Rule collection and cascade

AF5 builds one fallible immutable `RuleCollection` per style execution.
Collection preserves opaque source identity, sparse stylesheet order, raw rule
position, style-rule position, declaration order, condition state, selector
parse state, and declaration applicability. Stylesheet declaration
classification occurs once and matched rule inputs borrow the classified
collection.

The executable current cascade orders supported declarations by:

1. origin and importance band;
2. element attachment versus style-rule attachment where applicable;
3. actual matched selector specificity for style rules;
4. stylesheet and style-rule source order;
5. declaration and shorthand-expansion order.

Supported runtime inputs include the Browser's current UA stylesheet, author
stylesheets, and element inline styles. User-origin ordering is typed and
semantically tested, but Browser has no runtime user-stylesheet manager.

Invalid values, invalid shorthands, unsupported properties, custom
properties, invalid property names, invalid selectors, unsupported selectors,
and inactive/deferred rules do not become candidates. Exact candidate identity
reuse, inconsistent source data, or a complete-priority collision is an
invariant failure; there is no stable-sort or incoming-order tie fallback.

### Inheritance, defaulting, and CSS-wide keywords

AF7 turns sparse winners into one total `ResolvedStyle` source per AD4
longhand. A property receives exactly one of:

- an authored winner;
- symbolic default inheritance from an immediate parent;
- a registry-derived initial source;
- a supported CSS-wide inherited or initial source.

`initial`, `inherit`, and `unset` use the shared AD3 path. At the root,
inheritance falls back to the property initial value. `unset` selects inherited
or initial behavior from registry metadata. `revert` and `revert-layer` are
recognized but unsupported and remain non-candidates.

The resolved layer records source provenance. It does not copy a parent
resolved value. Top-down AF8 materialization obtains inherited values from the
immediate parent's `ComputedStyle`, so descendants inherit normalized computed
values rather than authored text.

### Computed style and downstream authority

`ComputedDocumentStyle` is the authoritative CSS-owned per-element computed
artifact. Canonical initial construction is total and registry-derived.
Supported specified values normalize through typed `ComputedValue` variants;
percentages that need a layout basis remain typed computed inputs for Layout.

`build_style_tree_from_computed_styles` validates selector identity, element
name, namespace, document order, and matching environment before building the
borrow-backed `StyledNode` view. The Browser production style phase retains
resolved/computed artifacts and rebuilds `StylePhaseOutput` without writing
legacy declaration vectors into parser-created DOM nodes.

Layout consumes `StylePhaseOutput` and owns used-value resolution and geometry.
Paint consumes Layout output and visual computed values. Neither subsystem
reads declarations, recalculates specificity, selects cascade winners, or
suppresses elements by reinterpreting HTML tag names.

### Selector/cascade invalidation dependencies

AF9 extracts dependencies only from AF5 active rules that contain at least one
supported declaration candidate. The owned artifact records keyed type, ID,
class, attribute predicate, supported structural pseudo, and relationship
effects with composed paths to the rightmost subject.

Browser retains this opaque artifact under a generic lifecycle key containing
the document identity domain and the existing stylesheet generation. CSS
separately validates the retained artifact against the current
`SelectorMatchingEnvironment`. Document identity replacement, stylesheet
generation change, or CSS-detected matching-environment incompatibility makes
the artifact ineligible. Browser does not encode CSS selector-environment
semantics into `PageStyleDependencyKey`. Browser supplies bounded
committed-before/final attribute and text facts to CSS and consumes only the
resulting opaque plan. Relevant supported attribute and `:empty` transitions
may authorize suffix recomputation; irrelevant transitions may avoid Style
work; structural mutations remain full Style recomputation.

## Modeled or reserved concepts

The following types preserve future ordering or integration positions but do
not mean executable runtime support exists:

- user-origin cascade ordering is modeled and tested, but Browser has no user
  stylesheet manager;
- animation and transition origin bands reserve their normative relative
  positions, but no current declaration source emits either band;
- selector namespace constraints support internal UA matching policy, but
  author namespace selector syntax remains unsupported;
- retained computed artifacts and dependency paths are extension boundaries
  for later style sharing and targeted restyle, not implementations of those
  systems.

Closeout documentation and debug output must not describe a modeled or
reserved concept as executable support.

## Intentionally conservative behavior

- Structural DOM mutations require full Style recomputation in AF9.
- Unsupported or unavailable dependency metadata widens work instead of
  preserving potentially stale style.
- Selector/cascade invariant and resource failures propagate through
  resolved, computed, and Browser style paths.
- Non-empty stylesheet `media` attributes and unsupported conditional rules
  fail closed.
- Incremental suffix eligibility can fall back to full recomputation when
  retained artifacts or identity validation are unavailable.
- AD7 impact classification occurs only after new computed style exists;
  conservative property impacts may cause broader Layout/Paint work.

These behaviors are safety policies, not claims of advanced invalidation or
media-query support.

## Unsupported and deferred behavior

The following remain unsupported or incomplete after Milestone AF:

- broad selector coverage, including `:has()`, `:nth-child()`, dynamic and
  functional pseudo-classes beyond the documented static subset;
- pseudo-elements, including legacy single-colon spellings as executable
  pseudo-elements;
- author namespace selector syntax, attribute-selector `i`/`s` modifiers,
  standards-complete selector escape decoding, and XML-document matching;
- media queries, container queries, and complete conditional-rule evaluation;
- custom properties, `var()`, environment substitution, and invalid-at-
  computed-value-time behavior;
- cascade layers, layer ordering, rollback semantics, `revert`, and
  `revert-layer`;
- CSS `@scope`, scope roots/limits, scope proximity, scoped styles, Shadow DOM,
  and encapsulation ordering;
- animations and transitions as declaration sources or runtime systems;
- presentational hints and runtime user stylesheet management;
- general style sharing, selector caches, bloom filters, per-node reverse
  dependency graphs, and dependency-directed structural restyle;
- complete targeted Layout/Paint execution;
- CSSOM, stylesheet mutation APIs, `getComputedStyle()`, and JavaScript-facing
  style APIs;
- broad CSS property, shorthand, value/unit, color, typography, layout, and
  paint coverage;
- broad CSS WPT integration or a claim of web-platform conformance.

Unsupported selector features remain typed unsupported parse results and do
not match. Invalid selectors remain typed invalid results. Deferred at-rule
contents such as `@media`, `@layer`, and `@scope` are skipped and never
flattened into the active cascade. Unsupported/custom/invalid declarations
remain diagnostic non-candidates.

## Deterministic evidence inventory

The fixture and evidence index lives at
`crates/css/tests/fixtures/README.md`. The following bands directly prove the
AF10 requirements:

| Behavior | Direct semantic evidence | Deterministic projection |
| --- | --- | --- |
| selector parsing and AST | selector parser unit tests and `selector_golden.rs` | selector list/parse snapshots include typed nodes, spans, and specificity |
| selector matching | matching unit tests, `af4_parser_conformance.rs`, and Browser materialization parity | selector-matching and document-selector-matching snapshots |
| specificity | typed component, saturation, comparison, parser-derived, pseudo, and matched-only assertions | selector, matching, rule-input, candidate, and winner snapshots |
| rule collection | collection identity/order, active/inactive, declaration classification, and limit tests | bounded AF5 rule-collection diagnostic |
| cascade winners | origin/importance, attachment, specificity, source/declaration order, filtering, and invariant tests | exact internal R8/AF6 snapshots plus bounded AF6 production diagnostic |
| inheritance/defaulting | resolved-style and document-resolution semantic tests | resolved-document and version 5 document-resolution snapshots |
| CSS-wide keywords | specified parser, shorthand, resolved source, root, and computed materialization tests | resolved source and computed document snapshots |
| computed style | normalization, total builder, document, reuse, fallibility, and style-tree tests | computed value, `ComputedStyle`, `ComputedDocumentStyle`, and `StylePhaseOutput` snapshots |
| representative documents | four `representative_pages` cases using `html::parse_document` | per-page computed-style snapshots |
| invalidation dependencies | AF9 extraction, matching, classification, limit, and Browser integration tests | bounded versioned dependency and invalidation-decision snapshots |
| Layout/Paint authority | Browser phase-boundary, UA override, layout input, and paint input tests | style/layout/paint phase snapshots |

Synthetic DOM and rule-input tests remain appropriate for isolated invariants,
representation exhaustion, malformed historical fixtures, and exact priority
matrices. They do not substitute for the parser-backed AF4, representative-
page, Browser materialization, and render-phase integration bands.

## Debug and regression surfaces

The supported debug family remains phase-owned rather than wrapped in a new
AF10 aggregate schema:

- selector list and selector parse snapshots;
- selector matching and integrated document-selector diagnostics;
- AF5 rule-collection diagnostics;
- AF6 cascade candidate/winner diagnostics;
- resolved-style and document-resolution snapshots;
- computed value, style, document, and style-phase snapshots;
- AF9 dependency and invalidation-decision snapshots;
- Browser retained-state and render phase-boundary snapshots;
- Layout and semantic Paint snapshots.

Derived Rust `Debug` is not a stable semantic serializer. Exact test-only
snapshots and bounded production diagnostics have different roles and must not
be conflated. AF4e, AF5, AF6, and AF9 diagnostic record/storage/serialization
limits remain in force. Debug tooling cannot weaken production resource limits
or convert a typed failure into partial success.

Snapshots are regression records, not correctness oracles. Semantic
assertions and owning contracts determine expected behavior. A golden change
requires an independently understood, in-scope behavior or schema change;
AF10 introduces neither.

## Future extension points

Future work must extend the existing CSS-owned pipeline rather than add
parallel paths:

### Media queries

Add typed media/container condition models, evaluation environments,
stylesheet filtering, viewport/input dependency facts, and invalidation before
activating deferred contents. Browser supplies environment facts; CSS owns
query meaning.

### Pseudo-elements

Extend selector IR, matching results, generated-box/style representation,
specificity, cascade provenance, invalidation, Layout box generation, and Paint
ordering together. Do not model pseudo-elements as ordinary parser-created DOM
elements.

### Custom properties

Add custom declaration storage, inheritance, token preservation,
substitution/dependency graphs, cycles, fallback, and invalid-at-computed-
value-time behavior before `var()` participates in supported longhands.

### Cascade layers

Add layer statement/block parsing, layer ordering at the normative cascade
position, retained lower-priority candidate history, rollback behavior, and
bounded diagnostics. Do not flatten `@layer` contents before layer semantics
exist.

### Scoped styles

Add CSS `@scope` root/limit matching and scope proximity independently from the
historical HTML `scoped` attribute. Shadow DOM and encapsulation require their
own tree-scope and ordering contracts.

### Style sharing

Build sharing keys from CSS-owned computed-style dependencies, matching
environment, parent computed inputs, and retained identity compatibility.
Browser may retain shared artifacts but must not derive property or selector
meaning.

### Advanced selector invalidation

Extend AF9 with stable identity-bound entries, transactional structural deltas,
per-node reverse dependency information, and dependency-directed self,
descendant, child, sibling, root, empty, and child-order execution. Metadata
failure must continue to widen work.

### WPT integration

Add focused adapters only when the relevant supported surface and expectation
mapping are explicit. Keep external fixture provenance, deterministic skips,
and supported-subset reporting separate from claims of broad CSS conformance.

## Closeout invariants

- CSS owns selector parsing and matching, specificity, cascade, inheritance,
  CSS-wide keywords, computed values, and style dependency meaning.
- HTML owns parser-created DOM and document-mode selection.
- Browser consumes opaque CSS plans and artifacts while owning lifecycle and
  scheduling.
- Layout consumes computed values and owns used geometry.
- Paint consumes Layout/visual data and owns paint output.
- Production and conformance tests use the same typed implementation paths.
- Invalid and unsupported selector states never partially match.
- Effective specificity comes only from actual matches.
- Candidate priority is explicit; exact ties are invariant failures.
- Every supported longhand resolves exactly once from registry-backed data.
- Inheritance consumes the immediate parent's computed value.
- Unsupported and inactive inputs never become active candidates or
  invalidation dependencies.
- Resource or metadata failure never authorizes stale style reuse.
- Debug output is deterministic, phase-owned, and bounded where exposed for
  production triage.
- Milestone AF completion means foundation completion for the documented
  subset only.

## Deliberate exclusions

AF10 deliberately excludes:

- new Rust production types or public APIs;
- new selector, cascade, property, computed-value, or invalidation behavior;
- a new aggregate AF10 serializer, diagnostic envelope, snapshot format, or
  fixture harness;
- Browser/runtime, Layout, Paint, GFX, or platform behavior changes;
- snapshot regeneration without a separately justified behavior change;
- WPT import or adaptation;
- opportunistic removal of legacy compatibility APIs;
- rewriting historical contracts beyond explicit reconciliation notes.

## Milestone decision

The AF1-AF9 production foundation plus the deterministic evidence inventoried
above satisfies Milestone AF's exit criteria. Future CSS work can extend typed
selector IR, cascade inputs, defaulting/computed stages, and CSS-owned
dependency artifacts without introducing ad hoc selector/cascade paths.

Milestone AF is closeable as **CSS selectors, specificity, cascade, and
computed-style foundation complete for the documented supported subset**. It
is not closeable under, and this document does not make, a claim of complete
CSS selector, cascade, property, media-query, custom-property, animation,
CSSOM, JavaScript API, or web-platform conformance.
