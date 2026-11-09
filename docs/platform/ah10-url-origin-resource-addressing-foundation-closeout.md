# AH10: URL, Origin, And Resource Addressing Foundation Closeout

Issue: #1120. Milestone AH — URL, Origin, and Resource Addressing Foundation.
Evidence baseline: master `b0e617eda1c6142c91cff3a3cc89ea538f4aa958`.
Scope: implemented AH1–AH9 addressing foundation for the documented subset.
GitHub publication, revision qualification, and issue/milestone closure are
separate decisions; this document records implementation scope and dated evidence.

AH1–AH9 provide an explicit, deterministic addressing foundation for the subset
below. AH10 consolidates its contracts and evidence without changing Rust, APIs,
runtime behavior, fixtures, dependencies, CI, or resource loading. Foundation
completion does not establish complete networking, WHATWG Fetch, caching,
navigation, security enforcement, scheme support, or standards conformance.
In particular, typed addressing is available to future consumers; current
transport still accepts strings. The networking exit criterion is qualified
explicitly under [integration boundaries](#integration-boundaries-and-the-networking-exit-criterion).

## Ownership and authoritative contracts

| Owner | Implemented responsibility and authority |
| --- | --- |
| Addressing | Immutable URL parsing, normalization/serialization, explicit-base resolution, diagnostics, origins, same-origin comparison, operation capabilities, and retrieval identity. [AH1](ah1-url-origin-resource-addressing-contract.md), [AH2/AH3](ah2-typed-url-parsing-and-diagnostics.md), [AH5](ah5-origin-representation-and-same-origin.md), [AH6](ah6-resource-addresses-and-identity-keys.md). |
| HTML/parser | Parsed names, namespaces, attributes, text, and tree relationships supplying authored references. Browser traverses committed DOM for the bounded HTML inventory. [AE2](../html5/ae2-parser-created-dom-node-model.md), [AE10](../html5/ae10-template-tree-construction-contract.md), [node identity](../html5/node-identity-contract.md), [AH7](ah7-html-resource-reference-discovery.md). |
| CSS | CSS syntax/escapes, authored reference production, source spans, shape/context, property acceptance, cascade, and computed style. [AH8](ah8-css-resource-reference-discovery.md), [AF5](../css/af5-stylesheet-rule-collection-source-order-contract.md). |
| Browser | Document/base state and origin lifetime, committed publication, source/attachment associations, issuance, response admission/provenance, and invalidation. [AH4](ah4-document-url-and-base-state.md), AH5–AH8. |
| Networking/runtime | Existing transport, request execution, cancellation flags, and streaming response events. It does not select document bases or own origin semantics. |
| Layout / Paint | Geometry and visual output respectively; neither owns URL parsing, reference discovery, or policy decisions. |
| Conformance | Production observations, independent fixture expectations, comparison/reporting, and deterministic internal snapshots. [AH9](ah9-addressing-conformance-fixtures-and-snapshots.md), [AG5](../conformance/ag5-phase-comparison-projections.md), [AG6](../conformance/ag6-fixture-discovery-and-metadata.md), [AG7](../conformance/ag7-focused-external-tests-and-references.md), [AG8](../conformance/ag8-deterministic-reporting-and-suites.md). |
| Future cache / security | Compose complete cache identities or authorization decisions from addressing plus their own request, response, partition, credentials, and policy context. No such policy follows from address equality or same-origin alone. |

AH1 records original architectural decisions and unresolved questions. Later AH
contracts refine those decisions; their dated failures and superseded dependency
choices remain historical evidence rather than current implementation claims.

## URL parsing, diagnostics, and explicit-base resolution

[`BrowserUrl`](../../crates/addressing/src/lib.rs) wraps the maintained URL parser
behind immutable Borrowser-owned values. `parse()` has no base or address-bar
guessing. Accessors expose normalized scheme, host kind, credentials, nondefault
port, path, query, fragment, authority, and opaque-path state. `as_str()` provides
complete semantic serialization; URL equality/hash use that complete value.
Absent and empty query/fragment components remain distinct where represented.
Query order/repetitions and credentials remain significant. Normalization does
not perform DNS, filesystem canonicalization, or consumer-specific trimming.

[`UrlParseOutcome`](../../crates/addressing/src/lib.rs),
[`UrlFailure` and `UrlDiagnostic`](../../crates/addressing/src/diagnostics.rs)
separate successful parsing with recoverable callbacks, fatal input/base errors,
and capacity/backend failures. Events retain observed order and repetitions,
including events before failure. Unsupported resource operations are separate
capability outcomes, not fabricated syntax failures. Diagnostics have no source
positions and are not a complete WHATWG validation taxonomy; silence is not
proof of conforming input. `format_resolution_debug()` reports structure and
typed outcomes while withholding authored contents; it does not reconstruct
exact transformations or prove a successful result's input provenance.

`resolve()` uses an explicit typed base without mutating it or replaying its
construction diagnostics. Coverage includes relative/root/scheme-relative paths,
dot segments, absolute overrides, query, fragment, empty references, and opaque
base restrictions. `parse_if_base_independent()` is parser-owned proof for an
unavailable base: Some is an independent result, Err an independent failure,
and None means unresolved. Consumers do not guess an absent base, install a
dummy base, or infer independence from a scheme prefix.

The current main/fuzz Cargo source is the [approved local snapshot](../../vendor/rust-url/BORROWSER-PATCH.md)
of maintained-parser revision `92f7fb395f9ffb2f706e7c664283781fd3b294a2`, checked by the frozen
[dependency policy](../../tools/ci/check_url_dependencies.py). AH2's published
2.5.7 decision and AH3's earlier fork pin are historical. The qualified consumer
graph uses adapter 1.2.1 / ICU 2.0, with the same approved 35-package closures.
The AH2/AH3 and AH8 contracts retain source qualification and the modern-graph
IDNA/setter limitations; this closeout does not qualify all library behavior.

## Document URL, fallback, and active/frozen base

Browser's [`DocumentAddressState`](../../crates/browser/src/page/addressing.rs)
retains document URL and origin. For ordinary HTTP/HTTPS/file response documents,
the fallback base is the document URL; the active base is the selected frozen
base or that fallback. Missing managed state does not manufacture about:blank.
[`on_html_network_start`](../../crates/browser/src/tab/html.rs) prepares addressing
before acceptance mutations using nonempty final response URL, otherwise the
existing requested-URL compatibility fallback. An invalid nonempty final URL
does not fall back. This differs from external CSS's stricter provenance below.

[`BaseObservation`](../../crates/browser/src/dom_store/base.rs) supplies ordered
structural facts. Browser selects the first ordinary-tree HTML `base` with an
unnamespaced `href`, including outside head, excluding foreign lookalikes and
template contents. Target-only elements do not win href selection. Resolution
uses the fallback, not the previous active base. Empty/relative hrefs use the
existing resolver; an invalid first href or a data/javascript result freezes
the fallback with a distinct outcome rather than trying a later base. Capacity
or backend failures reject publication instead of becoming authored-input verdicts.

Selection/href transitions freeze results; unrelated changes do not. A document
URL update changes its derived fallback without refreezing an unchanged selected
element. Browser [stages DOM and addressing together](../../crates/browser/src/tab/events.rs)
before publication; stale/rejected publications cannot publish speculative bases.
Existing image and stylesheet associations retain their selected addresses until
their own source/owner lifetime requires retirement. Current-base inventory does
not retroactively retarget them. Same-document navigation retains origin and
frozen state; cross-document loads have their own lifecycle.

This is the UTF-8, publication-boundary model in AH4, not full HTML setter,
microtask, image, or stylesheet scheduling. Inherited about bases, base overrides,
sandboxed creation, non-UTF-8 URL encoding, base target, and CSP hooks remain
unsupported. No public dynamic DOM or CSSOM loading semantics are implied.

## Origins and same-origin comparison

[`Origin`](../../crates/addressing/src/origin.rs) represents normalized tuples
for http/https/ftp/ws/wss and identity-bearing opaque origins. Tuple comparison
uses scheme, canonical host, and normalized port; credentials, path, query,
fragment, and active base do not participate. The domain field remains null;
document.domain and same-origin-domain relaxation are unsupported.

Independent opaque determinations are fresh; Clone preserves identity. All opaque
origins serialize as `null`, which cannot reconstruct or compare their identity.
Browser retains the origin chosen for each admitted document instead of
recomputing it. File policy is a fresh opaque origin per new load, retained through
same-document transitions. It establishes no filesystem access permission.

URL-origin determination is distinct from document creation. Without a blob entry,
the existing fallback parses the embedded URL and uses only http/https/file origin
cases; other/invalid cases are opaque. Associated blob entries, blob storage/loading,
inherited about documents, and sandbox context are unsupported. Browser's ordinary
response admission accepts only http/https/file contexts. Same-origin is a tested
classification, not CORS, CSP, mixed-content, storage, or access authorization.

## Resource addresses, capabilities, and identities

[`ResourceAddress`](../../crates/addressing/src/resource.rs) holds a complete
`BrowserUrl` and producer-known `ResourceKind`. Construction performs no I/O,
origin computation, or permission check. `retrieval_key()` derives
`ResourceIdentityKey` from serialization through the query, excluding fragment
without reparsing or mutation. Keys retain query order, repetitions, credentials,
and absent/empty query distinctions. Purpose and requesting origin are not key
inputs; Rust hash values are not persistent resource IDs. ResourceAddress itself
implements neither Eq nor Hash; consumers must choose the relevant identity.

| Scheme | Retrieval capability | Navigation capability |
| --- | --- | --- |
| http / https / file | Supported | Supported |
| about | Unsupported | Reserved |
| data / blob | Deferred | Deferred |
| ws / wss | Reserved | Reserved |
| ftp / javascript / unknown | Unsupported | Unsupported |

This is AH's operation matrix, separate from parseability, tuple-origin support,
document admission, transport success, decoder support, and security permission.
In particular FTP tuple origins do not imply FTP loading. Only Supported passes
the integrated stylesheet/hyperlink gates; this is not a universal Fetch gate.

The existing invariants remain:

- Complete URL equality is not fragmentless retrieval identity.
- Retrieval identity is not request identity, response identity, decoded-resource
  identity, stylesheet attachment identity, or a same-document navigation decision.
- Request generation and `StylesheetSlotId` remain separate correlation inputs;
  equal retrieval keys do not merge attachments or authored occurrences.
- Retrieval identity is only an input to a future complete cache key.
- Origin serialization is not origin identity; same-origin is not authorization.
- An authored reference is not necessarily an effective dependency or issued request.
- Address capability classification does not authorize a Fetch operation.

## Authored HTML and CSS reference discovery

[AH7](ah7-html-resource-reference-discovery.md) inventories stylesheet `link[href]`,
`img[src]`, media-parent `source[src]`, `a[href]`, `form[action]`, authored
`iframe[src]`, and inert classic/module `script[src]`. The
[production traversal](../../crates/browser/src/page/html_resources.rs) preserves
ordinary DOM order, duplicate occurrences, source node/attribute, kind, complete
addresses, diagnostics, and failures. Missing attributes produce no occurrence.
Exactly empty form action uses the document URL; empty hyperlinks resolve;
other supported empty references retain an empty outcome. Whitespace is not
universally trimmed by the inventory. HTML source spelling/byte positions are
not recovered from stored DOM attributes. Namespace/template boundaries and
script-purpose classification do not introduce execution, MIME sniffing, frame
loading, responsive selection, or form submission.

[AH8](ah8-css-resource-reference-discovery.md) inventories parsed authored URL
forms before cascade filtering. [CSS](../../crates/css/src/resource_references.rs)
owns decoding and source-bound spans, image/cursor/mask/font/import/unknown roles,
shape, blocking contexts, diagnostics, and completeness. CSS positions refer to
the decoded CSS buffer, not original HTML byte offsets. Ordinary eligible
literals can resolve even for unsupported properties; that does not add property
acceptance or effective participation. Imports, font-face, image-set, custom
properties, substitution, and unsupported functions/rules remain blocking
contexts. Empty/local-fragment/invalid shapes and source incompleteness remain
separate facts. Comments and ordinary strings do not become guessed URLs.

Browser's [CSS outcome integration](../../crates/browser/src/page/css_resources.rs)
uses the external attachment's parsed admitted final response URL, including
missing/invalid provenance. It never substitutes request URL or document base.
Inline styles use current committed document base at observation. Unavailable
external bases permit only parser-proven independent results. Reporting-budget
truncation is not source incompleteness. Inventories/debug calls are observational:
no request, retained dependency graph, cascade decision, or invalidation is added.

## Integration boundaries and the networking exit criterion

| Boundary | Actual current behavior |
| --- | --- |
| Typed foundation | `BrowserUrl`, `Origin`, `ResourceAddress`, and `ResourceIdentityKey` are available production values, not placeholder interfaces. Producers/Browser supply the relevant base/source context. |
| Stylesheets | [`DocumentStyleSet`](../../crates/browser/src/document_style.rs) retains complete addresses on external attachments, emits typed retrieval keys, and stores admitted final-response bases. [`Tab::discover_resources`](../../crates/browser/src/tab/discovery.rs) serializes a key into `CoreCommand::FetchStream.url`; `StylesheetLoadState` separately retains the issued key, generation, and phase. |
| Images | [`reconcile_image_sources`](../../crates/browser/src/page/image_sources.rs) retains selected `BrowserUrl` bindings and emits complete URL strings to ResourceManager. [`resolve_image_source`](../../crates/browser/src/resources.rs) has its existing HTML ASCII-whitespace preprocessing. Image cache keys are not universally migrated to AH6 retrieval keys/capability admission. |
| Navigation | [`activate_link`](../../crates/browser/src/tab/nav.rs) resolves a typed destination and checks navigation capability, then passes complete serialization to existing navigation. Address-bar normalization, history, and document issuance retain their existing string machinery. |
| Transport | [`CoreCommand::FetchStream`](../../crates/bus/src/lib.rs) contains `url: String`; [`runtime_net`](../../crates/runtime_net/src/lib.rs) forwards strings to [`net::fetch_stream`](../../crates/net/src/fetch.rs). No typed address or requesting-origin/policy context crosses that API. |
| Response provenance | [`NetworkResponseInfo`](../../crates/core_types/src/lib.rs) carries requested/final URL strings, status, and content type. HTML/CSS admission validates the applicable immutable snapshot and exact issued-request correlation; CSS runtime progress remains separately validated. |

### Project-owner acceptance decision (2026-11-05)

The original AH10 exit criterion remains **“Future networking receives normalized
typed resource addresses”**. On 2026-11-05, the Borrowser project owner explicitly
approved interpretation A in the AH10 review conversation:

> Milestone AH is responsible for providing normalized, typed URL, origin, and
> resource-addressing foundations that future networking consumers can use. It is
> not responsible for completing the end-to-end typed network transport interface.

This scope acceptance is grounded in the [original milestone's networking exclusions](https://github.com/joris97jansen/borrowser/milestone/94),
[AH1's future handoff contract without a bus migration](ah1-url-origin-resource-addressing-contract.md#future-networking-cache-and-policy-handoffs),
[AH6 #1116's stable future inputs without fetch/cache implementation](https://github.com/joris97jansen/borrowser/issues/1116),
and the implemented ResourceAddress/ResourceIdentityKey foundation.

The owner decision accepts the intended addressing-foundation scope; it does
**not claim satisfaction of literal end-to-end typed transport receipt**. Current
bus/network APIs still receive URL strings, and typed transport integration
remains incomplete and deferred to AI. No requirement wording or implementation
is silently changed. For durable GitHub traceability, the eventual PR description
or issue discussion must also record this dated owner acceptance and the unchanged
transport limitation. Scope acceptance does not replace hosted qualification of
the final AH10 revision and review before closure.

Existing HTTP/HTTPS/file fetching is real, not absent. The network agent configures
redirects (default limit 10); transport emits final URL/status/content-type and
streaming body events. Browser already has navigation/history and response
admission machinery. These bounded implementations do not establish complete
WHATWG Fetch, redirect chains/policy, response processing, MIME sniffing, or
navigation lifecycle semantics. Networking owns transport execution and
transport-level redirect/response processing responsibilities. Browser owns
navigation/history transitions, document creation/admission, document/base/origin
lifetime, and publication lifecycle. Addressing retains URL parsing, normalization,
origin, and resource-addressing semantics.

## Deterministic evidence and AH9 completion

| Behavior | Existing regression evidence |
| --- | --- |
| URL values, diagnostics, resolution | [`browser_url.rs`](../../crates/addressing/tests/browser_url.rs), [`qualification.rs`](../../crates/addressing/tests/qualification.rs), [`unavailable_base.rs`](../../crates/addressing/tests/unavailable_base.rs), [`resolution_debug.rs`](../../crates/addressing/tests/resolution_debug.rs). Pinned discovery covers 560 null-base and 336 explicit-base outcomes, separate from diagnostic completeness. |
| Origins / same-origin | [`origin.rs`](../../crates/addressing/tests/origin.rs), [`origin_debug.rs`](../../crates/addressing/tests/origin_debug.rs), and 31 selected pinned origin-serialization cases in qualification.rs. Opaque relationships are tested independently of serialization. |
| Resource identities / capabilities | [`resource_address.rs`](../../crates/addressing/tests/resource_address.rs), [`resource_address_debug.rs`](../../crates/addressing/tests/resource_address_debug.rs). |
| Document/base lifetime | [`document_addressing.rs`](../../crates/browser/src/tab/tests/document_addressing.rs), [`base.rs`](../../crates/browser/src/dom_store/tests/base.rs), and [`resource_associations.rs`](../../crates/browser/src/tab/tests/resource_associations.rs): ordered transitions, rejection, replacement, navigation/history, and retained ownership. |
| HTML / CSS provenance and inventory | Browser [`html_resources.rs`](../../crates/browser/src/tab/tests/html_resources.rs), [`css_resources.rs`](../../crates/browser/src/tab/tests/css_resources.rs), [`css_provenance.rs`](../../crates/browser/src/tab/tests/css_provenance.rs), and CSS [`resource_references/tests.rs`](../../crates/css/src/resource_references/tests.rs). |
| AG addressing / observations | [`addressing/tests.rs`](../../crates/conformance/src/addressing/tests.rs), [`addressing_observation.rs`](../../crates/browser/src/tab/tests/addressing_observation.rs), [`fixtures/addressing`](../../crates/conformance/fixtures/addressing), and [`fixture_build.rs`](../../crates/conformance/tests/fixture_build.rs). |
| Existing transport, not complete Fetch | [`net/tests/fetch.rs`](../../crates/net/src/tests/fetch.rs), including `follows_redirects_and_reports_final_url`. |

The pinned WPT source/standard revisions, checksums, selections, and licensing
remain in [addressing fixtures](../../crates/addressing/tests/fixtures/wpt).
These populations are representative pinned evidence, not broad WPT integration
or complete URL/origin conformance. No expectation is regenerated for AH10.

AH9's fixed local addressing suite has 22 runnable fixtures / 65 rows and two
unsupported creation contexts. Checked-in structured expectations preserve exact
synthetic inputs/results and local source labels, not unstable runtime IDs. Pure
cases call addressing; document cases observe actual Browser publication and
account for emitted/declared requests through the bounded offline driver. They
do not perform real network I/O or invent successful completion for pending work.
Production Debug privacy and AG5's accepted-frame authority remain unchanged.
Snapshots are internal regression contracts, not public APIs or wire formats.
Chromium/AWS addressing observation remains explicitly unsupported; successful
Linux Chromium qualification proves compatibility of the supported AG paths,
not cross-engine URL/origin/addressing equivalence.

### Hosted qualification verified on 2026-11-04

[PR #1450](https://github.com/joris97jansen/borrowser/pull/1450) is merged and
[AH9 #1119](https://github.com/joris97jansen/borrowser/issues/1119) is closed as
completed. [CI run 41864987854, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/41864987854)
completed successfully for this provenance:

| Identity / gate | Evidence |
| --- | --- |
| PR head / base | `d26f44e732f2413422f57d791f31770fe1b19eaf` / `1cef5bceafd79c872fe6edc9750384e8281a338a` |
| Actual tested synthetic merge | `48322eafaa59f8f534bb59fa6eba03d36b9ded08` |
| Tested tree | `96995c3ff5b10e9ca3960d7a65f481f57ae94f12` |
| Published merge / same tree | [`b0e617eda1c6142c91cff3a3cc89ea538f4aa958`](https://github.com/joris97jansen/borrowser/commit/b0e617eda1c6142c91cff3a3cc89ea538f4aa958); same tree verified from Git commit metadata, not inferred from PR state alone. |
| Rust Ubuntu job | [126696827383](https://github.com/joris97jansen/borrowser/actions/runs/41864987854/job/126696827383): addressing 22 PASS / 2 UNSUPPORTED, AG8 suite, dependency checks, lints, default/HTML5 workspace lanes, parser/golden/build checks pass. |
| Supported native Linux x86-64 Chromium job | [126696827791](https://github.com/joris97jansen/borrowser/actions/runs/41864987854/job/126696827791): native ownership, pinned real Chromium/composition/suite qualification, and profile cleanup pass. Required ignored browser tests were explicitly executed here. |
| Other jobs | AG4 synthesis/snapshots and parser/CSS fuzz/regression jobs pass. The separate special-branch proxy timing job is skipped, not qualified by this run. |

The Rust workspace lanes filter duplicate native_procfs wrappers; the native
ownership qualification runs in the separate required Linux Chromium job.
This hosted workflow is not a newly successful local `make ci` invocation.

The [AH9 local checkpoint](ah9-addressing-conformance-fixtures-and-snapshots.md#local-implementation-validation-checkpoint-2026-11-04)
retains nine reproduced native macOS ownership failures, 184 passing conformance
tests and 13 existing ignores (baseline: 177 passes, same nine failures, 13 ignores).
Its pending conclusion describes that pre-hosted checkpoint. Later Linux success
supersedes pending supported-host qualification; it does not convert macOS failures
to passes or establish macOS Chromium support. That AH9 run qualifies its recorded
implementation tree; it does not establish qualification of an AH10 revision.

## Remaining functionality and future extension points

The following remain partial, deferred, or unsupported:

- Full URL validation diagnostics/positions, complete scheme/context support,
  inherited/override/sandboxed document contexts, associated blob entries,
  public URL mutation APIs, and library-wide dependency qualification. Historical
  ten modern-graph IDNA findings and nine setter findings retain their AH2/AH3
  scope; immutable BrowserUrl does not expose those setters.
- Broad HTML reference coverage, responsive image/media selection, nested
  document loading, form submission, script loading/execution, effective CSS
  dependencies, computed URL-valued properties, imports/fonts/image-set loading,
  substitution, and complete resource-loading timing.
- Complete WHATWG Fetch/redirect/response semantics and MIME sniffing despite
  existing transport, redirect configuration, metadata, and MIME admission paths.
- CORS, CSP, mixed-content policy, cookies, HTTP cache, storage, service workers,
  full security policy enforcement, and JavaScript-facing URL/platform APIs.
- Complete navigation/history/session semantics despite existing machinery;
  broad URL/origin WPT integration and Chromium addressing comparison.
- Existing file loader query/fragment-to-path behavior, potentially stranded
  cancelled image Loading state, legacy raw-URL error/status exposure, and
  per-resource transport cancellation/worker acknowledgements. AH10 fixes none.

| Future work | Existing inputs to extend | Missing context / responsibility |
| --- | --- | --- |
| AI fetch/networking | BrowserUrl, ResourceAddress, operation capability, retrieval key, issued request facts, NetworkResponseInfo | Typed transport integration; full request/response/redirect semantics and request context. Transport must not choose bases or reinterpret authored references. |
| AJ document loading | DocumentAddressState, retained origin, response admission, ordered publication, current history associations | Additional creation/inheritance/override contexts and complete loading lifecycle. Preserve atomic state and opaque identity. |
| AK image/resource loading | Authored occurrences, selected image bindings, stylesheet attachments/response bases, resource keys | Effective dependency/candidate selection, scheduling, decoding, resource-specific fragment handling and cancellation; authored discovery is not a ready-made loading graph. |
| AT cache/cookies/network hardening | Fragmentless identity, issued request/response provenance, existing HttpClientPolicy | Complete cache keys, partitions, methods/headers/credentials, freshness, cookies, response policy, and transport hardening. Existing ResourceManager is not an HTTP cache. |
| Security policies | Retained requesting origin, target address/URL-origin operation, capability facts | Explicit policy and creation context, CORS/CSP/mixed-content/access decisions. Capture needed origin/context at issuance; classification never grants permission. |
| Navigation | Complete URL destinations, document-state associations, retained/fresh origin lifetime | Full navigation/history semantics and loading integration; resource keys and origin equality cannot decide same-document identity. |
| JavaScript platform APIs, if scope reopens | Existing owned URL/origin semantics and deterministic evidence | Bindings, mutable URL/URLSearchParams behavior, observable platform semantics, and missing creation context; no speculative API is added now. |

An implementation phase is not a GitHub issue. An abstraction is not justified
by the possibility that it may be useful later. These extension points identify
existing inputs and missing semantics, not new traits, adapters, generic
frameworks, placeholder interfaces, or additional issues.

## AH10 requirements and exit-criterion assessment

| Requirement / exit criterion | Evidence and assessment |
| --- | --- |
| Milestone AH closeout document | This document consolidates the implemented subset and its exclusions; GitHub publication and closure are separate lifecycle decisions. |
| URL parsing support and parse diagnostics summarized | URL section, AH2/AH3, lib.rs/diagnostics.rs, native qualification and debug tests. Complete diagnostics/conformance are not claimed. |
| Relative/base resolution summarized | Explicit-base API, unavailable-base proof, AH2/AH3 and resolution tests. |
| Document URL, active/frozen base, base href and limitations summarized | Document section, AH4, Browser addressing/base/publication paths and lifecycle tests. |
| Origin representation and same-origin summarized | Origin section, AH5, origin.rs and identity/serialization tests. No security authorization. |
| Resource addresses and identity keys summarized | Resource section, AH6 and key/capability tests. Complete/reference/request/attachment/cache identities stay distinct. |
| HTML and CSS resource-reference discovery summarized | Discovery section, AH7/AH8 and source-bound inventory/provenance tests. Effective loading remains separate. |
| AG conformance fixtures and debug snapshots summarized | Evidence section, AH9 and AG contracts; 22 runnable / 2 unsupported addressing fixtures, exact synthetic observations, privacy and platform limits. |
| Feature-gap tracker updated; completed gaps narrowed honestly | [Tracker](../engine-feature-gap-tracker.md#browser-runtime--platform) links scoped completion, current provenance, remaining addressing gaps and incomplete browser functions. |
| Behavior documented and linked; known limitations explicit | Owner contracts, implementation/test index, remaining-functionality list; historical failures/decisions retained. |
| Future networking receives normalized typed resource addresses | **Foundation scope accepted by the project owner on 2026-11-05; actual typed transport handoff not implemented.** The [dated owner decision](#project-owner-acceptance-decision-2026-11-05) accepts interpretation A, not literal end-to-end receipt. String-based transport remains incomplete; integration is deferred to AI. |
| Future cache/security/resource loading has stable addressing inputs | Existing immutable URL/address/key/origin inputs are reusable; additional consumer-owned policy, request, response, partition and lifetime context remains necessary. |
| Future AI/AJ/AK/AT/security/navigation/JavaScript extension points documented | Extension matrix names current types/boundaries and missing semantics without adding architecture. |
| Milestone closeout without claiming Fetch/cache/security completion | Supported claim is addressing foundation completion only, with the typed-networking qualification above explicit. Publication, revision qualification, and GitHub closure are separate assessments. |

## AH10 local verification checkpoint (2026-11-04)

At the initial AH10 local verification checkpoint on 2026-11-04, AH10 was uncommitted and
unpublished. Checks ran on Darwin arm64 from the synchronized baseline above
with documentation-only changes. This dated record does not describe later
publication, revision qualification, or closure states.

| Level / check | Result |
| --- | --- |
| Targeted documentation checks | Local links/anchors, referenced symbols and string/typed boundaries, cited PR/CI/job/commit/tree metadata, fixture/corpus counts, whitespace, and exact four-file scope pass. AH1/AH9 original contract bodies and historical local checkpoint are preserved byte-for-byte. No new behavior test or expectation change is needed. |
| Subsystem/feature gates reached by `make ci` | Main/fuzz approved 35-package closures and 32 Python CI-control tests pass. AG8 suite: 7 PASS. Addressing suite: 22 PASS / 2 UNSUPPORTED, no errors. Formatting and all three warnings-denied Clippy lanes pass. |
| Runtime smoke | Not needed: no visible/runtime behavior changes. Existing deterministic evidence remains the authority. |
| Full local `make ci` | **FAIL, exit 2.** Default workspace test command stops in the conformance binary: **186 passed, 9 failed, 13 ignored**, Cargo exit 101. The nine existing native macOS ownership wrappers report process-ownership failures including `task_name_for_pid(...): Mach 5`, with cleanup diagnostics. These are failures, not passes or exclusions. |
| Unreached local CI stages | Explicit HTML5 aggregate, subsequent parser/external/fatal/golden/smoke/WPT/fuzz lanes, builds/benches and generated-entity check were not reached after the workspace failure. They are not qualified by this AH10 attempt; earlier hosted evidence remains separately attributed. |
| Hosted AH10 / publication | Not performed at this checkpoint. AH9's hosted run qualified the identical baseline implementation tree; no AH10 hosted run, commit, push, PR update, merge, issue closure, or milestone closure was performed as part of this local verification. |

The 186-pass aggregate records this attempt's result; it does not replace AH9's
historical 184-pass pre-correction checkpoint. The local aggregate failed; later
qualification, publication, and closure were not assessed by this checkpoint.
