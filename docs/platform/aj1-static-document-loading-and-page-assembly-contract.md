# AJ1: Static Document Loading And Page Assembly Contract

> AJ4 refinement: [document character decoding](aj4-document-character-decoding.md) selects the declared/default UTF-8 subset before activation and transports typed terminal decoder evidence. Silent cancellation, unavailable interruption evidence, and explicit BOM/meta deferral remain qualified limitations. Earlier charset-selection gaps below describe their historical checkpoints.

> AJ3 current qualification: AJ3 qualifies the earlier eager navigation reset and response-policy gaps: fetch submission preserves the actual active document/work/history until eligible response admission. Typed status/MIME/disposition preparation and 204/205 preservation follow the [AJ3 contract](aj3-main-document-response-preparation.md). This establishes the declared-HTML subset, not full response applicability or assembly.

Issue: [#1132](https://github.com/joris97jansen/borrowser/issues/1132).
Milestone AJ — Document Loading Lifecycle, Stylesheet Loading, and Static Page Assembly.
Architecture baseline: master `44097fafa00ca221f128d1ab4fc8a42188c1e83d`,
inspected on 2026-11-22.

## Scope, status, and authority

AJ1 defines the architecture for assembling a static HTML/CSS page from an
address, fetched document, parser-created DOM, and supported stylesheet inputs.
It adds documentation only. It implements no loading behavior and does not close
Milestone AJ or establish new conformance coverage.

**Existing implementation** below describes the inspected baseline.
**Required architecture** constrains subsequent work; it does not assert that
the baseline already implements every admission policy, completion result, or
diagnostic. **Deferred functionality** remains unsupported or incomplete, rather
than becoming a permanent browser rule. These labels are prose, not new enums.

Document Loading is a logical Browser/runtime responsibility. Existing
`DocumentLoadState`, `StylesheetLoadState`, `DocumentAddressState`,
`DocumentStyleSet`, and `PageState` already own the relevant lifetimes and
handoffs. This contract introduces no separate loader, crate, state machine,
resource framework, assembled-page wrapper, API, or duplicate authoritative
state. Later changes need a concrete unmet requirement before adding any such
structure. An implementation phase is not itself a GitHub issue; AJ2–AJ10 labels
do not establish necessary future engineering boundaries.

The authoritative foundation is:

| Boundary | Existing contracts |
| --- | --- |
| Addressing and document lifetime | [AH1 ownership](ah1-url-origin-resource-addressing-contract.md), [AH4 document/base state](ah4-document-url-and-base-state.md), [AH5 origins](ah5-origin-representation-and-same-origin.md), [AH6 resource identity](ah6-resource-addresses-and-identity-keys.md). |
| Authored reference observation | [AH7 HTML references](ah7-html-resource-reference-discovery.md), [AH8 CSS references and response bases](ah8-css-resource-reference-discovery.md). |
| Fetch | [AI1 ownership](ai1-fetch-http-networking-response-contract.md), [AI2 requests](ai2-typed-fetch-request-construction.md), [AI3 responses](ai3-typed-fetch-response-status-headers-body.md), [AI6 redirects](ai6-redirect-handling-limits-final-url.md), [AI7 failures](ai7-network-errors-cancellation-timeouts-resource-failures.md), [AI8 metadata](ai8-response-metadata-and-loader-handoff.md), [AI10 assessment](ai10-fetch-http-networking-response-handling-foundation-closeout.md). |
| HTML | [AE1 parser/DOM ownership](../html5/ae1-html-parser-dom-ownership-contract.md), [DOM patches](../html5/dompatch-contract.md), [node identities](../html5/node-identity-contract.md), [AE14 supported parser subset](../html5/ae14-html-parser-foundation-closeout.md). |
| CSS | [AD1 property ownership](../css/ad1-css-value-property-ownership-architecture.md), [AF1 style architecture](../css/af1-selector-cascade-computed-style-architecture-contract.md), [AF5 collection/order](../css/af5-stylesheet-rule-collection-source-order-contract.md), [AF6 precedence](../css/af6-cascade-ordering-winner-selection-contract.md), [AF8 computed artifacts](../css/af8-computed-style-document-artifact-contract.md), [AF9 invalidation](../css/af9-selector-cascade-invalidation-dependencies.md). |
| Retained rendering | [AC10 consolidated contract](../rendering/ac10-retained-rendering-runtime-closeout.md), including its AC1–AC9 references. |
| Conformance | [AG5 observations](../conformance/ag5-phase-comparison-projections.md), [AG6 fixture discovery](../conformance/ag6-fixture-discovery-and-metadata.md), [AG8 reporting](../conformance/ag8-deterministic-reporting-and-suites.md), [AI9 networking fixtures](ai9-fetch-network-conformance-fixtures-and-snapshots.md), [harness guide](../../crates/conformance/README.md). |

Historical checkpoints remain historical. AH5 refines AH4's original origin
exclusion; AI response admission supersedes AH4's old requested-URL fallback.
AH8 resolves the effective response-base question in AH1/AF5. AC6/AC7 refine
AC1's initial retention exclusions. AI10's working-tree qualification narrative
is not the current status of its presence in this master. No historical result,
milestone acceptance, or CI evidence is rewritten or inherited as AJ1 validation.

## Ownership and dependency direction

| Owner | Owns | Must preserve |
| --- | --- | --- |
| Document Loading within Browser/runtime | Static document load lifecycle; main-document fetch orchestration; document response handling; document/final URL, origin and base-state coordination; stylesheet discovery and fetch orchestration; attachment ordering; static assembly state; loading diagnostics/debug envelope. | Delegate URL semantics to AH, Fetch to AI, parsing/tree construction to AE, and CSS meaning to CSS. Coordinate existing state instead of mirroring it. |
| AH / Addressing | URL parsing, normalization and resolution; origin representation and derivation; base-address semantics; resource addresses, operation capabilities and retrieval identity. | AH4's Browser-owned HTML base selection/freezing and document-state lifetime remain in Browser. Addressing primitives neither traverse DOM nor schedule I/O. |
| AI / Fetch | Typed request admission, response facts and metadata interpretation, redirects, body processing/completion, network errors and networking diagnostics. | A response or complete transfer does not decide document replacement, stylesheet applicability, character decoding, parsing or installation. |
| AE / HTML | HTML input preprocessing, tokenizer/tree-builder semantics, parser-created DOM, document mode, parser diagnostics and patch emission. | Runtime materializes parser output without repairing markup or reinterpreting parser state. |
| CSS / AD–AF | Stylesheet/declaration parsing, selector semantics, rule/condition applicability, cascade precedence, inheritance, computed style, property meaning and semantic invalidation facts. | Browser supplies ordered available attachments and exact context; it does not reconstruct CSS decisions. |
| Browser/runtime / AC | Retained render identity and artifact lifetime, invalidation coordination, work planning, frame execution and observation validity. | Retention does not transfer CSS, Layout or Paint semantics. Document Loading enters this existing path. |
| Layout / Paint | Layout structures, used geometry and formatting / semantic paint output and ordering. | Consume computed style and layout/visual inputs; acquire no HTTP, decoding, URL, dependency-discovery or parsing duties. |
| AG / tooling | Fixture inputs, selected observations, independent expectations, comparison and reporting. | Exercise production boundaries without duplicate loading, parsing, cascade or rendering algorithms. |

## Artifact vocabulary

These are semantic distinctions, not required new Rust types.

| Term | Meaning and boundary |
| --- | --- |
| Document address | AH `ResourceAddress` identifying the intended document operation. Representability is distinct from AI request admission and document-creation capability. |
| Document fetch | An admitted AI `FetchRequest` and its execution, including redirects, body delivery or failure. It does not create a DOM. |
| Document response | Acquired immutable `NetworkResponseInfo` plus separately tracked body delivery/outcome. Metadata alone is neither a complete body nor an accepted/installable document. |
| Parser input | Decoded character input under an explicit encoding policy, before AE's HTML preprocessing. The existing byte entry point can perform decoding internally; bytes are not assumed to be valid text. |
| Parser-created document | AE's tree and patch semantics, materialized and accepted by Browser for a document handle/version/mode. A committed preview is a document publication without necessarily being the final parser result. |
| Stylesheet dependency | A stylesheet-bearing occurrence selected for the supported static loading scope, with owner, source, order and applicability/capability outcome. Discovery alone does not authorize a request. |
| Loaded stylesheet | A current attachment with an admitted available CSS parse and retained source provenance. Loaded does not mean every rule is valid, supported, active or a cascade winner, nor does parse availability alone prove runtime terminal settlement. |
| Static stylesheet set | Browser's ordered current attachments and their outcomes; available CSS parses feed AF with UA/author origins and source coordinates. Failed/pending attachments do not become fabricated empty CSS inputs. Element-attached style attributes enter CSS separately. |
| Assembled page | The current committed document and its static stylesheet inputs after the settlement conditions below. It is a Browser-owned association of existing state, not a second DOM, computed-style cache or render object. |
| Rendered page | An accepted render execution using the current DOM/style context and valid retained artifacts. A preview can be rendered before final assembly; a work plan or scheduled redraw alone is not rendered output. |

## Identity, provenance, and atomic publication

| Fact | Authoritative meaning and lifetime |
| --- | --- |
| Originally requested URL | Complete AH address in AI `FetchRequest::original_url()` and retained response facts; unchanged by redirects, base elements or toolbar updates. |
| Final response URL and redirect history | AI's selected response provenance and actual entered redirect transitions. A failed destination does not gain an invented response URL; contextual failure can retain the last acquired redirect response. |
| Committed document URL | `DocumentAddressState` is prepared from usable selected response provenance before parsing. It accompanies the committed document; supported same-document updates do not rewrite acquired response facts. |
| Document origin | AH computes it for supported creation context; Browser retains it. Same-document changes do not regenerate identity, including an opaque file origin. URL equality and origin equality are distinct. |
| Fallback base | Derived from the document URL for current ordinary managed documents; not another synchronized URL cache. Inherited about contexts and overrides require their actual creation context and are deferred. |
| Selected frozen base | AH4's selected parser key and frozen result. Browser applies ordered canonical DOM/base facts atomically; AH resolves URLs. Invalid first candidates retain their documented fallback outcome rather than choosing a later candidate. |
| Stylesheet attachment identity | Browser `StylesheetSlotId`, associated with a current live document-local owner and authored source. Retirement ends installation authority. IDs are not URLs or CSS precedence. |
| Stylesheet retrieval identity | AH's fragmentless `ResourceIdentityKey` for retrieval/correlation. Equal retrieval keys do not merge attachments or establish cache reuse. Complete selected references retain query/fragment facts. |
| Stylesheet response base | AH8's parsed admitted response URL retained on the external attachment, independently of authored href and issued request. Missing/invalid provenance remains explicit; no document/request fallback is fabricated. Inline sources use their documented document context. |
| CSS source identity and order | `StylesheetSourceId` is CSS provenance; `StylesheetOrder` is sparse input order. Neither is a document identity, retrieval key or substitute for CSS cascade precedence. CSS cascade origin and parser origin metadata are also distinct from security origins. |

The [HTML document-base rules](https://html.spec.whatwg.org/multipage/urls-and-fetching.html#document-base-urls)
and [base-element rules](https://html.spec.whatwg.org/multipage/semantics.html#the-base-element)
are the standards authority; AH4 defines the implemented UTF-8 ordinary-document
subset. AJ does not replace temporal freezing with a final-tree or head-metadata
scan. An unchanged attachment keeps its selected address across base-only changes;
an external loaded sheet keeps its response base. New/changed sources use the
applicable committed context.

Production integration is in [navigation](../../crates/browser/src/tab/nav.rs),
[HTML response admission](../../crates/browser/src/tab/html.rs),
[event/publication handling](../../crates/browser/src/tab/events.rs),
[document addressing](../../crates/browser/src/page/addressing.rs) and
[DomStore](../../crates/browser/src/dom_store/mod.rs). Required invariants are:

- Prepare and enqueue an admitted navigation request before committing its
  navigation mutations. Queue acceptance is not response admission or transport
  success; current eager page reset remains an explicitly incomplete boundary.
- Admit events by tab/generation, route and actual issued provenance. CSS also
  requires the live slot and its request record. A final/equivalent URL spelling
  is not an alternative chunk correlation key. Duplicate response starts cannot
  replace admitted facts or reopen a terminal document.
- Stage DOM changes, ordered base changes, identities, mode and version checks
  before committing. Discover/schedule resources only from accepted publication.
  Rejected/stale publications cannot publish speculative addresses or ownership.
- Preserve separate `PatchKey`, DOM `Id`, handle/version, request generation,
  stylesheet slot/source and retained-render identity domains. Matching numeric
  IDs after replacement establish no continuity.
- Retire attachments on removal/source change, Clear, replacement or navigation
  under their original request generation. `CssAbort` releases CSS buffers;
  generation-wide network/parser cancellation is a separate operation. Late
  events cannot resurrect retired attachments.
- Retain the last committed preview after a fatal parser/document failure, with
  its identity/address state and permitted completed styles. Failure remains
  latched; retained preview availability is not successful static assembly.

## Supported document sources

| Source | Existing capability and explicit limit |
| --- | --- |
| HTTP / HTTPS | AI's admitted GET, redirect, response and bounded body path. This is a qualified Fetch subset, not complete navigation, authentication or security-policy processing. |
| Local `file:` | AI's separate local loader uses AH native-path conversion. Results have local MIME facts and no fabricated HTTP status/headers. File origin identity is not filesystem authorization or universal platform qualification. |
| Supplied fixture responses | Tooling supplies controlled response facts/bytes at production boundaries; AI fake networking exercises shared HTTP Fetch. These are deterministic test inputs, not new public document schemes or alternate parsers/renderers. |
| Standalone parser text/bytes | AE can parse without managed URL context. That alone is not an addressed Browser page load, origin, resource-fetch permission or completed assembly. |
| `about:`, `data:`, `blob:`, nested/srcdoc documents | No general production document loader/creation context is established here. AH representability is separate from loading capability; do not infer inherited bases or blob-entry origins. |
| `javascript:` and other unsupported operations | No execution or implicit scheme handler. Request/creation rejection stays distinct from URL syntax failure. |

## Overlapping lifecycle and static input settlement

The data dependencies are address → AI request/response → character input → AE
document → stylesheet inputs → AF style → AC rendering. This explanatory order
does not prescribe serial execution. Document bytes stream into the parser;
accepted partial publications discover/reconcile stylesheets; stylesheet I/O
overlaps further parsing; DOM and installed CSS can invalidate preview frames.
Each accepted publication can replace earlier dependency membership/order.

| Boundary | Existing evidence and what it does not prove |
| --- | --- |
| Document network completion | `NetworkDone` validates immutable response/body accounting and sends `ParseHtmlDone`. It does not prove parser finalization, commit, stylesheet settlement or rendering. |
| Parsing completion | `HtmlParseFinished` follows parser finalization and publications. Browser accepts success only for its committed handle/version/mode and existing DOM. Parser success alone is not Browser commit acknowledgement. |
| Stylesheet network completion | `StylesheetLoadPhase::AwaitingRuntime` can still await decoded text, parsing/installation and runtime completion. Receiving the last byte does not settle the attachment. |
| Static input settlement | Required document/dependency conditions below; it is not currently a public completion API or another enum value. |
| Computed style / rendering | CSS execution and AC artifact validation/execution use the current inputs. A plan, old cache, loading flag or submitted redraw is not an accepted current frame. |

### Dependency membership

Keep these distinctions visible in architecture and future diagnostics:

| Classification | Contribution to settlement |
| --- | --- |
| Discovered authored reference | An AH7/AH8 observation. It is not automatically selected, scheduled or assembly-blocking. |
| Eligible stylesheet dependency | A current supported stylesheet-bearing occurrence selected under the implemented loading/applicability rules, retaining its source and ordered outcome. Invalid/unavailable sources can settle without a request. |
| Scheduled/pending request | An actually issued current attachment request; admission, transfer and runtime installation must reach a permitted terminal outcome or be safely retired. |
| Loaded attachment | Has available CSS parse/provenance; its lifecycle must also complete. CSS may classify its rules inactive or unsupported. |
| Failed/aborted attachment | Retains a terminal dependency outcome and order; contributes no active parse. Recoverable failure can permit assembly. |
| Excluded dependency | A supported rule has established exclusion from the selected set. No eligible outstanding installation may later change the settled input set. Exclusion is not fabricated success or fetch failure. |
| Unsupported/deferred behavior | Explicitly unevaluated or unimplemented capability. It neither promises standards applicability nor creates work for a nonexistent loader. |

Only supported rules justify exclusion. In particular, existing discovery does
not filter disabled/alternate sheets or nonempty media before scheduling. CSS's
inactive media classification is not evidence that the associated request was
never issued. Settlement must account for actual current attachment work or a
concrete exclusion/retirement that prevents later installation; AJ1 changes
neither path. Deferred media evaluation itself introduces no assembly wait.
Unsupported `@import`
creates no nested fetch dependency; inert scripts, image/font/media resources,
preloads and other deferred capabilities are outside the static stylesheet set.
They do not implicitly delay its assembly, even where other existing Browser
resource work runs independently.

### Assembly completion conditions

**Required architecture:** final static inputs are settled for the current
committed document when all of the following hold:

1. The main response/body has the applicable accepted outcome, with no fatal
   document failure, and successful parser termination agrees with the committed
   handle, version and mode. No more parser publications belong to that result.
2. Stylesheet reconciliation reflects that final accepted document and its
   selected attachment membership, source context and document order.
3. Every selected dependency has a terminal installed/parsed contribution,
   recoverable failure, or supported exclusion. Invalid URLs/unsupported
   retrieval can settle as noncontributing outcomes without dispatch. Skipped
   deferred features are reported separately, without speculative work.
4. No selected attachment still awaits request admission/delivery, transfer,
   decoding, parsing, runtime completion or an eligible installation that could
   change these inputs. Terminal/retirement guards prevent late installation.
5. The ordered available stylesheet inputs and current element-attached style
   attributes are ready for the existing CSS-backed render preparation.

This permits assembly with some or all external sheets failed, and with
recoverable HTML/CSS diagnostics. It does not equate unsupported behavior with
standards success, suppress fatal style errors, or require an empty global work
queue. It does not require another eager computed-style pass. A rendered final
result additionally requires accepted current rendering; intermediate previews
remain allowed before settlement.

`DocumentParseStatus` and the loading/pending counters remain their existing
implementation, not proof that every condition or response policy above is
implemented. No `Document.readyState`, DOMContentLoaded, load event or browser
event-loop semantics are claimed.

## Document response handling and character input

The [Fetch response model](https://fetch.spec.whatwg.org/#concept-response)
defines response facts separately from higher-level document processing.
[HTML document loading](https://html.spec.whatwg.org/multipage/document-lifecycle.html#loading-html-documents)
connects fetched input, parser processing and EOF. Borrowser uses existing Rust
workers and events; this does not implement HTML's task/event-loop machinery.

AI interprets response headers and retains declared MIME/charset/length facts.
Document Loading uses those typed facts to decide document applicability and
coordinate parser input. It must not split raw HTTP fields again, implement
redirects, equate `HttpStatus::is_success()` with page success, or infer complete
representation from transfer completion. Local-file responses remain non-HTTP.

| Case | Standard semantics / current subset and gap | Future responsibility |
| --- | --- | --- |
| HTTP error status | AI delivers 404/500 as ordinary response facts; body completion and consumer acceptance remain independent. Current HTML admission does not establish complete status/MIME applicability. | Document Loading decides whether/how a response produces a document, retaining status and diagnostics. No blanket 2xx-only rule is introduced. |
| HTTP 204/205 | [HTML navigation](https://html.spec.whatwg.org/multipage/browsing-the-web.html#navigate) preserves the active document for these responses. AI's null-body/framing rules do not implement that effect; current eager navigation reset does not preserve the old page. | Document Loading/Browser response and activation lifetime, without moving navigation policy into AI or claiming history completeness. |
| HTTP 304 | [HTTP semantics](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.4.5) describe a conditional response without a new representation body. AI can retain metadata and complete its body contract; current document applicability/revalidation is incomplete. | Document Loading owns non-installable response handling; future cache owns validated representation reuse/merging. No empty HTML substitution. |
| HTTP 206 | [Partial-content semantics](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.3.7) remain distinct from a complete representation. AI transfer completion cannot establish full document applicability. | Document/stylesheet consumers own acceptance; future range/cache capability must supply actual representation context. |
| Missing/unsupported content type | AI8 preserves Missing/Empty/Unusable/Parsed and declared charset facts without fabricating defaults or sniffing. Existing HTML response admission has no complete MIME selection/rejection policy. | Document Loading owns document applicability and explicit limitations; Fetch retains header interpretation. Full sniffing is not implemented here. |
| Empty body and body failure | A completed empty transfer, failed transfer and parser EOF are different outcomes. AI retains byte bounds/completion/errors; consumer and parser behavior remains separately observable. | Document Loading defines the resulting document outcome under supported response policy; no success inferred from zero bytes or declared length. |

Character encoding is another boundary. The standards authority is
[HTML character-encoding determination](https://html.spec.whatwg.org/multipage/parsing.html#determining-the-character-encoding)
and [CSS input decoding](https://drafts.csswg.org/css-syntax-3/#input-byte-stream).
The implemented subset uses incremental UTF-8 input/diagnostics through
[AE's parser facade](../../crates/html/src/parser/session.rs) and
[shared decoding helpers](../../crates/tools/src/utf8.rs); external stylesheet
text assembly lives in [runtime_css](../../crates/runtime_css/src/lib.rs).
AE owns subsequent HTML preprocessing, tokenization and tree construction.

AI's HTTP content decompression is not character decoding. A declared charset
is not an encoding decision. Document Loading coordinates supported text-input
policy and diagnostics using existing parser/decoder boundaries, preserving
AE's byte and already-decoded text entry points without decoding twice. Current
metadata transport does not establish charset-label selection, legacy decoding,
conflict resolution, full BOM sniffing/switching or meta-charset scanning/restart.
Those require explicit future document/CSS input policy and qualification, not
a second decoder or an assumed always-valid UTF-8 string. AJ1 changes none of
the existing invalid-input or EOF behavior.

## Stylesheet discovery, ordering, and installation

Existing production discovery in
[DocumentStyleSet](../../crates/browser/src/document_style.rs) visits ordinary
HTML DOM order, excluding template contents and foreign lookalikes. It collects
`link` elements whose rel-token helper recognizes stylesheet and whose href
passes its existing nonempty/preprocessing rules, and concatenates text children
of HTML `style` elements. Inline style attributes remain CSS element inputs.
AH7's authored inventory has a different purpose and some different preprocessing;
it cannot silently replace loader discovery or become scheduling authority.

The handoffs are:

1. Browser discovers occurrences and reconciles owner/source attachments against
   the committed DOM/base state. Current source changes retire old slots;
   unchanged sources and media-only changes can retain identity and parse.
2. Browser resolves supported hrefs through AH and produces request descriptors.
   Actual AI request/route admission and successful command delivery are separate
   from retained slot membership. Unsupported/invalid sources issue no request;
   dispatch failure settles locally without phantom work.
3. AI fetches external sheets, owning redirects, immutable response metadata,
   body outcomes and typed network failures. Browser authenticates the actual
   issued key, generation, route and attachment.
4. Browser's existing CSS admission consumes AI8's typed metadata. `runtime_css`
   buffers/decodes admitted bytes and emits text/completion. It owns no CSS
   grammar. Browser invokes CSS parsing and installs the parse with admitted
   response provenance, subject to runtime phase/completion guards.
5. Browser projects available current attachments into AF5 collection inputs.
   CSS classifies conditions/rules/declarations and owns matching and precedence.
   Reconciliation/installation requests invalidation through existing CSS/AC
   contracts; it does not compute style inside Fetch or HTML parsing.

Browser slot identity, CSS source identity, sparse stylesheet order and cascade
priority stay distinct. UA defaults are an explicit UA-origin sheet; external
author links and inline `style` blocks interleave in document order, not in
separate external-first or inline-last groups. Pending/failed slots retain
their positions; available CSS inputs preserve the resulting sparse order.
Duplicate URLs/content retain independent slots and CSS source IDs. Element-
attached style attributes retain AF6 precedence rather than becoming another
sheet or synthetic specificity. [CSS cascade ordering](https://drafts.csswg.org/css-cascade-5/#cascade-sorting)
is the authority for semantic precedence; AJ preserves the AF supported subset
and does not recreate its priority comparison.

### Standards, current limitations, and future ownership

Standards references here identify semantic authority rather than copy algorithms:
[HTML stylesheet links](https://html.spec.whatwg.org/multipage/links.html#link-type-stylesheet),
[link processing](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element),
[style elements](https://html.spec.whatwg.org/multipage/semantics.html#the-style-element),
[CSS stylesheet parsing](https://drafts.csswg.org/css-syntax-3/#parse-stylesheet)
and [CSS imports](https://drafts.csswg.org/css-cascade-5/#at-import).
These references were checked on 2026-11-22. Current limitations below are not
permanent normative loading rules.

| Behavior | Existing implemented subset / incomplete behavior | Future responsibility |
| --- | --- | --- |
| Media attributes | Browser preserves exact optional media text. AF5 treats missing/empty/ASCII-whitespace-only as active; all other values, including `all` and `screen`, are deferred/inactive. Discovery can still issue their requests. Standards use media-query matching, not this fail-closed approximation. | CSS owns media parsing/evaluation; Browser supplies environment and coordinates selection/lifetime. No media evaluator is added. |
| Disabled/alternate/title selection | HTML defines stylesheet selection/disabled behavior. Current discovery does not enforce those semantics and may load/apply an otherwise qualifying link. A rel token is not proof of correct selection. | Browser document/stylesheet selection with CSS applicability inputs; classify incompleteness instead of claiming disabled/alternate support. |
| MIME applicability and status | Current CSS policy accepts Missing/Empty or parsed `text/css`, rejecting Unusable/other MIME. This compatibility rule and independent HTTP-status/range gaps are not complete HTML stylesheet applicability/security processing. | Browser stylesheet consumer owns applicability/recovery; AI keeps metadata interpretation. |
| Empty external CSS | With no forwarded body, the runtime emits no decoded block; current slot completion supplies no loaded contribution and settles it as failed. CSS itself can parse empty input; this loader outcome is an existing limitation, not a syntax requirement. | Stylesheet loading/installation policy, separate from CSS grammar. |
| Invalid CSS | CSS parsing/recovery retains diagnostics and usable supported content. Unsupported/invalid declarations are not Fetch failures. Resource/invariant/style-execution errors are not silently converted into empty/default CSS. | CSS owns recovery/classification and explicit execution failure; Browser reports and propagates results. |
| `@import` | AF5 skips it; AH8 can observe authored import references in an unsupported context without following/resolving them as dependencies. Standards import semantics are not implemented. | Concrete future stylesheet loading plus CSS ordering/condition support; no recursive loader or assembly wait is introduced. |
| Other conditions/at-rules | `@media`, `@supports`, `@layer`, CSS `@scope` and unknown rules retain typed skipped/deferred records; preserved blocks are not flattened into active declarations. | CSS owns semantics; Browser extends only actual context/lifecycle requirements. |
| Resource-related attributes | Current discovery does not establish crossorigin/integrity/referrerpolicy enforcement. Retaining origin or observing an attribute is not policy support. | Actual security/integrity/referrer-policy capabilities with captured context, outside AJ1. |

## Assembled inputs and retained rendering

The existing production handoff is:

```text
committed DOM + document mode + ordered available UA/author stylesheet inputs
  + element-attached style attributes
  -> PageState retained style preparation
  -> CSS matching/cascade / ResolvedDocumentStyle
  -> CSS computation / ComputedDocumentStyle
  -> borrow-backed StyledNode / StylePhaseOutput
  -> AC frame preparation/work planning and retained artifact checks
  -> Layout / Paint execution or qualified artifact reuse
  -> recorded current-frame outcomes and optional AG observation
```

This shows semantic inputs/outputs, not a new scheduler or an assertion that
every pass rebuilds each artifact. The entry points remain
[`prepare_style_phase_for_frame`](../../crates/browser/src/page/style_phase.rs),
the [retained state](../../crates/browser/src/page/retained_render_state.rs),
[`prepare_page_frame` / `execute_prepared_page_frame`](../../crates/browser/src/rendering/frame.rs)
and [work-plan contracts](../../crates/browser/src/rendering/work_plan.rs).

AJ supplies coherent document/style inputs and coordinates their lifetime.
Existing preparation invokes CSS or reuses qualified retained results; AJ does
not move computation into a new pre-render loader stage. CSS owns resolved and
computed artifacts and the exact borrowed style view. Browser retains them and
their keys; Layout owns used geometry, and Paint owns paint semantics/artifacts.

Style keys, matching-environment compatibility, dirty state, retained identity
domain and layout/paint keys remain the reuse authority. Stylesheet changes flow
through existing CSS-owned invalidation decisions. New document identity retires
old continuity. A plan does not prove reuse or execution; actual accepted results
are recorded through AC. The same render path can produce previews before input
settlement and final frames afterwards. No duplicate DOM/style cache, retained
`StyledNode` wrapper, alternate renderer or new render-ready API is introduced.

## Outcomes and failure isolation

These are architectural outcome distinctions, not a new aggregate result model
or a claim that all current consumer policies are complete.

| Outcome | Meaning and required isolation |
| --- | --- |
| Successful transport | AI completed its admitted body contract. It proves neither acceptable document content nor accepted parser/style/render execution. |
| Accepted document response | Browser admitted current response provenance/creation context and applicable document policy. Existing provenance admission is implemented; complete status/MIME policy remains incomplete. |
| Rejected/non-installable response | A consumer outcome distinct from network failure. It can require preserving the old document or declining replacement; do not collapse all such cases into fatal new-page failure or fabricate a response body. |
| Fatal document load failure | Main acquisition, required decoding/parsing, or publication cannot produce a successful current document result. Existing failure latching/cancellation and retained-preview rules remain; no later sibling completion erases the failure. |
| Recoverable stylesheet failure | One selected attachment fails acquisition/admission/installation or is safely aborted. Settle that attachment, keep diagnostics/order, omit its active contribution and continue supported document assembly. Do not cancel siblings or retry against a guessed URL. |
| Assembled with failed dependencies | Final document/input settlement succeeds with explicit noncontributing outcomes. No requirement for every external stylesheet to succeed, and no claim that unsupported features ran. UA/default and remaining valid author inputs can still participate. |
| Style/render execution failure | Input settlement did not guarantee CSS resource/invariant execution, Layout, Paint or frame acceptance. Propagate the owning failure; do not report rendered output from stale caches or substitute silent default styles. |

Recoverable HTML/CSS syntax diagnostics are different from parser fatal errors,
publication invariant violations and CSS execution/resource failures. A network
error before response selection has no fabricated response/status/final URL.
Accepted immutable Start facts remain the authority at completion and contextual
failure. A document can fail after a preview has rendered; the preview remains
evidence only of that earlier accepted state.

## Diagnostics, snapshots, and AG

**Existing implementation:** [Tab lifecycle state](../../crates/browser/src/tab/state.rs)
and response/stylesheet event handlers retain admission and progress facts;
[`document_address_debug_snapshot`](../../crates/browser/src/tab/mod.rs)
exposes committed addressing plus AH inventories. CSS and AC supply their own
semantic debug surfaces. These are internal regression contracts, not public
APIs or cross-engine identity schemas.

**Required architecture:** loading diagnostics must make request versus final
provenance, response metadata categories used, decode decision/outcome, accepted
parser result, document/base/origin state, dependency selection, actual dispatch,
fetch/parse/install outcomes, attachment order and input settlement distinguishable.
They must distinguish failed, excluded, pending and unsupported work and expose
which existing CSS/AC work/result corresponds to the current inputs. This is a
diagnostic requirement, not a claim of a complete existing AJ snapshot.

Use canonical document/attachment order, stable outcome labels and bounded,
redacted metadata. Never serialize hash iteration, pointers, elapsed time or
raw private URL/body/header/charset contents as a default loading contract.
Snapshot-local labels must not become runtime identity. Controlled semantic
fixtures can assert exact addresses separately from default redacted debug.
Observation cannot schedule requests, mutate state or repair provenance.

[AG's execution boundary](../../crates/conformance/src/execute.rs) uses Tab,
the production parser/publication path and actual render execution. The phase
harness currently rejects external subresource commands; AI9's fake-networking
fixtures separately exercise production Fetch. Neither is already a complete
external-stylesheet assembly harness. Future integration must connect existing
production seams, retain independent expectations and explicit unsupported/error
results, and observe settled current inputs/accepted output without recreating
engine logic. No AG category, schema, fixture or executable test changes in AJ1.

[`render_observed_frame`](../../crates/browser/src/tab/observation.rs) already
requires parser completion, committed state, no outstanding loading, unchanged
document identity, frame acceptance and valid current retained artifacts. It
does not turn the ordinary preview renderer into a final-assembly gate or prove
all future admission/dependency policies. AG's selected style/layout/canvas
observations keep their documented limits; a passing snapshot is not general
visual or browser conformance.

## No-JavaScript scope and non-goals

Scripts can be parser-created nodes and AH7 inert authored references. Script
classification or an address's retrieval capability does not authorize scheduling.
AI's current route admission does not provide a Script execution route. In AJ,
inert representation means observation only: no script fetching for execution,
module graphs, execution, parser-blocking scripts or `document.write`. No
implicit inert-script download is introduced by this contract.

Milestone AJ also excludes dynamic DOM mutation, complete navigation/history,
multiple browsing contexts/tabs as platform semantics, HTTP cache, cookies,
CORS/CSP/mixed-content enforcement, image/font/media loading or decoding,
service workers, preload scanners/preload/prefetch and a full browser event loop.
Document readiness and DOMContentLoaded/load events remain unimplemented.
Existing UI/history/image paths and parser-generated patches are not removed;
their presence does not establish complete platform capabilities or make them
static stylesheet dependencies. Rust worker/channel orchestration is not a
browser event loop.

AJ1 additionally excludes executable changes, refactoring, dependencies, new
types/interfaces, AG schema/fixture changes and unnecessary tests. Its outcome
is an independently reviewable architecture contract, not a loading milestone
implementation or authorization to close other issues.

## Concrete extension points

| Future capability | Existing seam and owning responsibility |
| --- | --- |
| Document response/activation lifetime | Browser navigation preparation and HTML response admission, preserving AI facts, current document association and atomic publication. Address 204/205 and other non-installable responses with actual lifetime semantics. |
| Character encodings | Existing AI metadata plus AE byte/text input and decoder diagnostics. Document/CSS input policy must qualify supported labels, precedence and any reparsing without duplicate decoding or speculative parser APIs. |
| Broader stylesheet selection/imports | Current attachment reconciliation, AF conditions and source coordinates, AI requests and admitted response bases. Add only supported selection/ordering/dependency rules with terminal and retirement semantics. |
| Image/font/media lifecycle | AH addresses, actual AI route capability and existing Browser resource/invalidation boundaries. Each capability needs concrete format, installation and lifetime behavior; no generic resource framework follows from this contract. |
| Cache/cookies/security | Existing Fetch coordination, request context, origins, response facts and retrieval identity inputs. Real reuse keys, storage, revalidation/304 merging and policy enforcement belong to those capabilities, not AJ's assembly predicate. |
| Future navigation or scripting | Existing document identity/publication and retained invalidation seams. Actual activation, readiness, mutation, task and security semantics require separate justification; no placeholder hook or mode is added. |
| Assembly conformance | Production Fetch fixtures, parser publications, attachment installation and accepted-frame observations. Extend actual supported behavior without a second executor, weakened expectations or inflated coverage. |

These are responsibility boundaries, not proposed phase-based issues or promises
of new abstractions. The [feature-gap tracker](../engine-feature-gap-tracker.md)
continues to record incomplete loading and platform behavior.

## Requirement and exit coverage

| #1132 requirement / exit criterion | Contract evidence |
| --- | --- |
| AJ architecture document | This dedicated contract and its architecture overview link; implementation status is explicit. |
| Document Loading owns all nine requested responsibilities | Ownership table, publication/lifecycle, response handling, dependency settlement and diagnostics sections. |
| AH / AI / AE / CSS / Browser ownership explicit | Ownership table and authoritative foundation references; no semantic transfer or new implementation layer. |
| All ten required artifact distinctions | Artifact vocabulary, refined by identity/provenance and outcome tables. |
| End-to-end assembly and AC handoff | Overlapping lifecycle, settlement conditions, ordered AF inputs and existing retained render preparation/execution. |
| Supported document load sources | Source/capability table; addressed loading, tooling and standalone parsing are distinct. |
| No-JS scope | Inert script representation grants no scheduling/execution; parser blocking and document.write are excluded. |
| Every listed non-goal | Dedicated exclusions, without removing existing behavior or claiming platform completeness. |
| Relationship to AC, AE, AF, AH, AI and AG | Linked contracts and actual production handoffs/observation limits. |
| Future lifecycle extension points | Concrete existing seams, owners and required semantics; no speculative framework/API. |
| Closeable without loading implementation | Documentation-only outcome; tracker records contract scope, while implementation, conformance and milestone completion remain separate. |

AJ5 candidate diagnostic transport and passive parsed-document observations are
specified in [AJ5](aj5-parsed-document-and-parser-diagnostics.md). A successful
parser terminal requires present finalized diagnostic evidence independently of
capture completeness. This does not complete later stylesheet discovery or full
static page assembly, and remains subject to independent implementation review.
