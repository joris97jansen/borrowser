# AF4e: Selector Invalidation, Parser Conformance, And AF4 Closeout

Status: implemented AF4e contract and AF4 parent closeout evidence

Last updated: 2026-08-18

AF4e integrates the AF4 selector semantics with retained rendering and proves
the supported matcher against parser-created and Browser-materialized DOM.
This contract is the final child contract for **AF4 — Implement selector
matching against parser-created DOM elements**. It does not depend on future
cascade-winner work.

## Ownership

- HTML owns parsing, `DocumentMode`, `DomPatch`, patch-key lifetime, and the
  parser-created DOM.
- Browser/DOM owns atomic patch publication, neutral mutation facts, neutral
  identity resolution, retained-artifact lifetime, and intrinsic rendering
  scheduling.
- CSS owns selector parsing, the selector-DOM projection, host-language
  comparison, matching, specificity, selector diagnostics, and classification
  of neutral DOM facts into an opaque `StyleInvalidationPlan`.
- Layout owns text/geometry consequences and consumes computed style. Paint
  consumes layout/visual data. Neither interprets selectors or mutation facts.

## One publication, composable neutral facts

`PendingDomMutationFacts` scans the complete patch batch without choosing a
dominant mutation. After staged patch application, Browser resolves target
keys and constructs one `DomMutationFacts` containing simultaneous facts for:

- document replacement;
- ordinary node allocation;
- tree topology or order operations;
- template-contents association;
- changed attribute targets;
- changed text targets;
- future patch variants not classified by this Browser build.

Attribute and text dimensions each distinguish whether the mutation occurred,
surviving canonical DOM identities, and a count of valid historical targets.
Topology deliberately remains a coarse boolean in AF4e. Structural node
identity and dependency graphs are an optimization boundary, not a missing
correctness condition.

The known patch mapping is exact:

| `DomPatch` operation | neutral fact |
| --- | --- |
| `Clear`, `CreateDocument` | document replacement |
| `CreateDocumentType`, `CreateElement`, `CreateText`, `CreateComment`, `CreateProcessingInstruction` | ordinary node allocation |
| `AppendChild`, `InsertBefore`, `RemoveNode` | tree topology/order operation |
| `CreateTemplateContents` | template-contents association |
| `SetAttributes` | attribute mutation target |
| `SetText`, `AppendText` | text mutation target |
| future unknown variant | unclassified patch count |

Allocation alone is not called a topology operation. An unknown variant is not
misreported as structure, attributes, or text. If `DomStore` accepts such a
future patch before Browser learns its meaning, CSS and Browser independently
choose conservative fallbacks and stable mutation diagnostics expose the
unclassified count.

A handle change participates only while Browser constructs the neutral fact;
the committed `DomMutationFacts` is authoritative downstream. In particular,
a valid same-handle publication beginning with `Clear` is a document
replacement: the fact drives CSS classification, the intrinsic
`DocumentReplaced` request, and the retained identity-domain reset even though
the handle value did not change.

## Transactional identity resolution

Browser preserves the publication transaction in this order:

```text
clone candidate DomStore
  -> apply complete patch batch
  -> materialize candidate DOM
  -> resolve attribute/text target keys in the staged post-apply store
  -> construct complete DomMutationFacts
  -> commit store, handle/version, PageState, retained facts and render work
```

`DomStore::resolve_mutation_node_ids` owns the patch-key-to-materialized-ID
bridge. Callers cannot infer numeric equivalence. Its typed states are:

```text
allocated and live       -> surviving materialized DOM identity
allocated and not live   -> valid historical/transient target
not allocated            -> DomIdentityResolutionError::NeverAllocated
live without an identity -> DomIdentityResolutionError::LiveIdentityUnavailable
```

Duplicate and repeated targets canonicalize deterministically. A target changed
and then removed, including one inside a removed subtree, is valid history and
does not make an otherwise valid publication fail. A never-allocated target or
an impossible live materialization is a typed pre-commit failure. Therefore a
failure cannot partially change `DomStore`, handle/version, `PageState`,
retained artifacts, retained mutation facts, or pending rendering work.

## CSS classification and rendering composition

Browser converts neutral facts only through CSS-owned invariant-safe
constructors: `ChangedStyleNodeFacts`, `DomStyleChangeFactsBuilder`, and
`StyleChangeFacts::dom_publication`. Fields remain private; changed identities
are sorted and deduplicated at the CSS boundary, and an empty identity set can
still truthfully represent a mutation whose targets are all historical.

The publication fans out once:

```text
DomMutationFacts
  +-> CSS aggregate classification
  |     -> apply at most one StyleInvalidationPlan
  |     -> advance style-input generation at most once
  |     -> consume one AppliedCssStyleInvalidation capability
  |     -> one DomPublicationStyleInvalidated request
  |
  +-> Browser intrinsic classification
        -> independent document/structure/attribute/text/unknown requests
```

`AppliedCssStyleInvalidation` is an invariant-carrying, non-`Copy`,
non-`Clone`, non-`Default`, privately constructed capability. It exists only
after CSS classification and retained-plan application. The base
`DomPublicationStyleInvalidated` contract contains no work; only the factory
that consumes this capability produces direct Style work. Its Style reason is
`StyleInputChanged`, without attributing CSS's aggregate decision to text,
attributes, or structure.

Intrinsic requests never authorize Style. In particular, a mixed attribute
and text publication produces one CSS-authorized Style request while the text
request independently carries direct Layout work. CSS authorization is not
copied onto either mutation dimension.

`DomPublicationRenderInvalidation` preserves this distinction structurally as
an ordered intrinsic-request collection plus one optional CSS Style request.
The closed `IntrinsicRenderInvalidationSource` domain cannot express either
`DomPublicationStyleInvalidated` or `StylesheetSetChanged`; only the
capability-consuming CSS factory can produce the former. Queuing iterates
intrinsic requests first and then the optional CSS request, while requesting
redraw only once for the publication.

The rendering vocabulary is:

| entry point | requester | Style | Layout | Paint | frame |
| --- | --- | --- | --- | --- | --- |
| `DomPublicationStyleInvalidated` | `CssEngine` | direct `DomPublicationStyleInvalidated` | cascaded from Style | cascaded from Layout | cascaded from Style |
| `DomMutationUnclassified` | `BrowserRuntime` | none | direct `DomMutationUnclassified` / `ConservativeUnknownImpact`, document scope | cascaded from Layout | direct `DomMutationUnclassified` |

The first is one validated engine-owned source entering the rendering
pipeline; it does not name a DOM mutation cause. The second is a truthful
conservative intrinsic cause and never masquerades as `DomStructureChanged`.
Viewport, resource, and input-state contracts are unchanged.

## Conservative selector invalidation

CSS classifies the aggregate fact set once. Document replacement, topology,
text, or unclassified facts currently produce full-document Style
invalidation. A surviving attribute identity can retain the established
document-order suffix plan; an attribute mutation with no surviving identity
falls back to full-document. Allocation or template association alone is
style-neutral because it does not alter the published selector-visible tree.

Text is full-document because `:empty` depends on ordinary direct text and no
reverse selector-dependency index exists. The scope is conservatively broad
but semantically correct. The neutral text identities remain available for a
future targeted CSS classifier; Browser does not inspect stylesheets or test
for `:empty`.

## Parser-backed conformance

`crates/css/tests/af4_parser_conformance.rs` parses actual HTML and matches
against the returned `ParseOutput.document`. Mode-sensitive cases use real
DOCTYPE inputs, assert the returned `ParseOutput.document_mode`, and construct
the matching environment from that exact mode.

The matrix covers universal, type, ID, class, every supported attribute
operator, compounds, lists, descendant/child/adjacent/general-sibling
combinators, all parser-selected modes, HTML name/value policy, HTML/SVG/MathML
boundaries, adjusted SVG `foreignObject`, intervening text/comment/processing
instruction nodes, template boundaries, all five static pseudo-classes,
unsupported dynamic pseudos and pseudo-elements, malformed selectors, parser
limits, and matcher-limit failures against valid parser-created DOM.

### Synthetic invariant/error fixtures

Separate synthetic fixtures prove typed terminal selector-DOM/index
construction failures that conforming parser-created DOM cannot naturally
produce, including nested document structure. These fixtures preserve the
explicit error-propagation contract without being described as part of the
real parser-backed semantic matrix.

The direct/materialized parity test performs one `html::parse_document` call,
asserts `contains_full_patch_history`, and compares:

```text
ParseOutput.document -> CSS
```

with:

```text
the same ParseOutput.patches -> Browser DomStore -> materialize -> CSS
```

Both paths use the same parser-selected mode and stylesheet input. They compare
the stable CSS-owned selector report, not raw DOM IDs or Rust `Debug`. The
fixture exercises representative host-language, combinator, namespace,
template, static-pseudo, unsupported, and invalid behavior without duplicating
the full CSS matrix in Browser.

## Authoritative matching and diagnostics

Authoritative cascade integration, AF4 conformance, Browser debug, and the
document diagnostic use the checked matcher result. Parsed-unmatched,
unsupported, invalid/malformed, parser-limit invalidity, matcher-limit errors,
and selector-DOM/index errors remain distinct. The conservative compatibility
helper is not called by an authoritative engine path and may only collapse a
matcher limit for explicitly non-authoritative callers.

The integrated diagnostic lives in CSS's document/style integration layer,
above `selectors::matching`. The core matcher remains unaware of stylesheets,
origins, cascade inputs, candidates, or winners. The versioned report includes
the matching environment, contextual CSS-local element identity, namespace,
local name, stylesheet/rule/selector source-order identities, matchability,
matched state, specificity, and stable invalid/unsupported reasons. It also
reports declaration-free and non-contributing rules and does not use cascade
winner output as a substitute for matching evidence.

The only production diagnostic is bounded. It evaluates each original parsed
complex selector directly through the authoritative checked matcher, without
cloning selector ASTs or constructing synthetic selector lists. Independent limits cover
stylesheets, stylesheet rules, elements, selector evaluations, report records,
report storage, serialized bytes, matcher traversal, and selector-DOM/index
construction. Report-record vector growth uses Rust's fallible reservation API;
a reservation failure is a typed, stably serialized terminal failure. Any
limit, reservation, or setup failure discards partial output; a matcher failure
for a real evaluation retains its element/rule/selector context. This guarantee
does not claim recovery from allocation failures outside operations for which
Rust exposes fallible reservation.

## AF4 parent closure

The contract chain is cumulative rather than duplicative. AF1 owns subsystem
and invalidation boundaries; AF2 owns selector parse/validity state; AF3 owns
typed specificity; AF4a owns the explicit parser-selected environment; AF4b
owns the neutral fallible selector-DOM query; AF4c owns HTML host-language
comparison; AF4d owns the five static pseudos; and AF4e supplies aggregate
invalidation plus final integration evidence. Q1-Q2 define the matcher/context
boundary, Q3-Q5 define simple/compound/combinator matching, Q6 defines typed
match outcomes, Q7 defines stable low-level diagnostics, and Q8 fixes
invariants and extension rules. AF4e's higher-level report extends Q7 without
changing the core Q matcher or moving into cascade winners. AE's parser,
identity, patch, namespace, processing-instruction, and template contracts
remain the neutral source of DOM facts.

| AF4 parent requirement | evidence |
| --- | --- |
| CSS-owned typed selector parsing | AF2 and Milestone P contracts/tests |
| matching parser-created DOM elements | AF4a environment + AF4b projection + AF4e parser matrix |
| type, universal, ID, class, attributes, compounds, lists | Q3/Q4 plus AF4e parser matrix |
| descendant, child, sibling combinators | Q5 plus AF4e parser matrix |
| static pseudos | AF4d plus AF4e parser matrix |
| unsupported dynamic pseudos and pseudo-elements | AF2/Q6 plus AF4e parser matrix/report |
| deterministic malformed/error behavior | AF2/Q6 plus AF4e typed error matrix |
| stable matched-selector output | AF4e bounded integrated diagnostic |
| clean AE DOM consumption | AF4a/AF4b plus AF4e direct/materialized parity |
| deterministic representative DOM tests | AF4e parser matrix and Browser parity fixture |
| CSS ownership; no Layout/Paint traversal semantics | AF1 and this ownership contract |
| closeable before future cascade ordering | matching report evaluates rules directly through the authoritative matcher and is independent of cascade winners |

AF4a through AF4e therefore satisfy every AF4 requirement and exit criterion.
No future cascade-winner ordering work is required to close AF4.

## Deliberate remaining gaps

AF4e does not add reverse selector-dependency indexing, targeted or
fine-grained selector invalidation, structural dependency graphs,
candidate-rule indexing, selector caches, bloom filters, `:nth-child()` or new
functional pseudos, dynamic pseudo state, pseudo-elements, namespace selector
syntax, attribute `i`/`s` modifiers, broader escape decoding, cascade-winner
ordering, CSSOM, JavaScript mutation APIs, or selector semantics outside CSS.
