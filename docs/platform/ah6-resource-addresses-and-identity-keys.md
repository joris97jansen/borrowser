# AH6: Resource Addresses And Retrieval Identity

Date: 2026-11-01
Scope: Milestone AH resource addressing and the existing Browser stylesheet and
hyperlink consumers. No new networking or CSS loading capability.

## Ownership and vocabulary

`addressing` owns `ResourceAddress`, `ResourceKind`, `ResourceOperation`,
`ResourceCapability`, and `ResourceIdentityKey`. It reuses immutable `BrowserUrl`
and the revision-pinned URL backend. It depends on no Browser, DOM, CSS, runtime,
layout, or transport implementation.

| Concept | Contract |
| --- | --- |
| Complete resolved reference | `BrowserUrl`, with normalized absolute URL, query and fragment. Existing full URL equality/hash are unchanged. |
| Resource address | Complete URL plus producer-known kind. No descriptor Eq/Hash: consumers choose URL comparison or retrieval identity explicitly. |
| Retrieval-address identity | `ResourceIdentityKey`, the URL serialization excluding its fragment. |
| Request identity | Owning Tab, recorded issuing generation, and stylesheet slot; exact issued URL authenticates producer provenance within that identity. |
| Response provenance | The immutable `NetworkResponseInfo` admitted for that request. |
| Attachment identity | Browser-owned stylesheet slot, source owner, lifetime and cascade order. Equal keys never merge attachments. |
| Future cache identity | Cache-owned composition using address identity and request/response/partition inputs. Matching keys do not establish reuse. |

Kinds are document, stylesheet, image, font, media, form action, hyperlink and
unknown. They describe producer purpose, not a Fetch destination or resource MIME
type. Current `core_types::ResourceKind::{Html,Css,Image}` remains transport routing
and limit classification. Only the known stylesheet consumer maps to `Css` here.
Media cannot select audio versus video; hyperlinks/form actions are not separate
Fetch destinations. No blanket conversion or guessed kind is introduced.

## Construction and identity

`ResourceAddress::new(BrowserUrl, ResourceKind)` accepts an already parsed URL.
`url()` preserves the complete reference; `kind()` exposes producer knowledge.
There is no hidden base, new parser, setter, backend escape hatch, or raw key
constructor. Parse/resolve outcomes and their ordered diagnostics remain AH2/AH3.
An unsupported address is representable without admitting an operation.

`retrieval_key()` projects the backend serialization through
`url::Position::AfterQuery`: no reparse, setter, string splitting, trimming,
percent decoding, query sorting, path canonicalization, DNS or filesystem I/O.
The key owns its serialization; `Eq` and `Hash` use exactly those bytes. Kind,
requesting origin, request/attachment IDs and parse diagnostics do not participate.
Rust hash values are not persistent identifiers or a serialization format.

Queries retain values, order, repetitions, encoding, and absent-versus-empty
markers. Credentials remain significant after existing URL normalization.
`/sheet?a=1&a=2#one` and `/sheet?a=1&a=2#two` share a retrieval key; reversing the
parameters, changing a value, or replacing no query with `?` changes the key.

| Kind | Retrieval key | Complete reference |
| --- | --- | --- |
| Document | Excludes fragment | Retains navigation destination. |
| Stylesheet | Excludes fragment | Retains selected source provenance. |
| Image | Excludes fragment | Retains resource-specific interpretation. |
| Font | Excludes fragment | Retains applicable fragment semantics. |
| Media | Excludes fragment | Retains media-fragment information. |
| Form action | Excludes fragment | Retains target; method/body are separate. |
| Hyperlink | Excludes fragment when projected | Retains navigation target; does not trigger retrieval. |
| Unknown | Excludes fragment | Preserves information without inventing interpretation. |

Absent, empty and nonempty fragments remain distinct in the complete URL. The
key is not decoded-image identity, stylesheet location, a local CSS reference,
or a same-document navigation decision. CSS owns local-reference semantics and
applicability; AH6 does not discover new CSS requests.

## Context and origins

Browser retains requesting-document origin in `DocumentAddressState`, source
owner/slot context in its stylesheet set, and issuance context in the Tab's load
record. Addresses do not duplicate document URL/base/origin or store speculative
request properties. An asynchronous request consumer must capture needed context
at issue time, preserving opaque identity through `Clone`.

Address construction/key projection never invoke origin determination. A target
URL origin is distinct from the requesting origin and from document creation
rules. A cross-origin base does not change the document origin. Independent file
documents can have equal resource keys and unequal opaque origins; serialized
`null` is never an origin key. AH5's Origin equality and lack of Hash are unchanged.

## Operation capability

`capability(Retrieval | Navigation)` classifies AH addressing support, not Fetch
mode, permission, decoder availability, successful transport or document creation.
Only Supported passes the integrated consumers' gates.

| Scheme | Retrieval | Navigation |
| --- | --- | --- |
| http / https / file | Supported | Supported |
| about | Unsupported | Reserved document handling |
| data / blob | Deferred | Deferred |
| ws / wss | Reserved for WebSocket integration; neither operation is admitted | Reserved; not admitted |
| ftp / javascript / unknown | Unsupported | Unsupported |

AH5 document-response admission remains separate. No about inheritance, blob
store, data loader, WebSocket operation, JavaScript execution or policy is added.
Hyperlink activation now uses this classification after explicit-base resolution
and passes the complete destination into the existing navigation machinery.

## Stylesheet reference, issuance and admission

`StylesheetSlotSource::External` retains authored href and optional complete
`ResourceAddress`. Unsupported parsed addresses remain in Failed slots, never
produce fetch candidates, and cannot create pending/issued loads.

Reconciliation preserves owner/source retention, media-only updates, sparse
cascade order, frozen reference selection and monotonic slot IDs. A base-only
change does not retarget an existing association. Retirement identifies removed
attachments by ID; source presence is never evidence that transport was issued.
The historical active-slot observer remains test-only attachment observation.

`Tab.stylesheet_loads` is the authority for actually queued requests. Browser
registers an AwaitingStart record containing the exact retrieval key and issuing
generation before enqueueing FetchStream. All event handling requires exclusive
`&mut Tab` access, so no response can be processed between registration and send.
Absent/disconnected command senders cause immediate rollback and attachment
settlement, without a phantom abort. Queue acceptance is not network execution or
response admission. NetworkStart remains mandatory before byte consumption.

| Phase | Permitted progress |
| --- | --- |
| AwaitingStart | Admit one matching NetworkStart, or settle a transport/provenance failure. |
| Receiving | Consume exact-request chunks under the fixed MIME decision; validate one immutable completion. |
| AwaitingRuntime | Accept the expected zero/one decoded block and sheet completion; preserve the admitted response. |
| Complete | Retain issued provenance until attachment retirement; ignore further events. |

NetworkStart must name the exact issued requested URL. Duplicate starts never
overwrite admitted requested/final URLs, status, content type or MIME decision.
Chunks before admission fail that request. Chunks during Receiving require the
exact issued URL; equivalent normalized spellings and the final redirect URL do
not authenticate a chunk. NetworkDone must match every admitted
NetworkResponseInfo field, without normalization or replacement.

The existing HTTP producer emits one final-response start after redirects and
uses the original request URL for chunks/errors; completion repeats its immutable
start snapshot. Different final response URLs are therefore legitimate. AH6
neither implements redirects nor selects a stylesheet base from the retrieval key.

The CSS runtime buffers Unicode text and emits a complete decoded block only
when CssDone is processed, then CssSheetDone. Browser accepts a block only after
validated network completion and successful CssDone queue delivery, for the
matching request URL/generation/slot, accepted MIME and expected block progress.
Duplicate blocks cannot replace CSS. Premature completion or mismatched runtime
provenance fails the unfinished attachment and retracts its contribution.
Empty-body and MIME-rejected completions settle without installing CSS.

## Failure, retirement and preview behavior

Unknown/unissued slots, stale generations, wrong routing and terminal duplicates
are ignored without creating state or commands. Malformed events for an unfinished
issued request fail that attachment deterministically, remove admission, clear its
pending state, retract an unfinished contribution when necessary and abort only its
CSS runtime buffer. No event URL is used to locate or retire a different request.
Provenance-failure messages use fixed labels without raw URL contents.

CssChunk delivery is marked forwarded only after successful enqueue. Failed
CssChunk or CssDone delivery settles the attachment rather than waiting forever.
CssAbort delivery is checked after admission removal; failure is reported without
reopening state, repeating terminal transitions, or scheduling a later abort for
another generation. The fixed failure status is retained when cleanup also fails.
No stylesheet-local failure sends the generation-wide CancelRequest.

Retirement consumes the actual issued record and uses its original generation/key.
Unsupported references and failed initial dispatches have no such record and
cannot produce phantom aborts. A rejected DOM publication commits no speculative
DOM, document/base state or resource associations, performs no discovery and causes
no replacement-driven retirement. Those publication effects occur only after
validation and commit.

Rejection of a current publication may then fail the document through AH4's
established terminal cleanup. That cleanup preserves the committed DOM/address
state and attachment membership, but cancels unfinished work and retires its issued
stylesheet records and CSS buffers. Retained attachment membership is not evidence
of an active request. CssAbort releases the retired request's CSS buffer;
CancelRequest retains its existing generation-wide network/parser cancellation
semantics. A stale publication from an older navigation generation is ignored
before validation and triggers neither document failure nor this cleanup.

Parser or document failure preserves loaded preview styles and validated network
completions already waiting for CSS runtime results, including rejected-MIME
completions that settle without CSS. Unfinished transfers lose admission and their
partial buffers are aborted. Navigation, accepted Clear/replacement and tab removal
retire actual issued requests. Late events cannot recreate buffers or contributions
through Browser.

CssAbort retires CSS buffering; it does **not** cancel an individual HTTP transfer.
Network cancellation remains generation-wide. A locally retired transfer may
continue and its events are ignored. There is no per-resource cancellation protocol.
Delivery checks observe Browser's existing outgoing command queue, not downstream
router/worker acknowledgements; worker failure detection is not added by AH6.

## Deterministic diagnostics and future extension

ResourceAddress Debug inherits AH3's content-redacted BrowserUrl Debug and prints
fixed kind labels. ResourceIdentityKey Debug always withholds its serialization,
including in nested/pretty output. Issued-request Debug prints only redacted key,
phase and response-admission presence, never raw response URLs or opaque identity.
Exact semantic URL/key assertions remain separate controlled test evidence.
Snapshots are internal regression contracts, not public wire formats. No hashes,
fingerprints, sensitive lengths or automatic logging are added.

Future mode, credentials, referrer, integrity and priority belong to a request
consumer that has the applicable HTML/CSS integration facts. Cache partitioning,
method, variation and response reuse belong to future cache composition. No
placeholder fields, defaults, new Fetch object, or Origin Hash is introduced.

The image manager retains its complete string lookup/callback/decoding keys.
Changing those keys requires coordinated presentation and request-lifetime work.
No image migration, bus redesign, HTTP cache, new transport, CSS resource grammar,
CORS, CSP, cookies, storage, service workers, mixed-content policy, navigation
history feature or JavaScript API is implemented. Existing file-query/path mapping
and raw legacy status/logging limitations remain outside these new diagnostics.

## Authority and regression evidence

- [WHATWG URL serialization/equivalence](https://url.spec.whatwg.org/#url-equivalence)
- [Fetch requests](https://fetch.spec.whatwg.org/#requests)
- [HTML origins](https://html.spec.whatwg.org/multipage/browsers.html#origin)
- [HTML navigation](https://html.spec.whatwg.org/multipage/browsing-the-web.html#beginning-navigation)
- [CSS URL processing](https://drafts.csswg.org/css-values-4/#url-processing), including local fragments
- [AH1](ah1-url-origin-resource-addressing-contract.md), [AH2/AH3](ah2-typed-url-parsing-and-diagnostics.md), [AH4](ah4-document-url-and-base-state.md), [AH5](ah5-origin-representation-and-same-origin.md), [AF5](../css/af5-stylesheet-rule-collection-source-order-contract.md)

Five permanent Browser regressions first failed on AH5: pre-admission chunks,
wrong requested URL at start, duplicate-start MIME overwriting, inconsistent
completion metadata, and unsupported dispatch. They pass with AH6. Expanded
coverage exercises routing, exact strings, redirects, immutable metadata, runtime
ordering, send failures, attachment isolation and unsupported retirement. Existing
runtime-backed cancellation/preview and bounded per-chunk lookup tests remain.
The document-addressing publication-rejection regression distinguishes atomic
staging rejection, current-load terminal cleanup and ignored stale publications.
It checks retained DOM/address/attachment state and completed preview CSS, exact
issued-request abort provenance, absence of speculative fetch/replacement effects
and rejection of late retired traffic. Existing
`resource_associations_completed_css_queued_before_parser_failure_still_applies`
and `resource_associations_rejected_css_queued_before_parser_failure_settles_without_style`
cover permitted queued runtime results through the same document-failure cleanup.
Validation results are recorded after the implementation checks below.

## AH6 acceptance mapping

| Exit criterion | Implementation/evidence |
| --- | --- |
| Resource addresses are typed | `addressing::ResourceAddress`; retained external stylesheet source and hyperlink consumer. |
| Resource identity keys are defined | Private serialized representation with fragmentless `Eq`/`Hash`, no raw constructor. |
| Stable addressing input for future networking/cache | Immutable address/key APIs below consumers; actual stylesheet handoff uses the key. Request/cache composition remains separate. |
| Resource kind represented where known | All eight required variants; production Stylesheet and Hyperlink construction. |
| Fragment/query behavior documented and tested | Per-kind matrix; normalization/relative equivalence, optional components, query ordering/repetitions, percent encoding and credentials. |
| Deterministic debug output | Literal address/key snapshots and issued-record redaction tests in every phase. |
| Closeable without implementing fetching/cache | Existing bus/transport/image manager remain; no new fetch/cache implementation. |

Additional integration gates cover exact request provenance, immutable accepted
response, checked command delivery, unsupported-reference retirement, bounded
lookups and preservation of already queued CSS results in failed previews.
Functional acceptance and a clean full-CI result are reported separately below.

## Validation on 2026-11-01

Final results use the AH6 source tree after a clean workspace-artifact rebuild.
No assertions were weakened, no admission bypass was introduced and no new ignores
were added. Existing cascade/MIME/lifetime fixtures now establish actual dispatch
and admission before supplying runtime results.

| Level / command | Result |
| --- | --- |
| Initial five `ah6_css_` regressions on AH5 | All five failed before corrections; same regressions pass with AH6. |
| Targeted AH6 Browser tests | 18 pass, including source retention and every request phase. |
| Addressing suite | 45 pass, including six new identity/debug tests and unchanged pinned URL/origin qualification. |
| Browser default and `--features html/html5` | 327 unit tests and five integration tests pass in each configuration. |
| Networking / CSS runtime | 14 / three tests pass with host local-socket access. An initial sandbox-only run denied test-server binds; the host rerun passes. |
| Dependency graph / formatting / lint | `check-url-dependencies` (including 32 Python checks), `fmt-check`, `lint`, `lint-html5`, `lint-html5-hardening` pass. |
| `make conformance-ci` | Seven pass; no failures, errors or skips. |
| Workspace default and html5 lanes excluding `borrowser-conformance` | Both pass: 66 reported test groups and 2,423 passing results per lane, including the long-running parser streaming guard. |
| Remaining parser/conformance/golden/regression/fuzzer targets from `make ci` | Pass: parser conformance, external fixtures, fatal-failure contracts, DOM/patch goldens, real-page smoke, rawtext/script regressions, six deterministic fuzz smokes, pipeline regressions and WPT tree builder. |
| Build / benches / generated data | Default/html5 debug and release builds, both bench compile targets and entity check pass. |
| Conformance integration targets | 29 pass; five existing ignored cases. |
| `make ci` / `make test-html5-runtime` | FAIL in the existing native macOS conformance ownership tests: 177 pass, nine fail, 13 existing ignores in the conformance unit binary. No clean full-CI pass is claimed. |

The nine failures are `native_capture_failure_diagnostics`,
`native_cleanup_diagnostic_failures`, `native_lifecycle_regressions`,
`native_macos_bsd_ownership_states`, `native_process_and_pipe_contract`,
`native_resource_completion_and_shutdown`, `native_shutdown_uses_capture_deadline`,
`native_startup_exit_event_classification` and `native_temporary_artifact_containment`.
They report an unavailable live-process executable during ownership scanning, with
cleanup identity/time-out failures. Running `native_macos_bsd_ownership_states`
against clean exported master reproduces the same failure. AH6 changes no native
ownership tooling. Remaining validation was run explicitly rather than treating
the early workspace failure as coverage for later targets.

The publication-failure contract correction changes only this document and the
existing addressing regression. Its targeted test and complete Browser suite pass
in default and HTML5 configurations (327 unit and five integration tests per full
lane); formatting and default/HTML5 workspace lint pass. Full CI and runtime smoke
were not repeated for this documentation/test-only correction. The native failures
above remain unresolved; only the named representative has baseline reproduction.

### Runtime smoke

`make run` successfully launched the native application. Automated Shell URL entry
did not load the fixture, so it is not claimed as a completed toolbar scenario.
A temporary harness outside the repository linked the actual Cargo-selected
Browser/platform artifacts and used public UiApp/Tab APIs with real file, parser,
CSS, layout, image and paint runtimes. It loaded a local page with an explicit base,
two stylesheet references differing only by fragment, an image and a fragment link.
It exercised fragment navigation, back/forward/back and refresh, received four CSS
completions over two document loads and returned Rendered. Window inspection
confirmed the expected blue heading, green text, background and decoded image.
This supplements the deterministic production pointer-activation/navigation and
runtime-backed preview regressions; it does not claim manual toolbar coverage.

### Architecture review and closure boundary

Every new public abstraction has an AH6 requirement or present stylesheet/hyperlink
consumer. The image manager, transport, bus, parser, CSS semantics, layout and paint
owners are unchanged. The issued-request record, immutable response and attachment
remain separate; unsupported sources never acquire an issued lifecycle. Checked
queue delivery settles avoidable pending work, and local failure never cancels the
document generation. No shortcut or demo-specific engine behavior was introduced.

All AH6 functional exit criteria have implementation and deterministic evidence.
The independent native macOS full-CI failure remains unresolved; this document does
not waive a clean-CI requirement or claim unconditional issue-close approval.
