# AH5: Origin Representation And Same-Origin Comparison

Date: 2026-10-31
Baseline: merged AH4, master `970a4b76aaa7ab72285645f0c4ca0a9712e21669`
Scope: typed URL origins and retained ordinary response-document origins

## Ownership and representation

`addressing` owns `Origin`, URL-origin determination, same-origin comparison,
semantic serialization, and structural origin Debug. Browser owns document
admission and the lifetime of its retained origin in `DocumentAddressState`.
HTML, CSS, Layout, Paint, transport, and conformance tooling do not compute origins.

`Origin` privately contains either a tuple (owned normalized scheme, owned typed
host, optional nondefault port), or shared opaque allocation identity (`Arc<()>`).
The backend host remains private; the borrowed public `UrlHost` API is unchanged.
Origin values outlive the URL from which they were determined. No URL, DOM,
request, history identity, or registry is retained to stand in for origin identity.

`Clone` shares opaque identity. `PartialEq`/`Eq` mean same-origin: equal tuple
components, or the same opaque allocation. Opaque equality uses `Arc::ptr_eq`,
not the unit payload's value equality. Independent determinations are unequal,
even for the same file URL. Tuple and opaque origins are never equal. No Hash,
Display, public component constructor, mutation, or deserialization API is added.
The HTML domain field is implicitly null; document.domain is unsupported.

## Operations and standards

- `BrowserUrl::url_origin() -> Origin` determines a URL origin. Each opaque
  result is fresh. Parsing itself does not select or cache an origin.
- `Origin::is_opaque() -> bool` distinguishes kind without exposing identity.
- `Origin::is_same_origin(&Origin) -> bool` compares semantic origin values.
- `Origin::serialize() -> String` implements HTML semantic origin serialization.

Authority: [WHATWG URL origin](https://url.spec.whatwg.org/#origin),
[HTML origins](https://html.spec.whatwg.org/multipage/browsers.html#origin), and
[HTML document-origin determination](https://html.spec.whatwg.org/multipage/document-sequences.html#determining-the-origin),
inspected for the approved plan (URL updated 2026-10-19; HTML updated 2026-10-30).
The pinned WPT source and revision below remain independent expectations.

The maintained URL parser supplies normalized components. Origin determination
does not invoke the backend origin accessor: the pinned backend recursively
accepts blob embedded schemes beyond the current URL Standard's fallback set,
and its opaque identity uses a counter. AH5 implements the origin operation in
the addressing owner, without changing the parser, dependencies, or lockfile.

## Scheme behavior

All rows accept syntactically valid inputs through existing URL parsing. Parsing,
origin semantics, document admission, and transport support are separate.

| Scheme | URL origin | Ordinary response-document admission | Existing transport |
| --- | --- | --- | --- |
| http | Tuple | Supported | HTTP |
| https | Tuple | Supported | HTTPS |
| ftp | Tuple | Unsupported | None |
| ws | Tuple | Unsupported | None |
| wss | Tuple | Unsupported | None |
| file | Fresh opaque | Supported; origin retained per new load | Existing local-file loader |
| about | Fresh opaque | Unsupported; inheritance context unavailable | No general handler |
| data | Fresh opaque | Deferred | None |
| blob | No-entry fallback described below | Unsupported | No store/loader |
| javascript | Fresh opaque URL origin | Execution/document creation unsupported | None |
| Unknown | Fresh opaque | Unsupported | None |

Tuples use parser-normalized scheme, domain/IPv4/IPv6 host, and `port()`.
Default ports have already normalized to absence. Credentials, paths, queries,
fragments, and document bases do not participate. No DNS, filesystem lookup,
second host parser, percent decoding, or separate default-port table is involved.

### File policy and interoperability

Borrowser selects a fresh opaque origin for each independent file-origin
determination. A new file document load determines once; a reload, independent
tab, or cross-document history refetch receives a fresh identity. Same-document
fragment navigation, traversal within the available document, and retained
state clones preserve identity. File host/path equality and directory ancestry
grant no origin equivalence. An unchanged base does not determine origin either.

WHATWG leaves file-origin policy implementation-defined and recommends a new
opaque origin when uncertain. Interoperability evidence inspected during the
plan demonstrates why serialization and resource permissions cannot substitute
for an explicit decision:

- [Chromium Blink origin checks](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/platform/weborigin/security_origin.cc)
  have local-file access checks and null-serialization behavior distinct from
  ordinary tuple handling. Chromium's
  [origin type documentation](https://chromium.googlesource.com/chromium/src/+/HEAD/url/origin.h)
  also identifies layer-specific file behavior.
- [Firefox URI comparison](https://raw.githubusercontent.com/mozilla-firefox/firefox/main/netwerk/base/nsNetUtil.cpp)
  includes strict file-specific comparison, while
  [web-exposed serialization](https://raw.githubusercontent.com/mozilla-firefox/firefox/main/dom/base/nsContentUtils.cpp)
  is a separate operation. A `null` string alone proves no identity relationship.
- The [WHATWG interoperability discussion](https://github.com/whatwg/html/issues/3099)
  records historical differences between document, storage, and resource cases.

This is source and historical interoperability evidence, not a new released-
browser execution claim. Borrowser deliberately does not reproduce filesystem-
based principal equality or every engine's local-file compatibility rules.

**This origin policy does not define filesystem resource-access permissions or
future same-origin enforcement for local resources.** Existing local-resource
loading is unchanged; AH5 does not add access control. In particular, the
existing file loader's query/fragment path-mapping limitation remains open.

### About and blob creation contexts

An about URL's opaque URL origin is not necessarily its document's origin.
`about:blank` can inherit a source origin; `about:srcdoc` requires one.
Ordinary response-document admission rejects these contexts rather than
silently substituting a URL-derived opaque origin. No browsing-context creation,
source-origin hierarchy, or sandbox model is introduced.

BrowserUrl currently has no associated blob URL entry. For this actual no-entry
case, addressing parses the serialized path through existing URL parsing and
returns the embedded URL's origin only for http, https, or file. Invalid paths
and every other embedded scheme produce fresh opaque origins. This includes
nested blob, ftp, ws, and wss. The embedded file case follows Borrowser's file policy.

The Standard's existing-entry rule instead returns the entry environment's
retained origin, including any shared opaque identity. That context is unsupported
because there is no blob store or entry association; the no-entry fallback is not
a substitute for it. Browser rejects blob response documents. No override or
general resolver abstraction is added to anticipate future storage.

## Document admission and lifetime

The supported context is an ordinary unsandboxed http/https/file response
document. `DocumentAddressState::new()` validates that context before determining
origin and returns a complete state or `UnsupportedDocumentCreationContext`.
Preparation mutates no Tab, PageState, history, DOM store, or parser state.

`on_html_network_start()` keeps the existing generation, duplicate/terminal,
requested-history URL, and effective-response URL checks. It prepares address/
origin state before `accept_history_document()` and before any response acceptance,
DOM retirement, final-toolbar URL update, page installation, or parser-start command.
`PageState::begin_document()` installs the prepared state without fallible admission.
Empty final URLs preserve the requested-URL fallback; invalid or unsupported
nonempty final URLs never fall back.

Rejection calls the existing `fail_document()` with the fixed message
`Unsupported document creation context`. Failure stops loading, records status,
cancels the current request, and redraws using AH4's existing terminal semantics.
It does not become a new DocumentPublicationFailure category. No history URL or
association is acceptance-rebound, no response is accepted, and no parser starts.
Navigation may already have cleared the old page: rejection does not restore it.
Existing history selection/entry creation and failure cleanup remain unchanged.

### Streaming admission and immutable response provenance

The response snapshot in `DocumentLoadState.response` is installed only by a
successfully admitted NetworkStart. `Pending` is not admission: it also describes
the interval after navigation starts but before any response is accepted.

HTTP [`net/src/fetch.rs`](../../crates/net/src/fetch.rs) constructs one
NetworkResponseInfo after redirects, sends its clone in Start, then streams
chunks synchronously before moving the same snapshot into Done. The
[`file` producer](../../crates/net/src/file.rs) does the same after opening the
file. Read/cancellation/limit failures send Error instead of Done. The
[`network runtime`](../../crates/runtime_net/src/lib.rs) forwards each callback
directly through the same event sender; the platform bridge receives events in
order. The conformance executor also injects start/chunks/done with a shared
snapshot. These existing producers guarantee identical requested_url, final_url,
status_code, and content_type at start and completion, with no late metadata
updates. The public Browser event API does not encode those guarantees in its
types and must enforce admission independently of well-behaved producers.

HTML navigation requests use `stylesheet_slot_id: None`; the network runtime
copies that association into Start/Chunk/Done/Error unchanged. Conformance's HTML
injection also uses None throughout. No current HTML path legitimately owns a
stylesheet slot. Browser routes all four HTML network event variants only with
None; Some(slot) falls through as an unmatched route, without failing or changing
the active document. This follows the existing dispatcher convention for
inapplicable associations (for example CSS network events without a slot).

HTTP passes the original request string, not the post-redirect response URL, to
the shared `stream_reader`. File streaming uses that same original request
string as well. Each chunk therefore carries the admitted response's exact
`requested_url`, and runtime forwarding preserves it. Conformance uses its
injected requested URL. Browser does not substitute `final_url`, active base,
or mutable document URL when validating chunk provenance. This remains true
after redirects and same-document fragment navigation, and when transport
normalizes the final URL's spelling. No legitimate current stream changes.

After the existing current-generation and terminal-state guards:

- A chunk without an accepted response fails the load before changing byte
  count or sending ParseHtmlChunk.
- A chunk whose URL differs from the accepted snapshot's requested_url fails
  through `fail_document()` before incrementing progress or forwarding bytes.
  The comparison preserves the exact producer spelling, including query and
  fragment; it is response provenance comparison, not URL or origin equivalence.
- Completion without an accepted response fails before installing metadata,
  changing byte count, or sending ParseHtmlDone.
- Completion with any different response field fails through `fail_document()`
  before changing accepted provenance or progress. Comparison uses the existing
  complete NetworkResponseInfo equality; even an empty final_url versus an
  explicit equivalent URL is a changed snapshot. This is response provenance
  comparison, not origin or URL-semantic equality.
- Matching completion retains the start snapshot, updates final byte count, and
  sends ParseHtmlDone. It never assigns document_load.response or recomputes
  document origin. A same-document fragment move during streaming can change the
  document URL without changing its accepted response provenance.

The existing failure path cancels the current request, stops loading, and
retains any committed preview, address/origin, and history associations while
performing existing stylesheet cleanup. No old document is restored if navigation
already retired it. Late events cannot reopen failure; a fresh navigation can
recover. Duplicate starts and stale-generation events retain their previous
handling. No new state machine, response identity type, or networking policy is
introduced.

| Boundary | Origin behavior |
| --- | --- |
| New tab / navigation reset | No retained document address/origin |
| First accepted supported response | Determine exactly once from effective response URL |
| Chunk/completion before accepted start | Fail without admitting response metadata or parser input |
| Matching network completion | Preserve accepted response snapshot and document origin |
| Inconsistent network completion | Fail; preserve accepted provenance and retained preview/origin |
| Initial publication / changed parser handle / same-load Clear | Preserve accepted load's origin |
| Accepted publication and base changes | Clone retained identity through staging and commit |
| Rejected/stale publication | Preserve committed origin |
| Duplicate response start | Ignore before admission; preserve existing origin and parser work |
| Wrong tab / stale generation | Ignore before admission |
| Parser failure after committed preview | Retain preview's origin |
| Fragment navigation / same-document history traversal | Preserve identity while updating document URL |
| Refresh / independent load / history refetch | New determination after accepted response |

History DocumentState associations remain distinct from origin, URL equality,
DOM handles, and requests. Repeated origin access borrows retained state. A failed
lifecycle cannot be reopened by a same-generation response or publication. A new
navigation generation can recover normally. No history redesign is introduced.

## Serialization and diagnostics

Semantic serialization is normalized `scheme://host[:nondefault-port]` for tuple
origins and `null` for opaque origins. It excludes credentials, path, query, and
fragment. Host formatting delegates to the existing backend host serializer,
including IPv6 brackets. Semantic serialization exposes host/port; it is not a
production logging interface or equality key.

Custom Origin Debug uses fixed kind/scheme/host-kind labels and redacts explicit
ports. It never emits hosts, opaque IDs, addresses, counters, hashes, random
tokens, payloads, or sensitive lengths. Nested/pretty Debug and the document
address snapshot inherit that policy. Snapshots are internal regression contracts.

Determinism means fixed classification, tuple values, serialization, diagnostics,
and identity relationships for a stated sequence of creation/sharing operations.
Fresh opaque origins are intentionally distinct; their hidden runtime identity
does not become a reproducible label or cross-engine comparison field.

## Regression evidence and validation

`addressing/tests/origin.rs` covers normalized tuples, default ports, domain/IDNA/
IPv4/IPv6, irrelevant URL components, ownership, fresh/shared opaque identities,
file policy, unsupported transport schemes, and blob fallback restrictions.
`origin_debug.rs` checks literal ordinary/pretty/nested private-safe output.

The separate 31-case `origin-selection.json` uses unchanged upstream WPT revision
`c48d58747e1f211527fb695fd60548a997fae617`, URL Standard revision
`845bafca22fb0c5a42c4bf384a9bdea525a2f10d`, and the existing corpus/license hashes.
Qualification checks source identities and expected semantic serialization through
the production API. This is a selected serialization claim, not full WPT origin
coverage, file-policy interoperability, opaque identity coverage, or an AG projection.
Opaque identity is proven by native relationship tests, not `null` assertions.

Browser's existing document-addressing, navigation, and lifecycle suites cover
origin retention/replacement, file loads/refetches, base independence, atomic
rejection, history association preservation, unsupported final responses,
stale/duplicate/terminal events, preview retention, and diagnostic privacy.

Initial AH5 validation on 2026-10-31 (before the streaming-provenance review correction):

| Level | Command / evidence | Result |
| --- | --- | --- |
| Targeted | `cargo test -p addressing --locked` | PASS: 39 tests; 31 selected origin serialization cases plus unchanged 896 parsing outcomes. |
| Targeted | Browser document_addressing / navigation / document_lifecycle filters | PASS: 17 / 18 / 11 matching tests. |
| Subsystem | `cargo test -p browser --locked` | PASS: 303 unit tests and five integration tests. |
| Subsystem | `make fmt-check`, `make lint`, HTML5 and hardening lint variants | PASS. The additional lint variants passed inside the full CI attempt. |
| Conformance | `make conformance-ci` | PASS: seven cases, no failures/errors/unsupported/skips. |
| Build | `make build`, `build-html5`, `build-release`, `build-release-html5`, both bench compilation targets, entity check | PASS. Untouched HTML/CSS debug-guard code retains release warnings. |
| Remaining CI lanes | Parser conformance, external fixtures, parser fatal failures, DOM/patch goldens, real-page smoke, rawtext/script regressions, six fuzz smoke lanes, pipeline regressions, WPT tree builder | All PASS when executed individually after CI stopped. |
| Runtime | `make run` | Reached native executable without startup errors; intentionally interrupted. Interactive local-page/fragment/history inspection was not performed. Deterministic Browser tests cover those behaviors. |
| Full conformance crate | `cargo test -p borrowser-conformance --locked` | FAIL: nine existing native macOS ownership tests; 177 pass, 13 existing ignores. |
| Full workspace / CI | `make ci`, separate `make test-html5-runtime` | FAIL at the same native ownership tests; not a passing full CI result. |

The initial sandboxed native tests failed with task_name_for_pid/Mach 5.
Host execution instead failed with `live process executable unavailable during
ownership scan`. The isolated native macOS ownership test reproduced that host
failure with one test thread on both AH5 and an exported, unchanged master
`970a4b76...`. The unchanged native scan rejects an uninspectable live same-user
process; it cannot establish complete ownership/cleanup. No conformance assertion,
ignore list, process-ownership implementation, or unrelated process was changed.
AH5 is not declared complete or fully CI-verified while that gate remains red.

Baseline reproduction initially shared Cargo output and exposed stale addressing
artifacts during a startup build. `cargo clean -p addressing` followed by fresh
addressing/Browser tests, formatting, lint, build, and conformance-ci restored
valid AH5 artifacts. The repeated startup check reached the executable. This
was build-output contamination, not a source/API fix or a skipped gate.

### Streaming-provenance review correction validation (2026-10-31)

The new admission and immutable-provenance regression groups failed before the
correction; the valid-stream control already passed. All three now pass.
Coverage includes chunk/done before start, changed requested/final URL (including
unsupported or empty final URLs), changed/absent status and content type, stale
events, duplicate starts, terminal late events, same-document fragment moves
during streaming, retained opaque file identity and preview after failure,
unchanged history associations, and fresh-generation recovery. The existing
unsupported-admission matrix still verifies ignored late chunks/completion.

| Command / evidence | Result |
| --- | --- |
| Browser document_lifecycle / document_addressing / navigation filters | PASS: 14 / 17 / 18 matching tests. |
| `cargo test -p browser --locked` | PASS: 306 unit tests and five integration tests. |
| `cargo test -p browser --features html/html5 --locked` | PASS: the same 306 unit tests and five integration tests. |
| `cargo test -p runtime_parse --locked` | PASS: 28 unit tests and the unchanged linear streaming guard. |
| `cargo test -p net --locked` | PASS: 14 tests, including redirect/stream production paths. |
| `make fmt-check`, `make lint`, `make lint-html5`, `make build`, diff whitespace check | PASS. |
| `make conformance-ci` | PASS: seven cases, no errors or skips. |
| `make test`, `make test-html5-runtime` with host permissions | FAIL: the same nine native macOS ownership tests; 177 pass and 13 existing ignores in that crate. |

The latest workspace failures report the same uninspectable-live-process scan
failure already reproduced on unchanged master above. No native ownership code,
test assertions, or ignore lists changed. Full `make ci` was not rerun for this
correction; its earlier failing gate remains unresolved, not a passing AH5 result.
No new manual GUI smoke was performed for these invalid-event boundary cases;
deterministic event/command and retained-state regressions are the direct evidence.

### Final chunk-provenance and routing review

Focused regressions exercise an exact requested URL (including raw caret, query,
and fragment) across a redirect and an in-flight document fragment move. The
control stream forwards all bytes and completes with unchanged origin and
response provenance. Invalid chunk cases include unrelated file/network URLs,
the final redirect URL, the current document URL, and an equivalent normalized
request spelling. Rejection preserves the preview, actual opaque file identity
where applicable, accepted metadata, history associations, and byte count.

A separate matrix checks that HTML Start/Chunk/Done/Error with Some(slot) are
ignored while awaiting admission, showing a preview, failed, and complete.
It checks retained state/identity and absence of parser/cancellation commands.
Both invalid-input groups failed before the correction; the valid redirect
control passed. The correction introduces no event validator, state machine,
resource identity abstraction, or producer change.

Validation on 2026-10-31:

| Command / evidence | Result |
| --- | --- |
| New chunk-provenance and slot-routing filters | PASS: three regression tests. |
| `cargo test -p browser --locked` and `--features html/html5` | PASS: 309 unit tests and five integration tests in each configuration. |
| `cargo test -p runtime_parse --locked` | PASS: 28 unit tests and the streaming guard. |
| `cargo test -p net --locked` | PASS: 14 tests. |
| `make conformance-ci` | PASS: seven cases, no errors or skips. |
| `make fmt-check`, `make lint`, `make lint-html5`, `make build`, diff whitespace check | PASS. |
| `cargo test -p borrowser-conformance --locked` with host permissions | FAIL: the same nine native macOS ownership tests; 177 pass and 13 existing ignores. |

The native failures still report an uninspectable live process during ownership
scan. No native machinery, assertion, ignore list, networking producer, CSS,
image, navigation, or history behavior was changed. The independently failing
full CI gate remains unresolved; this correction does not establish a full CI
pass. No new GUI smoke or full CI rerun was performed for this boundary check.

## Deliberate exclusions and integration points

Future security policies consume retained requesting-document and target origins;
they own authorization, not origin computation. Future networking/cache consumers
must keep request, response, address, partition, and origin identity distinct.
Same-origin classification alone does not grant any permission or cache reuse.

No CORS, cookies, storage, cache partitioning, CSP, mixed-content enforcement,
JavaScript-facing origin APIs, document.domain, sandboxing, browsing-context
creation, inherited about documents, blob storage/loading, transport changes,
navigation/history redesign, resource identity keys, or full security policy is
implemented. Milestone AH remains incomplete.
