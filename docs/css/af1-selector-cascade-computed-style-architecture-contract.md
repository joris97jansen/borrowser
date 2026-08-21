# AF1: Selector, Cascade, And Computed-Style Architecture Contract

Status: implemented architecture and ownership contract for Milestone AF issue 1

Last updated: 2026-08-17

AF1 establishes the CSS-owned boundary for selector parsing and matching,
specificity, cascade, inheritance, computed-style construction, and style-input
invalidation planning. It connects the AD value/property foundations and the AE
parser-created DOM contract to the retained rendering runtime without moving
CSS semantics into Browser/runtime, Layout, or Paint.

AF1 is intentionally a contract issue. It formalizes the current supported
subset and its conservative invalidation proof; it does not add selector
coverage or implement the future selector dependency graph.

AF5 now supplies the concrete pass-scoped stylesheet collection boundary
required by this architecture. Currently supported discovered and available
Browser stylesheet input enters CSS through opaque source identity plus sparse
order, and declaration classification occurs once per style execution rather
than once per matched element. Semantic cascade contracts remain below the
opaque collection integration. See
`docs/css/af5-stylesheet-rule-collection-source-order-contract.md`.

## Related code

- `crates/css/src/style_invalidation.rs`
- `crates/css/src/cascade`
- `crates/css/src/selectors`
- `crates/css/src/specified`
- `crates/css/src/computed`
- `crates/browser/src/page/dom_mutation.rs`
- `crates/browser/src/page/stylesheets.rs`
- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/page/style_cache.rs`
- `crates/browser/src/page/style_phase.rs`
- `crates/browser/src/document_style.rs`
- `crates/browser/src/rendering/lifecycle.rs`

## Related contracts

AD and value foundations:

- AD1: `docs/css/ad1-css-value-property-ownership-architecture.md`
- AD3: `docs/css/ad3-css-wide-keyword-handling.md`
- AD5: `docs/css/ad5-specified-computed-value-boundaries.md`
- AD6: `docs/css/ad6-shorthand-expansion-foundation.md`
- AD7: `docs/css/ad7-css-owned-invalidation-impact-classification.md`
- AD10: `docs/css/ad10-css-value-property-foundation-closeout.md`

Selector, cascade, and computed-style foundations:

- Q1/Q8: `docs/css/q1-selector-matching-architecture.md`,
  `docs/css/q8-selector-matching-invariants-extension-hooks.md`
- AF4b: `docs/css/af4b-selector-dom-query-contract.md`
- AF4e: `docs/css/af4e-selector-invalidation-parser-conformance-closeout.md`
- R1-R9: the structured cascade and resolved-style contracts under
  `docs/css/r*.md`
- S1/S6/S9: the computed-style property, assembly, and runtime contracts

Runtime and rendering:

- U1/U8: the CSS runtime integration contracts
- AC1, AC5, and AC10: retained rendering state, retained style artifacts, and
  retained-rendering closeout contracts
- AE1, AE2, and AE14: parser-created DOM ownership and parser-foundation
  closeout contracts
- `docs/architecture/ARCHITECTURE.md`
- `docs/engine-feature-gap-tracker.md`

## Ownership matrix

| subsystem | owns | may consume | must not own |
| --- | --- | --- | --- |
| CSS | selector syntax and AST, parsing, matching, specificity, rule/declaration applicability, cascade ordering, declaration precedence, inheritance/defaulting, CSS-wide keyword resolution, specified/computed construction, style-input invalidation dependencies and plans | parser-created DOM relationships and attributes, stylesheet inputs, AD property/value registries, retained artifacts supplied for CSS execution | layout geometry, paint ordering, retained artifact lifetime, render scheduling |
| HTML / Browser mutation path | DOM construction and mutation facts, parser-created node identities, stylesheet/resource observation | CSS fact classifier and plan results | selector meaning, selector safety proofs, cascade ordering, specificity, property meaning |
| Browser/runtime | retained plan lifetime, plan merge delegation, artifact key/identity validation, scheduling, dirty-state projection, recomputation orchestration, actual execution counters and debug envelope | opaque `StyleInvalidationPlan`, CSS execution results, computed-style artifacts and AD7 impact facts | constructing or combining CSS plans, inspecting selector ASTs, selector feature flags, selector kinds, or CSS scope variants |
| Layout | layout structures, used-value resolution, geometry, formatting, layout artifacts | computed layout-relevant values | declarations, cascade winners, specified values, selector semantics |
| Paint | visual structures and paint output, future actual/device-facing processing | computed paint-relevant values and layout output | declarations, cascade winners, selector semantics, property registry meaning |

The authoritative CSS-to-runtime style input is the CSS-owned plan. Browser
retains it directly as `Option<css::StyleInvalidationPlan>` and forwards it to
CSS execution APIs. `None` is the sole representation of no style
invalidation.

Selector diagnostics remain CSS-owned semantic results. `css::selectors`
normalizes invalid, unsupported, invariant, and resource-limit outcomes with
typed details and severity. `css::model` mechanically projects those outcomes
into the shared `SyntaxDiagnostic` transport while preserving syntax
diagnostic encounter order and appending selector diagnostics in model
style-rule traversal order. Browser/runtime does not interpret selector
diagnostic classes or features.

## AD and AE integration

AF consumes AD's registered property metadata, typed value model, shorthand
expansion, CSS-wide keyword handling, specified/computed boundary, and AD7
computed-style impact classification. AF does not create a second property
registry or reinterpret AD7's downstream Layout/Paint impact flags.

AF consumes the parser-created DOM produced under AE. CSS sees stable element
identities, names, attributes, parent/child relationships, and sibling
relationships through the DOM-facing selector index. CSS does not consume
tokenizer state, tree-builder insertion modes, parser recovery diagnostics, or
HTML parsing internals.

AF4b makes that handoff a fallible, neutral query projection. HTML supplies
canonical namespace/local-name, ordered qualified attributes, exact ordinary
text, ordinary child storage, source identity, and typed template-fragment
association. CSS constructs explicit document or declared element-subtree
projections, records actual document-element identity, indexes previous and next
element siblings, and rejects nested documents or ambiguous direct document
elements. CSS owns attribute-name/value selector policy, ID/class semantics,
and every future structural pseudo meaning. Browser/runtime does not construct
a competing selector provider.

`SelectorDomElementId` remains a CSS-local projection identity. Source DOM IDs
are used only through an explicit integration mapping and never become patch,
retained-render, or selector identity by reuse.

## Selector and declaration terminology

These are separate events and must remain separate in APIs and diagnostics:

1. A selector parses successfully into the supported CSS-owned selector AST.
2. Selector matching evaluates that AST against one parser-created DOM
   element.
3. A rule is matched when its supported selector produces a match for an
   element. A rule can still contain declarations that are not applicable.
4. A declaration is applicable only if the current CSS model accepts its
   property/value, shorthand expansion, CSS-wide keyword, conditional-rule
   status, and other validation/filtering requirements. Invalid, unsupported,
   or unsupported-conditional declarations do not become cascade candidates.
5. An applicable declaration contributes a declared value to the rule's
   candidate set. Declared values are authored, property-aware inputs; they
   are not yet cascaded or computed values.
6. Cascade ordering compares applicable candidates by the modeled origin and
   importance band, specificity, and deterministic source/declaration order.
   The winning candidate produces the cascaded value for that property.
7. The cascaded value, property metadata, and parent computed values where
   inheritance/defaulting applies determine the specified value.
8. The specified value is normalized into the computed value through the AD
   property/value registry and computed-style construction path.

Full conditional-rule and media-query evaluation remains outside AF1 unless a
current narrow path already supplies the condition result. AF1 must not imply
that every declaration in a matched rule automatically participates in the
cascade.

## CSS value lifecycle

AF uses the standard lifecycle below:

```text
selector parse/match
  -> matched/applicable rules and declarations
  -> declared values
  -> cascaded values
  -> specified values
  -> computed values
  -> future used values
  -> future actual values
```

Inheritance is not an independent sequential value stage after specified
values. During CSS cascade/defaulting, a property that inherits can use the
parent element's computed value to determine the child's specified value when
the winning declaration is `inherit`, `unset` under inherited-by-default
behavior, or when ordinary inherited defaulting supplies no child declaration.
The resulting specified value is then converted to the child's computed
value. At the root, inherited behavior falls back to the property's initial
value according to the AD3/R5 contracts.

The future used-value stage belongs to Layout: containing blocks, formatting
contexts, intrinsic sizing, and layout constraints are applied there. The
future actual-value stage belongs to the later layout/paint/backend pipeline:
device, rasterization, compatibility, and backend constraints may affect it.

`ResolvedStyle` and `ResolvedDocumentStyle` are Borrowser's internal
CSS-owned cascade/computed-style handoff artifacts. They are not CSSOM
"resolved values". In CSSOM, a resolved value is an API-facing compatibility
concept used by APIs such as `getComputedStyle()` and can correspond to a
computed or used value depending on the property and API rules. AF1 does not
add CSSOM, `getComputedStyle()`, or an internal resolved-value stage.

## Style origins and precedence

The CSS type system models these origin/importance bands:

- `CascadeOrigin::UserAgent`
- `CascadeOrigin::User`
- `CascadeOrigin::Author`
- normal and important bands for each modeled origin
- inline author style as an author-origin inline precedence input
- reserved animation and transition precedence levels in the broader cascade
  model; they are not emitted by the current runtime

The normal Browser runtime currently supplies:

- `MINIMAL_UA_STYLESHEET` as `CascadeOrigin::UserAgent`;
- document inline `<style>` and external stylesheets as `CascadeOrigin::Author`;
- inline `style` attribute declarations as author-origin inline style;
- no normal user-origin stylesheet source.

`CascadeOrigin::User` is modeled and directly testable in CSS cascade
contracts, but AF1 does not implement a user stylesheet system. Animation and
transition bands are reserved type-system space, not currently emitted
origins. The legacy element/default display behavior in compatibility style
paths is not an authoritative additional structured cascade origin; the
structured runtime UA sheet and CSS initial/default handling remain the source
of truth for the AF path.

AF1 does not add cascade layers, user stylesheets, a replacement UA stylesheet
system, animations, or transitions.

## Style-input invalidation boundary

AF1 separates three decisions:

### A. Mutation facts — HTML / Browser

Browser reports one composable neutral fact set per DOM publication. It can
simultaneously preserve document replacement, allocation, topology/order,
template association, attribute targets, text targets, and an unclassified
future-patch count. Attribute and text targets retain canonical surviving DOM
identities plus valid historical-target counts. Browser owns observation and
identity mapping; it does not decide what those facts mean for selectors.

### B. Semantic style invalidation plan — CSS

`classify_style_invalidation(&StyleChangeFacts)` returns
`Option<StyleInvalidationPlan>`. The plan is opaque outside CSS and has no
public constructors or public variants. CSS owns canonicalization, semantic
combination, and future extension of its representation.

Callers cannot mutate fact fields directly. They use CSS-owned
`ChangedStyleNodeFacts` constructors, `DomStyleChangeFactsBuilder`, and
`StyleChangeFacts::dom_publication`, which prevent contradictory occurrence
and identity state and canonicalize identities deterministically.

The current conservative outcomes are:

- `None`: CSS proved that no style recomputation is required for the fact;
- an opaque document-order suffix plan for non-empty, materialized attribute
  identities;
- an opaque full-document plan for document replacement, tree changes,
  stylesheet-set changes, text changes under AF4e, unclassified patches, and
  unprovable attribute changes.

Ordinary text changes currently receive full invalidation because exact text
can change `:empty` matching and no dependency index can prove a narrower
scope. Text inside `<style>` additionally participates in stylesheet
reconciliation, which submits `StylesheetSetChanged` when active CSS input
changes.

`merge_style_invalidation_plans(existing, incoming)` is CSS-owned. It preserves
an existing plan when the incoming result is `None`, canonicalizes and
deduplicates suffix identities, and makes full invalidation dominate. Browser
stores the returned option but never reconstructs these rules.

### C. Actual recomputation execution — Browser lifecycle plus CSS execution

The DOM execution path is:

```text
DomMutationFacts
  +-> one aggregate StyleChangeFacts::DomPublication classification
  |     -> apply at most one StyleInvalidationPlan
  |     -> at most one AppliedCssStyleInvalidation capability
  |     -> one DomPublicationStyleInvalidated request
  +-> independent Browser intrinsic mutation requests
```

`AppliedCssStyleInvalidation` is non-`Copy`, non-`Clone`, non-`Default`, and
not publicly constructible. It proves CSS classification and retained-plan
application have occurred. The rendering factory consumes it to authorize one
direct Style request. Intrinsic attribute, text, structure, and unknown
requests never acquire CSS Style work, so a mixed publication preserves every
cause without attributing one aggregate CSS decision to each dimension.

The retained execution path then applies the pending CSS plan through cache,
key, and identity feasibility checks and records `StyleRecalcKind` plus the
retained artifact action.

A suffix plan means CSS permits a suffix reuse attempt; it does not promise
that the runtime has a usable previous cache. Missing cache, incompatible
generation/key, identity mismatch, or CSS incremental validation failure
produces a safe full recomputation. `StyleRecalcKind` and
`RetainedStyleArtifactAction` record what actually executed, including a full
fallback after CSS permitted incremental reuse but retained execution could
not produce an incremental result. When no previous cache exists, the
incremental algorithm is not invoked; the result still records incremental
eligibility followed by a full fallback.

Adding a future selector such as `:has()` changes CSS-side selector dependency
and invalidation proof code. It can widen the opaque plan to a full or richer
CSS-owned scope without adding selector-specific branches, flags, AST reads,
or merge rules to Browser/runtime.

This first AF1 boundary is not a complete dependency graph. A later AF issue
should replace the conservative suffix/full proof with selector-aware
dependency extraction and richer CSS-owned invalidation planning when the
supported selector set requires it.

## Computed-style consumers

CSS constructs the authoritative `ResolvedDocumentStyle` and
`ComputedDocumentStyle` artifacts. Browser/runtime may retain their lifecycle
and consume CSS-owned AD7 impact facts, but does not parse declarations or
recompute property meaning. Layout consumes computed layout-relevant values;
Paint consumes computed paint-relevant values together with layout output.
Neither downstream subsystem consumes raw declarations or cascade winners.

## Debug and test contract

Selector parse/match, specificity, cascade, inheritance, computed style,
unsupported selector behavior, CSS plan classification/merge, and actual
retained execution are deterministic internal regression surfaces. AF4e adds
a bounded, versioned CSS-owned integrated matching report above the core
matcher. It covers unmatched, invalid, unsupported, and declaration-free rules
without depending on cascade winners, and serializes typed terminal setup or
limit failures. Browser exposes that bounded authoritative surface without
interpreting it. CSS also owns the stable plan debug projection; Browser does
not define a second Full/Suffix semantic enum.

Typed APIs and Rust visibility are the primary architecture enforcement:

- Browser constructs `StyleChangeFacts`, not plans;
- `StyleInvalidationPlan` has private representation and no public semantic
  constructors;
- `None` is the only no-invalidation state;
- only `Some(plan)` authorizes Browser to advance the retained style-input
  generation and schedule new Style work for that fact;
- CSS owns plan merging and canonicalization;
- CSS execution returns an invariant-safe result distinguishing
  full-required, incremental-unavailable, and incremental-computed outcomes;
- Browser records actual `StyleRecalcKind` and artifact actions.

## Deliberate non-goals

AF4a matching-environment refinement: authoritative selector matching receives
the CSS-owned immutable `SelectorMatchingEnvironment`, currently containing
only parser-selected HTML `DocumentMode`. It has no default or Browser
identity. Cascade, computed style, incremental reuse, and matching debug entry
points preserve it explicitly, and resolved/computed artifacts bind to the
environment that produced them.

AF1 does not implement:

- full selector coverage, new selector functionality, `:has()`, dynamic
  pseudo-classes, or pseudo-elements;
- a complete selector dependency graph, fine-grained subtree invalidation,
  selector bloom filters, style sharing caches, or broad invalidation taxonomy;
- full media-query or conditional-rule evaluation;
- cascade layers or a user stylesheet system;
- custom properties or variables;
- animations or transitions;
- CSSOM, `getComputedStyle()`, or JavaScript-facing style mutation;
- broad property expansion;
- unrelated Layout or Paint algorithms.

AF4e refinement: AF1's original broad pseudo-class non-goal is narrowed by the
typed tree-structural subset documented in
`af4d-tree-structural-pseudo-class-matching.md`. Dynamic and functional
pseudos, pseudo-elements, dependency indexing, and fine-grained invalidation
remain unsupported. Text changes conservatively receive a full CSS plan while
retaining their independent direct Layout consequence.
