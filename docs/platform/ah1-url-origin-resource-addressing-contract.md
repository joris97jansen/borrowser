# AH1: URL, Origin, And Resource Addressing Architecture Contract

Last updated: 2026-10-24
Status: architecture contract complete; addressing implementation remains pending
Scope: Milestone AH ownership, semantics, integration boundaries, and evidence

AH1 establishes one logical browser-platform owner for addressing. It changes
documentation only. Existing URL helpers, HTTP/HTTPS/file loading, navigation,
DOM publication, styling, rendering, and conformance execution remain unchanged.
It neither completes Milestone AH nor qualifies the existing URL dependency.

## Reading this contract

The following labels separate authority from implementation status:

| Label | Meaning |
| --- | --- |
| Current behavior | Inspected repository behavior, including limitations; not a conformance claim. |
| Standards requirement | Behavior defined by the cited browser standards. |
| Architectural decision | Borrowser ownership or invariant established by AH1. |
| Future requirement | Behavior or evidence required when existing AH implementation work reaches this boundary; not implemented by AH1. |
| Unresolved decision | A choice requiring implementation or interoperability evidence; no default is selected here. |
| Deferred behavior | Outside AH1, or outside the initial AH capability target as stated. |

The semantic names below are vocabulary, not proposed Rust types, APIs, modules,
or crates. An abstraction is not justified by the possibility that it may be
useful later. An implementation phase is not a GitHub issue.

## Standards authority and qualification

**Standards requirement:** the [WHATWG URL Standard](https://url.spec.whatwg.org/)
is the authority for browser URL parsing, relative resolution, normalization,
and serialization. Generic RFC 3986 URI processing is not an interchangeable
browser contract. HTML supplies document/base and creation-context rules; CSS
supplies authored syntax, value semantics, and stylesheet context.

References inspected on 2026-10-24: WHATWG URL (updated 2026-10-19), HTML
(updated 2026-10-22), CSS Values 4 (editor's draft 2026-08-24), and CSSOM
(editor's draft 2026-10-13). The CSS drafts are work in progress. CSS Values'
relative-URL prose still mentions RFC 3986,
while its [URL processing algorithm](https://drafts.csswg.org/css-values-4/#url-processing)
invokes WHATWG parsing. AH uses that explicit algorithm and WHATWG authority;
it does not create a second RFC-based CSS resolver.

**Current behavior:** [Cargo.toml](../../Cargo.toml) declares `url = "2"` and
[Cargo.lock](../../Cargo.lock) selects 2.5.7. Browser and GFX call it directly;
CSS also declares the dependency. This inventory is not parser qualification.
The library's [nonfatal syntax-violation callback](https://docs.rs/url/2.5.7/url/struct.ParseOptions.html#method.syntax_violation_callback)
is relevant to future evaluation, but does not establish complete diagnostic
coverage or compatibility with the cited standards revision.

**Future requirement:** evaluate the actual selected backend behind
Borrowser-owned semantics before claiming support. Record its version,
standards reference date/revision, pinned upstream fixture provenance, selected
coverage, observed deviations, and diagnostic limitations. Use representative
absolute/relative, host/IDNA/IP, port, escaping, special/non-special, opaque-path,
query, fragment, and failure cases. Include repeated parse/serialize stability
and explicit base-context cases. Independent expectations and scoped browser
comparisons supplement each other; a library description or one successful
example is insufficient. Unsupported coverage remains visible. AH1 selects no
backend, adds no fixture, and performs no parser qualification run.

## Ownership and dependency direction

**Architectural decision:** addressing is one logical ownership boundary.

| Owner | Responsibility | Boundary |
| --- | --- | --- |
| Platform/addressing | URL parsing, normalization, serialization, relative resolution, base-resolution semantics, origin determination/representation/serialization/comparison, resource-address construction, addressing identity projections, deterministic diagnostics | No DOM traversal, CSS grammar/cascade, I/O, render scheduling, or security-policy enforcement. |
| HTML/parser and DOM model | Parser-created names, namespaces, attributes, text, relationships, and resource-reference strings | No URL parser or origin computation. HTML syntax decoding is not URL parsing. |
| CSS | Authored resource-reference values, CSS escapes/syntax, property acceptance, cascade, computed style, and CSS-specific reference meaning | Delegates URL semantics to addressing using the applicable source context; does not independently parse URL strings. |
| Browser/runtime/document loading | Committed document and base-state lifetime, DOM discovery, stylesheet location provenance, request correlation, resource availability, invalidation, and orchestration | Supplies facts/context to addressing; does not invent normalization, origin, or relative-resolution algorithms. |
| Future networking | Transport and response processing using normalized resource addresses | Does not select document bases or reinterpret authored references. |
| Future cache | Response reuse and complete cache-key policy using resource identity inputs | Does not parse URLs or equate an address key with the complete cache key. |
| Future security policies | Policy decisions using origins, addresses, and policy context | Does not compute origins independently; same-origin classification alone is not permission. |
| Layout and Paint | Geometry/resource presentation and visual output respectively | No URL parsing, base selection, origin decisions, or resource discovery. |
| GFX/input | User interaction and presentation | Navigation intent is coordinated by Browser; GFX is not the future URL resolver. |
| Conformance tooling | Fixtures, observations, comparison, and reporting | Consumes production results; does not implement URL/origin semantics. |

**Unresolved decision:** physical crate/module placement is deferred until the
first implementation supplies evidence from dependency direction, actual
consumers, API requirements, cycle avoidance, and ownership/cohesion. No
dedicated crate or specific module is recommended by AH1.

The eventual implementation must sit below its consumers and must not depend
on Browser, HTML/CSS implementation details, Layout, GFX, or networking
implementations/runtimes. Inputs must carry the required context without
requiring addressing to inspect those subsystems' private state.

The current manifests demonstrate why names are insufficient evidence:
[platform](../../crates/platform/Cargo.toml) depends on GFX, bus, net, HTML,
and runtimes; [GFX](../../crates/gfx/Cargo.toml) depends on CSS/Layout/HTML;
[Browser](../../crates/browser/Cargo.toml) depends on those consumers and net;
[runtime_net](../../crates/runtime_net/Cargo.toml) depends on net and bus.
Making those consumers depend back on the existing platform crate would
introduce cycles. This constrains implementation direction without choosing a
new physical abstraction. No manifest changes follow from AH1.

## Semantic vocabulary and identity

**Architectural decision:** these concepts remain distinct even where their
values happen to coincide:

| Concept | Meaning and relationship |
| --- | --- |
| Document URL | Address associated with the document. Browser retains its lifecycle context; requested and accepted response URLs are separate provenance. |
| Document fallback base URL | Base supplied by document URL/creation rules before an applicable base element is considered. |
| Active document base URL | Effective base selected through HTML integration rules, used for relevant document-relative references. |
| Stylesheet base URL | Explicit stylesheet base when present; otherwise CSS selects its resolution base from stylesheet location and applicable environment rules. |
| Authored resource reference | HTML/CSS producer value plus its source context; preserves the producer's syntax/semantic distinctions before addressing. |
| Resolved resource URL | Absolute parsed URL with query and fragment information retained. Resolution does not imply an available loader. |
| Origin | Tuple or opaque authority identity, determined separately from URL/resource equality. |
| Fetch-oriented resource address | Normalized address eligible for a declared resource operation, with its address-identity projection. It is not a complete Fetch request. |
| Future cache identity | Cache-owned composition using address identity and additional request/response/partition context. |
| Stylesheet/DOM attachment identity | Identity and lifetime of a stylesheet attachment or DOM node, independent of whether references resolve to equal URLs. |

Browser navigation is a consumer with its own lifecycle and comparison rules.
Same-document decisions must not be reduced to origin equality or generic
resource identity. Existing navigation helpers are integration/migration
points. AH introduces no same-document navigation identity abstraction or
type; navigation-history behavior remains outside AH.

### Query, fragment, and normalization boundaries

**Architectural decision / future requirement:** normalized URL storage must
preserve query order, repeated parameters, and absent-versus-empty components
where meaningful. Consumers must not sort query parameters, decode/re-encode
paths independently, or apply additional string trimming as a universal rule.
Address-bar guessing and display elision are not URL normalization.

Resolved references retain fragments. The proposed fetch-address projection
excludes the fragment and preserves the query; the complete resolved reference
remains available to consumers that interpret fragments. For example,
`https://example.test/a?q=1#one` and `https://example.test/a?q=1#two` share this
address projection, while changing `q=1` to `q=2` changes it. This does not prove
interchangeable rendered resources or same-document navigation. A CSS local
reference must not become a network request just because it has URL syntax.

Address identity is not response identity, decoded-image identity, stylesheet
source identity, or a filesystem canonical path. No DNS lookup, symlink
resolution, filesystem case folding, or transport activity is part of URL
normalization. Do not silently discard credentials to form an equality key;
credential admission and diagnostic redaction require explicit consumer rules,
and AH does not add authentication behavior.

## Parsing results and deterministic diagnostics

**Standards requirement:** a URL validation error can accompany successful
parsing; it is not synonymous with parser failure.
[URL validation errors](https://url.spec.whatwg.org/#validation-error)

**Architectural decision / future requirement:** keep these outcomes separate:

- A parsed result, with any reported validation diagnostics.
- A fatal parse/resolution failure, with explicit base-context provenance.
- A syntactically parsed result whose requested capability is unsupported.
- An implementation/resource failure, distinct from invalid authored input.

Addressing owns semantic diagnostic categories and their deterministic order.
Source producers supply provenance; Browser may display or retain diagnostics
without reclassifying them. Stable categories and outcomes must not depend on
third-party error display strings. Human-readable explanations are secondary.
Missing positions or unreported validation categories must be identified as
coverage limitations, not fabricated as precise evidence or treated as proof
of valid input. No diagnostic enum or schema is specified in AH1.

Future debug output must distinguish the selected base and its source,
resolved URL, scheme capability, origin kind, and address projection. It must
avoid timestamps, pointer identities, process-specific IDs, unordered output,
and credential disclosure in display diagnostics. Any redacted display is
separate from semantic equality. Debug snapshots remain internal regression
contracts, not public APIs or transport formats.

## Origins and creation context

**Standards requirement:** HTML defines tuple and opaque origins. A tuple has
scheme, host, port, and a domain field; same-origin tuple comparison uses
scheme, host, and port. The domain field is normally null; `document.domain`
relaxation and same-origin-domain behavior are outside AH. An opaque origin
has identity that cannot be recreated from its serialization. Distinct opaque
origins can both serialize as `null` and still compare unequal.
[HTML origins](https://html.spec.whatwg.org/multipage/browsers.html#origin)

Same-origin succeeds for matching tuple components or the same opaque identity;
all other comparisons fail. Tuple serialization uses the scheme and serialized
host, with a port when present in the normalized origin. It is distinct from
URL serialization and must not expose a path, query, fragment, or credentials.

**Architectural decision:** parsing a URL, determining an origin, serializing
an origin, and comparing origins are distinct operations. Canonical tuple
components come from addressing, never host-string heuristics in consumers.
Paths, queries, fragments, and an active document base do not determine tuple
equality. CSS cascade origins and parser-entry origins are unrelated concepts.

**Future requirement:** origin determination receives explicit creation
context where needed. Browser retains the chosen document origin throughout
its lifetime. Preserve an existing opaque identity when the rules share it;
create a fresh identity only when the rules require creation. Do not repeatedly
invoke a URL-origin accessor to reconstruct an opaque document origin.
Deterministic test labels describe relationships within an observation context;
they are not semantic IDs, globally reusable identities, or cross-engine keys.

**Standards requirement:** document creation can inherit an existing origin,
including specified `about:blank`/`about:srcdoc` contexts, or require a new
opaque origin. A URL-only result is not a replacement for those rules.
[Document origin determination](https://html.spec.whatwg.org/multipage/document-sequences.html#determining-the-origin)

**Deferred behavior:** missing creator/blob/environment contexts must remain
explicitly unsupported where required. AH does not implement sandboxing,
origin-based access control, or navigation creation machinery merely to model
these distinctions. Resource origin and the requesting document's origin must
not be silently substituted for one another.

### Unresolved file-origin policy

**Unresolved decision:** WHATWG leaves `file:` origin behavior to the
implementation. AH1 selects no Borrowser default, including no rule that each
independent file document necessarily receives a fresh opaque origin.
[URL origin rules](https://url.spec.whatwg.org/#origin)

When existing AH implementation work reaches this behavior, its decision must:

- Be deterministic for explicitly defined inputs and creation context.
- Preserve origin determination, identity, serialization, and comparison as
  distinct concerns.
- Never derive origin accidentally from raw URL-string equality, common path
  prefixes, or identical `null` serializations.
- Record relevant browser/interoperability evidence and lifecycle implications,
  including repeated determination, independent documents, and shared context.
- State compatibility limitations and provide representative deterministic
  regression coverage for the chosen policy.

This is an outstanding implementation/interoperability decision within AH,
not a reason to create a separate issue. Existing local-file loading is not
evidence that any browser origin policy is already implemented.

## Document base integration

**Standards requirement:** the document base uses the first descendant HTML
`base` element with `href` in tree order, then a document base override if
present, otherwise the fallback. The fallback normally uses the document URL;
specified about-document creation contexts can supply an about base instead.
[HTML document bases](https://html.spec.whatwg.org/multipage/urls-and-fetching.html#document-base-urls)

The selected element's frozen base is parsed against the fallback using the
document's character encoding, not against itself or a previous active base.
Failure, a `data:`/`javascript:` result, or rejection by the standards policy
hook uses the fallback; an invalid first candidate does not authorize choosing
a later one. Becoming the first candidate or changing its `href` triggers
frozen-base processing. Removal, insertion, or reordering can change which
candidate applies.
[HTML base element](https://html.spec.whatwg.org/multipage/semantics.html#the-base-element)

**Architectural decision / future requirement:** HTML supplies names,
attributes, tree order, and ordinary document relationships. Browser discovers
the applicable facts from committed parser-created DOM; addressing owns their
base-resolution semantics. A head-only metadata scan is not a substitute for
the complete rule. Foreign lookalikes and inert template contents do not become
active HTML base candidates.

Browser owns document URL/fallback/active-base state and its relationship to
the current document handle/version. Base-dependent discovery must observe a
coherent committed state. Rejected publications and stale request generations
must not change it. Preserve relevant ordered mutation/creation facts when
frozen-base semantics require history; final-tree scanning or arbitrary byte
chunk boundaries must not silently determine the result. The implementation
must demonstrate this mapping against the existing publication contract; AH1
prescribes no new mutation API or batching algorithm.

Base changes never change the retained document origin. Browser coordinates
dependent work; CSS classifies style implications and Layout/Paint consume
their normal downstream inputs. Changing a base must not silently retarget an
already constructed resource request. Later integration must specify which
subsequent uses are re-resolved and preserve the provenance of earlier uses.

**Deferred behavior:** dynamic base updates, document base overrides, inherited
about-document creation behavior, and policy-hook enforcement are not added
by AH1. Describing their standards rules does not claim existing support.

## Stylesheet bases and authored references

**Standards requirement:** CSS Values selects a style resource base from the
stylesheet's stylesheet base URL when present, otherwise its location when
present, otherwise the relevant settings object's API base URL. Embedded
stylesheets and inline declarations therefore require their applicable
environment context. Local fragment references and empty URL values have
CSS-specific semantics. Constructed stylesheet base overrides remain deferred
with CSSOM.
[CSS resource URL rules](https://drafts.csswg.org/css-values-4/#url-processing)

Current HTML `<link rel="stylesheet">` processing supplies stylesheet
`location` from `response`'s URL list[0]. CSSOM describes `location` as the
absolute URL string for the first request, fixed for the stylesheet's lifetime.
HTML acknowledges the URL-versus-string integration issue tracked in
[CSSWG #9316](https://github.com/w3c/csswg-drafts/issues/9316).
[HTML stylesheet processing](https://html.spec.whatwg.org/multipage/links.html#link-type-stylesheet)
[CSSOM stylesheet location](https://drafts.csswg.org/cssom/#concept-css-style-sheet-location)

**Architectural decision / future requirement:** Browser preserves qualified
stylesheet-location provenance required by the applicable integration rules
and coordinates availability; CSS identifies the relevant authored value and
context and delegates URL semantics to addressing. External CSS must not
resolve against the embedding page merely because that page initiated the
load. Document base changes do not rebase an external sheet's own location.
Inline contexts must not silently freeze an obsolete document base without
a documented standards reason.

**Unresolved decision:** exact redirect/location reconciliation requires
standards and interoperability evidence in the existing AH implementation
work. AH1 selects neither the authored relative `href`, initially resolved
request URL, final redirect URL, nor an arbitrary network-response display URL
as a universal stylesheet base. This preserves stylesheet-location provenance
without implementing redirects or creating another GitHub issue.

**Standards requirement:** after stylesheet decoding into Unicode and CSS
syntax/escape processing yield the authored URL value, CSS URL parsing omits
the WHATWG URL parser's encoding argument. It therefore uses the default
UTF-8 behavior regardless of the stylesheet's original encoding.
[CSS URL encoding rule](https://drafts.csswg.org/css-values-4/#url-processing)
[WHATWG URL parser](https://url.spec.whatwg.org/#concept-url-parser)

**Architectural decision / future requirement:** preserve this integration
context when delegating to addressing. HTML algorithms can supply different
parsing context, including document character encoding for the frozen
`<base href>` algorithm above. No universal hidden encoding policy may replace
the applicable HTML or CSS rule. This distinction justifies no Rust context
type or future public API design in AH1.

HTML reference strings come from parser-created attributes; CSS reference
values come from CSS syntax/value processing. Neither producer reimplements
URL parsing. Do not reparse serialized CSS or scan all `url()` occurrences as
active requests: unsupported declarations, rules, and properties remain
subject to CSS applicability. Preserve source context for empty values,
escaping, local references, and base selection instead of erasing those
distinctions through universal trimming.

The current `CssUrlValue` is only an authored wrapper. AH1 adds no accepting
property grammar, `@import` loading, image/font property behavior, CSSOM API,
or stylesheet loader integration. Equal addresses never merge AF5 attachment
identities or change document/source order.

## Scheme capability matrix

**Future requirement / initial AH target:** parsing and classification are
separate from construction of a supported fetch-oriented address. The parsing
column describes the grammar to evaluate, not an assertion that every input
succeeds or that the existing backend is qualified. An unavailable operation
must produce unsupported-capability diagnostics rather than a fake syntax
error or a rewritten scheme.

The origin summaries below refer to the
[WHATWG URL origin algorithm](https://url.spec.whatwg.org/#origin); document
creation context and the unresolved file policy remain as described above.

| Scheme | Parsing/classification target | Origin semantics | Initial AH addressing capability | Current transport and deferred behavior |
| --- | --- | --- | --- | --- |
| `http` | Special URL | Tuple | Supported target | HTTP exists; fetching changes are outside AH. |
| `https` | Special URL | Tuple | Supported target | HTTPS exists; fetching/TLS changes are outside AH. |
| `file` | Special URL | Explicitly unresolved policy | Supported target | Local-file loading exists; origin policy and correct path mapping require later evidence. |
| `about` | Non-special URL | Opaque URL origin; document context can differ | Reserved document handling; no generic fetch address | No general about-document handler; creation contexts deferred. |
| `data` | Non-special URL | Opaque | Deferred | No data-resource loader. |
| `blob` | Non-special URL | Blob entry origin or specified embedded-URL/opaque fallback | Deferred | No blob store/loader; do not guess creation context from text. |
| `javascript` | Non-special URL | Opaque URL origin; navigation execution is separate | Unsupported operation | No JavaScript execution. |
| `ws` | Special URL | Tuple | Reserved | No WebSocket transport. |
| `wss` | Special URL | Tuple | Reserved | No WebSocket transport. |
| `ftp` | Special URL | Tuple | Unsupported operation | No FTP transport. |
| Unknown schemes | Generic non-special rules where valid | Opaque under generic URL rules | Unsupported operation | No implicit transport or handler registration. |

**Current behavior:** existing helpers do not implement this unified
capability classification. HTTP/HTTPS/file rows record real existing paths,
not full scheme conformance. The initial target is an architectural scope for
later AH work, not a claim of newly working behavior. Recognizing `ws`, `ftp`,
or an unknown scheme during parsing does not authorize transport. A parseable
opaque-path URL is not necessarily usable as a relative base.

## Future networking, cache, and policy handoffs

**Architectural decision / future requirement:** the conceptual handoff is:

```text
HTML/CSS reference + explicit source/base/parsing context
  -> addressing parse/resolve and diagnostic result
  -> supported operation classification and normalized resource address
  -> Browser coordination with request and attachment context
  -> future transport consumer
```

Networking receives an absolute normalized address already classified for the
operation. It must not need the DOM, CSS text, or a base URL to resolve it.
Request kind, correlation, cancellation, attachment, and initiating-document
context remain separate from address equality. Transport serialization uses
the addressing-owned representation; it must not become a second semantic
parser or silently change the accepted address. This defines responsibilities,
not a proposed request struct, public API, or bus migration.

Response/requested address provenance stays distinct. Future redirect-related
URL interpretation must delegate to addressing, but redirect handling is not
implemented or redesigned in AH. Address construction performs no HTTP, file,
DNS, or other I/O and is not a fetch or authorization decision.

Future cache consumes an address identity input. Complete HTTP cache keys can
also depend on method, variation headers, and partition context.
[HTTP caching model](https://httpwg.org/specs/rfc9111.html#caching.overview)
Cache owns that composition and reuse policy, not URL parsing. Neither a
matching resource key nor a matching origin proves that responses are reusable.
Future security policies consume explicit requesting/target origins and
addresses; they own policy decisions, not origin computation. AH introduces
no enforcement and no complete Fetch request model.

## Current implementation and migration inventory

**Current behavior:** all paths below remain unchanged by AH1. They are
integration/migration points, not exemptions permitting new independent URL
semantics. Paths are relative to the repository root.

| Path and symbols | Current behavior / migration concern |
| --- | --- |
| [browser/src/page/mod.rs](../../crates/browser/src/page/mod.rs): `PageState`, `start_nav`, `update_head_metadata` | `base_url: Option<String>` is populated from navigation/response state; fallback and active bases are not separate artifacts. |
| [browser/src/tab/html.rs](../../crates/browser/src/tab/html.rs): `on_html_network_start` | Accepts the first current document response, records its final URL, and initializes the page through `response.display_url()`. Existing redirects are real behavior. |
| [browser/src/tab/events.rs](../../crates/browser/src/tab/events.rs): `commit_document_publication_with_identity_resolver` | Stages and validates DOM/mutation state before commit, then updates head metadata and discovers resources. Preserve atomic publication and stale-generation filtering. |
| [html/src/head.rs](../../crates/html/src/head.rs): `HeadMetadata`, `extract_head_metadata`, `fill_head_metadata_from`; [html/src/collect.rs](../../crates/html/src/collect.rs): `collect_img_srcs`, `collect_stylesheet_hrefs` | Produce raw `base_href`, image, and stylesheet strings. Head extraction checks direct head children; no active-base consumer of `base_href` exists. Preserve namespace and template boundaries. |
| [css/src/values.rs](../../crates/css/src/values.rs): `CssUrlValue`; [css/src/model/mod.rs](../../crates/css/src/model/mod.rs): `ValueToken::Url`; [css/src/specified/core.rs](../../crates/css/src/specified/core.rs): `unsupported_component_error` | Authored wrapper and URL syntax exist; current property parsing rejects unsupported URL components. These are not a resource loader. |
| [browser/src/document_style.rs](../../crates/browser/src/document_style.rs): `DocumentStyleSet::reconcile_from_dom`, `collect_stylesheet_inputs`, `resolve_url`; [browser/src/page/stylesheets.rs](../../crates/browser/src/page/stylesheets.rs): `reconcile_document_stylesheets` | Stylesheet resolution trims with `str::trim`, rejects empty references, requires a base, and joins via `url::Url`. Attachment reconciliation is separate from resource identity. |
| [browser/src/tab/css.rs](../../crates/browser/src/tab/css.rs): `on_css_network_start`, `on_css_network_done`, `on_css_decoded_block`; [runtime_css/src/lib.rs](../../crates/runtime_css/src/lib.rs): `start_css_runtime` | Response metadata is tracked; decoded CSS is installed by slot. Completion transports the requested URL; there is no qualified stylesheet-base handoff for CSS resource values. |
| [browser/src/tab/discovery.rs](../../crates/browser/src/tab/discovery.rs): `discover_images`, `base_url`, `send_fetch` | Parses the page base and joins collected image references independently of the rendering resource provider. |
| [browser/src/resources.rs](../../crates/browser/src/resources.rs): `resolve_image_source`, `strip_surrounding_html_ascii_whitespace`, `ResourceManager::request_image` | Strips surrounding HTML ASCII whitespace, rejects empty input, tries absolute parsing before joining; image lookup uses string keys. This differs from stylesheet/GFX paths. |
| [browser/src/rendering/frame.rs](../../crates/browser/src/rendering/frame.rs): `BrowserReplacedElementInfo::resolve_image_source` | Browser provides image resolution to Layout through the existing resource boundary. |
| [browser/src/tab/nav.rs](../../crates/browser/src/tab/nav.rs): `normalize_url`, `is_same_document_navigation_with`; [browser/src/tab/ui.rs](../../crates/browser/src/tab/ui.rs): `url_label`, `display_title` | Navigation recognizes lowercase HTTP/HTTPS/file prefixes and otherwise guesses HTTPS; same-document logic compares selected URL components. UI also parses URLs for display. These are consumer integration points. |
| [gfx/src/util/url.rs](../../crates/gfx/src/util/url.rs): `resolve_relative_url`; [gfx/src/input/route/actions.rs](../../crates/gfx/src/input/route/actions.rs): `activate_release_target` | Link handling resolves in GFX and passes the raw reference through when no base exists. |
| [bus/src/lib.rs](../../crates/bus/src/lib.rs): `CoreCommand::FetchStream`, `CoreEvent`; [core_types/src/lib.rs](../../crates/core_types/src/lib.rs): `NetworkResponseInfo`, `ResourceKind`, `StylesheetSlotId` | Commands/events carry URL strings; requested/final URLs and request/attachment identities are distinct. |
| [runtime_net/src/lib.rs](../../crates/runtime_net/src/lib.rs): `start_net_runtime`; [net/src/fetch.rs](../../crates/net/src/fetch.rs): `fetch_stream_with_policy`; [net/src/file.rs](../../crates/net/src/file.rs): `is_file_url`, `fetch_file_url` | Existing HTTP/HTTPS client/redirect behavior, event forwarding/cancellation, and local-file streaming. File mapping strips `file://` manually rather than using a qualified addressing boundary. |
| [conformance/src/execute.rs](../../crates/conformance/src/execute.rs): `execute_fixture`, `forward_commands`; [conformance/src/environment.rs](../../crates/conformance/src/environment.rs): `URL`; [conformance/src/projection/mod.rs](../../crates/conformance/src/projection/mod.rs): `ObservationRequest` | Fixed document response injection, production execution, and subresource denial. No URL/origin observation family exists. |

## Architectural constraints against further duplication

**Architectural decision:** future changes must not add independent URL
parsers, origin computations, base-joining algorithms, scheme-prefix routing
as URL semantics, or competing normalization/equality rules in consumers.
Use the addressing owner once implemented; before then, a need for semantics
belongs in existing AH implementation work rather than another local helper.

This rule does not ban HTML attribute decoding, CSS tokenization, opaque
transport of strings in existing code, or UI presentation. It bans giving those
operations authority over URL meaning. The migration inventory is an explicit
baseline to review against, not a claim that present code already obeys every
future boundary. AH1 neither removes existing direct parser calls nor adds a
dependency allowlist test or speculative runtime guard.

## AE, AF, and AG integration

- AE supplies parser-created DOM through the
  [AE1 ownership](../html5/ae1-html-parser-dom-ownership-contract.md),
  [AE2 node model](../html5/ae2-parser-created-dom-node-model.md), and
  [DOM publication](../html5/dompatch-contract.md) contracts. Addressing does
  not consume tokenizer recovery state or change parser-created identities.
- AF supplies CSS ownership and artifact/invalidation boundaries through
  [AF1](../css/af1-selector-cascade-computed-style-architecture-contract.md),
  [AF5](../css/af5-stylesheet-rule-collection-source-order-contract.md),
  [AF8](../css/af8-computed-style-document-artifact-contract.md), and
  [AF9](../css/af9-selector-cascade-invalidation-dependencies.md).
  [AD2](../css/ad2-typed-core-css-value-model.md) supplies authored URL values.
  Address equality cannot collapse stylesheet slots or change cascade order;
  CSS still owns computation and semantic invalidation classification.
- AG supplies evidence discipline, not URL behavior. Its
  [guide](../../crates/conformance/README.md),
  [AG5 observation](../conformance/ag5-phase-comparison-projections.md),
  [AG6 fixture](../conformance/ag6-fixture-discovery-and-metadata.md), and
  [AG8 reporting](../conformance/ag8-deterministic-reporting-and-suites.md)
  contracts remain unchanged. AH does not expand AG beyond its nine issues.

### Later AH observations and validation

**Future requirement:** later AH tests must exercise production addressing
with explicit input/base/creation context and independently expected outputs.
Cover parse/serialization, validation versus failure, base selection and
lifecycle, tuple/opaque relationships, same-origin comparison, supported and
unsupported operations, HTML/CSS extraction, and address-identity projections.
Test relevant namespace/template exclusions and query/fragment distinctions.

Any later AG-compatible observation must separate native provenance from
common comparison fields, unsupported coverage from execution failure, and
independent expectations from cross-engine agreement. Opaque origins compare
through within-case relationships, never matching generated labels between
engines. Internal snapshots remain separate from common observations.

Pure addressing tests must not require rendering, JavaScript, or networking.
Document integration evidence must observe accepted Browser state without
reimplementing parser/CSS/addressing rules in tooling. Current AG subresource
denial is not weakened to accommodate prospective fixtures. No observation
schema, fixture, capture adapter, CI job, or snapshot is added in AH1.

## Deliberate exclusions and open decisions

**Deferred behavior:** AH1 adds no parser, resolver, origin computation,
document-base update, resource discovery, transport migration, or executable
guard. Milestone AH's addressing foundation does not add HTTP fetching,
redirect handling, cookies, storage, HTTP cache, CORS, CSP, mixed-content
blocking, navigation history, service workers, JavaScript URL APIs, or full
browser security-policy enforcement. Existing HTTP/HTTPS/file and navigation
behavior remains intact; these exclusions are not statements of absence.

Physical placement and `file:` origin policy remain explicitly unresolved.
Backend qualification, supported-context implementation, and publication/base
lifecycle evidence remain future requirements. Resolve them when existing AH
work supplies the evidence; they do not justify new issues solely because
they are separate decisions or implementation phases.

## AH1 acceptance and documentation validation

AH1 is closeable as a contract issue using the following mapping. It does not
declare the implementation targets above complete.

| Requirement / exit criterion | Contract evidence |
| --- | --- |
| Milestone AH architecture documented | Scope/status, standards authority, and ownership sections. |
| URL parsing, normalization, resolution, base, origin, same-origin, resource construction, and diagnostics have an owner | Ownership matrix, semantic vocabulary, parsing/diagnostics, origins, and base integration. |
| HTML/CSS are reference producers; Browser coordinates document/loading state | Ownership matrix, document/stylesheet integration, and AE/AF links. |
| Networking/cache/security consume addressing outputs | Future consumer handoff and identity distinctions. |
| Supported, unsupported, and reserved schemes listed | Scheme capability matrix with separate current transport column. |
| Future networking has a normalized-address input contract | Operation-classified absolute address plus separate lifecycle/request context; no raw reference resolution in transport. |
| Builds on AE, AF, AG without extending AG | Integration and later-observation requirements. |
| Raw URL semantics do not spread further | Architectural duplication constraint and concrete existing migration inventory. |
| Non-goals explicit; no full parser required for closure | Exclusions and distinction between contract completion and future implementation. |

Validation for AH1 consists of `git diff --check`, changed-link and referenced
symbol verification, complete diff/scope inspection, and consistency review
against AE/AF/AG and the [architecture overview](../architecture/ARCHITECTURE.md).
The [feature-gap tracker](../engine-feature-gap-tracker.md) records contract
completion separately from pending behavior. No test that duplicates Markdown
or guards a nonexistent API is warranted. Rust subsystem/runtime suites do
not prove these unimplemented semantics; normal merge-gate validation remains
separate.
