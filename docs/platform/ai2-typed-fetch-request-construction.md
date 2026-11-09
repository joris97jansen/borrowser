# AI2: Typed Fetch Requests And Production Request Construction

Issue: [#1122](https://github.com/joris97jansen/borrowser/issues/1122).
Milestone AI — Fetch, HTTP Networking, Redirects, and Response Handling.
Implementation baseline: master `92ab29634c29b8b68d202d9bda8becbed8e52c6a`.

## Scope and ownership

[AI1](ai1-fetch-http-networking-response-contract.md) governs this implementation.
`net` owns request construction/admission, HTTP field storage, execution-route
compatibility, and request diagnostics. `addressing` continues to own normalized
URLs, origins, complete resource addresses and fragmentless retrieval identity.
Browser captures actual source context and retains navigation, document lifetime,
attachment order, image identity and consumer admission. Bus/runtime carry typed
requests and existing correlation facts. CSS, Layout and Paint gain no URL,
request, HTTP, or routing policy.

This is a request-construction and production-handoff deliverable. It neither
implements AI3 responses nor qualifies AI5/AI6 execution or redirect policy.
Implementation, local validation, hosted qualification and GitHub closure remain
separate states. This document does not establish merge or milestone completion.

## Model and identities

`net::FetchRequest` has private immutable fields and one constructor,
`try_new(request_id, address, method, headers, initiator_url, requesting_origin)`.
It retains the complete AH `ResourceAddress`, an initial current `BrowserUrl`,
explicit `RequestMethod`, ordered `HttpHeaders`, optional actual initiating URL
and requesting `Origin`, and `RedirectPolicy::Follow`. `original_url()` borrows
the complete source address; there is no independently synchronized URL string.
`has_body()` is explicitly false. There is no body input, body buffer or body
mode. `Clone` preserves existing opaque-origin identity.

The existing `RequestId` remains a navigation generation qualified by tab;
stylesheet slots remain separately carried attachment identity. It is not a new
globally unique resource ID. No address hash becomes request/cache identity.

`transport_target()` uses AH's retrieval projection of the initial current URL:
it excludes the fragment, retaining query ordering, repetitions, escaping and an
empty query marker. Complete addresses retain absent/empty/nonempty fragments.
The current URL is the initial attempt; hidden legacy backend redirects are not
observable request-state transitions. AI5/AI6 own that evolution.

AH purpose remains producer knowledge, not runtime routing or a general Fetch
destination. Request metadata can preserve Font/Media/Script/FormAction/Unknown;
none acquires an execution route merely by being representable.

## Admission and execution compatibility

`RequestMethod::try_from(&str)` admits exactly `GET`. Every other token, including
HEAD and lowercase `get`, returns the fixed `UnsupportedMethod` category. The
stored enum has only Get; no free-form method survives admission. HEAD has no
current consumer. Follow is fixed metadata with no policy-selection API; manual
and error redirect modes are unavailable, rather than inactive configuration.

Constructor admission checks scheme, normalized credential components, then
request fields/defaults. HTTP and HTTPS GET are supported. Direct file GET uses
the separate existing local-resource loader and admits no HTTP fields. Reserved,
deferred and unsupported schemes are representable AH values but fail Fetch
admission. Missing source context remains `None` and never rejects by itself.

### Local-file execution

The local loader converts the request's typed current URL through
`BrowserUrl::to_file_path()`, which delegates to the revision-pinned URL fork.
Addressing owns interpretation; Fetch owns file opening and streaming. Conversion
occurs only at execution and never reparses or mutates request provenance.
Query/fragment components are excluded from the native path. Percent decoding is
performed once: `%20` addresses a space, while `%2520` addresses literal `%20`.
Unicode and, on Unix, non-UTF-8 native filename bytes use the backend conversion.

Empty/localhost authorities address local paths. Nonlocal authorities fail on
Unix; Windows delegates drive/UNC handling and invalid UTF-8 rejection to the
pinned implementation. Unsupported target platforms fail conversion. The wrapper
also checks the file scheme (the backend does not) and rejects decoded NUL (the
pinned implementation permits it despite its documentation). Errors contain no
URL/path contents. Conversion failure never falls back to a serialized filename.
Opening failures use a fixed diagnostic too. Existing LocalFile errors, issued
URL metadata, extension-derived MIME, no HTTP status/fields, byte caps and
cancellation remain the existing execution protocol. This is no filesystem
authorization, containment, symlink or remote-share access policy.

This corrects the review finding that newly normalized document issuance could
open a literal percent-encoded filename instead of the intended native path.
Historical AH/AI1 file-loader descriptions record the earlier implementation;
their prefix-stripping/query/fragment limitations no longer describe AI2's loader.

HTTP(S) URLs with a nonempty normalized username or a normalized password are
rejected. Empty authored userinfo that AH normalizes away is not reconstructed
from delimiters. No URL is silently stripped, repaired, or reclassified as a
syntax failure to make admission succeed.

Executable command combinations are closed:

| AH purpose | Runtime route | Stylesheet slot |
| --- | --- | --- |
| Document / Hyperlink | Html | None |
| Stylesheet | Css | Some(slot) |
| Image | Image | None |
| Any other purpose or combination | Rejected | Rejected |

`FetchRequest::validate_route` is the authority for purpose/route compatibility.
`CoreCommand::try_fetch_stream` also checks attachment routing before enqueue.
The runtime repeats this shared validation because direct enum construction is
public, before creating cancellation state or starting execution. Invalid runtime
commands receive only a fixed internal rejection diagnostic, never a resource
event classified through the invalid route. The typed public `net::fetch_stream`
also validates before agent/thread creation and returns an admission error.

## HTTP fields and defaults

`HttpHeaders` is one bounded ordered list shared with future AI3 work. Names are
nonempty ASCII HTTP tokens, canonicalized to lowercase; values retain their exact
bytes. Repetitions and insertion order remain intact. No map, sorting, trimming,
lossy UTF-8 conversion, or universal comma joining is performed. CR/LF/NUL/DEL and
other prohibited controls fail construction; HTAB and obs-text remain representable.

Bounds are 64 fields, 256 name bytes, 8192 value bytes, and 65536 aggregate bytes,
counting `name: value\r\n` overhead. These are initial Borrowser limits, not
universal HTTP limits. Validation occurs before allocation/copy and rejected
append leaves the list unchanged. AI3 can evolve concrete response needs using
the same representation without introducing a competing header model.

The executable request subset admits only singleton Accept and Accept-Language,
with ASCII values and no boundary SP/HTAB. All other caller fields are rejected,
including authentication, cookies, referrer/origin/CORS policy fields, validators,
authority/framing fields, encoding negotiation, and unfamiliar extension names.
No generic sensitivity registry or field-combination framework exists.

Pinned `ureq 2.12.1` retains repeated X-* fields but replaces identical ordinary
names through `.set()`. Differently cased names happen to bypass replacement;
the adapter never uses that quirk. Its value accessors trim boundary whitespace.
Therefore repeated executable request fields and values the adapter cannot
faithfully carry fail explicitly. Generic field storage retains both capabilities.

For HTTP(S), Fetch appends `accept: */*` only when absent, after supplied fields.
There is no resource-specific Accept negotiation, default Accept-Language, locale
lookup, or implicit policy/credential field generation. File requests have none.

The backend separately generates Host, configured User-Agent (the existing
`Borrowser/<version>` policy), framing and current gzip negotiation. The admitted
field list is not a complete wire capture. Existing ureq fallback Accept remains
in its serializer, but normal typed HTTP requests already supply Fetch's default.
Builder inspection and a test-only pre-transport interceptor are distinguished
from a local-server test of actual wire defaults.

## Production handoff and legacy execution adapter

Document navigation builds an AH address, preserving Hyperlink purpose when that
is the producer. `StylesheetFetch` carries the retained complete address rather
than only its retrieval key. Image reconciliation returns complete Image
addresses while keeping selected bindings unchanged. All producers construct
requests through Fetch and use checked command construction.

Stylesheet/image context is the committed document URL and cloned retained
origin, not the resolution base or target origin. Hyperlinks capture source
context before navigation reset. Browser-initiated address-bar, history refetch
and reload operations do not manufacture a document initiator.

`CoreCommand::FetchStream` carries the owned request in a Box (keeping other bus
variants small), tab, optional slot, and
existing runtime route. The runtime forwards that request into the network worker;
there is no parallel public raw-string execution API. The worker retains the
request, including context not yet consumed by execution policy.

The temporary adapter is confined to `net/src/fetch.rs`. `legacy_issued_url`
centralizes event correlation: documents/images retain the complete normalized
original URL, stylesheets retain the original fragmentless retrieval spelling.
Chunks/errors and immutable response snapshots keep those exact issued facts.
The HTTP facade receives the complete legacy input for metadata compatibility;
ureq omits fragments from its wire request target. No transport-local joining or
new URL parser is introduced. AI3/AI5 evolve this legacy event/adapter boundary.

Image cache/lookup keys stay complete normalized strings and ImageIds remain
stable. ResourceManager's checked enqueue callback settles admission/delivery
failures as Error without a lingering Loading state. It owns no Fetch admission.
Stylesheets register only admitted requests before enqueue, preserve exact
generation/key/slot provenance, and roll back failed sends without phantom aborts.
Attachment membership is separate from actual issuance even for rejected sources.

## Navigation prepare, enqueue, commit

All fallible target preparation and checked command creation precede navigation
mutation. A cross-document command is enqueued under exclusive `&mut Tab`; only
successful queue acceptance commits history selection/append, canonical visible
URL, prospective generation, retirement/cancellation, and existing page reset.
Response handlers cannot interleave before commit. Old-generation cancellation
and stylesheet retirement follow the newly queued fetch, using their old IDs.

Rejected preparation or failed enqueue preserves prior history and associations,
generation, visible URL at function entry (including an existing editable draft),
DOM, origin/base, loading, previews and outstanding resource work. Only a fixed
status and repaint notification change. Forward history is not truncated by a
rejected navigation. Queue acceptance is not a downstream worker acknowledgement;
existing downstream delivery-loss limitations remain AI7.

Same-document navigation keeps existing URL/document-association rules and performs
no Fetch admission or I/O. Traversal prepares a candidate index before selecting
it; equal URL strings do not establish shared document identity. Reload always
prepares a refetch without appending history. Existing-entry associations survive
until response-time group rebinding; successful cross-document navigation retains
the previous reset behavior rather than adding a new preview/history lifecycle.

History stores the same complete normalized string used for issuance. Response
start still requires exact requested-string matching; URL equivalence is not
authentication. Final response admission and `accept_history_document` preserve
the existing redirect/group and requested-fragment rules. Source address-bar
spelling is not promised as the normalized request URL.

### Address-bar compatibility decision

The owner explicitly approved HTTPS authority precedence for an address-bar
authority ending in `host:decimal-port`, optionally followed by path/query/fragment.
The host must be nonempty, there is no userinfo in this intent form, and AH parses
and validates the resulting HTTPS candidate. This preserves localhost:3000,
example.test:8443 and bracketed IPv6/port input. It deliberately interprets the
ambiguous custom:3000 as a hostname/port. Invalid ports fail, without another guess.

Other input is parsed directly; only RelativeWithoutBase permits HTTPS guessing.
Explicit custom:opaque and custom://example.test remain unsupported. Hyperlinks
never use address-bar intent rules: localhost:3000 there is an unsupported scheme.
No hostname allowlist, DNS guess, implicit HTTP downgrade or general omnibox
framework was added. AH URL semantics are unchanged.

## Diagnostics

Request-admission and field errors contain fixed categories only. Navigation
maps URL failures to bounded kinds, not diagnostics vectors or raw inputs.
Failed-send payloads are never formatted. Custom Debug for requests and fields
redacts URLs, arbitrary field names/values and bodies while retaining fixed
purpose/method/policy labels and safe structural context. Opaque origin identity,
private lengths, hashes, pointers and timestamps are absent.

CoreCommand has custom Debug for every variant, withholding CSS URLs and HTML/CSS
byte payloads as well as Fetch contents. Compact/pretty/nested and failed-send
formatting are tested. These are internal regression contracts, not public wire
formats. No Serde dependency or separate diagnostics framework was introduced.
Legacy backend logs and CoreEvent error strings are not claimed sanitized by AI2;
their execution/failure adaptation remains AI5/AI7.

## Explicit inherited limitations and extension ownership

**AI2 rejects credential-bearing initial HTTP(S) requests before enqueue. The
legacy backend can follow credential-bearing redirect targets internally and
synthesize authentication without Fetch admission. AI2 does not enforce AI1
credential admission for every redirect hop.** Controlled attempts and hidden
authentication prevention remain [AI5 #1125](https://github.com/joris97jansen/borrowser/issues/1125);
redirect-target admission remains [AI6 #1126](https://github.com/joris97jansen/borrowser/issues/1126).
No middleware, partial redirect loop or post-response check pretends to prevent
target execution. A controlled loopback characterization records this limitation
and must change to pre-execution rejection when AI5/AI6 land. Credential-bearing
legacy final documents remain possible; same-document changes remain possible,
while new requests inheriting those credentials fail initial admission.

Legacy following uses its configured default of 10, not AI6's intended 20-hop
contract. Current URL/hop state, response facts/body/status, image HTTP-error
differences, unqualified native file platforms, backend error strings,
generation-wide cancellation and downstream acknowledgements remain qualified
existing behavior. AI2 does not claim complete credential or browser security.

Future cache mode and validators require AT's concrete Fetch/cache coordination;
credentials require credential sourcing, scope and per-hop policy; referrer policy
requires actual source context plus a generating policy. Integrity requires a
concrete verification boundary; priority requires a real scheduling consumer;
request bodies require supported methods, ownership, framing and replay semantics.
These are documented extension points, not fields or inactive modes. AI3 owns
responses/shared field evolution, AI4 the backend/fake seam, AI5 real execution,
AI6 redirects, AI7 failures, AI8 metadata and AI9 networking conformance. AI10
reconciles milestone qualification. Broader document/image lifecycle remains AJ/AK.

## AI2 acceptance mapping

| Issue #1122 requirement / exit | Implemented boundary / evidence |
| --- | --- |
| Typed request and construction from AH | FetchRequest constructor; document, stylesheet and image production command tests. |
| Address/current URL/method/purpose/context/policy/body/identity | Immutable fields/accessors, explicit no-body invariant and existing generation/slot correlation. |
| Deterministic headers and documented defaults | HttpHeaders storage/limits, closed request policy and pinned adapter tests. |
| Unsupported methods/schemes | GET token admission and fixed scheme/method errors before any enqueue. |
| Unsupported credentials/fields and invalid field data | Initial URL and field admission; atomic generic field validation. |
| Stable request debug | Literal request snapshot and nested request/command/error redaction tests. |
| Extension points without speculative infrastructure | Documented capability owners above; no inactive configuration or new backend hierarchy. |
| Closeable without network execution | Offline request/producer assertions establish these semantics; execution and hosted qualification remain separate. |

## Regression evidence and qualification

`net/tests/fetch_request.rs` covers typed document/CSS/image construction, method
and scheme rejection, credentials, header policy, URL components, origin cloning,
route compatibility and literal/nested Debug. `net/tests/http_headers.rs` covers
ordered repetitions/bytes, invalid syntax, exact bounds and atomic failure.
`net/src/tests/request_adapter.rs` qualifies pinned builder behavior without
network I/O; `net/src/tests/fetch.rs` separately exercises local wire defaults and
the inherited credential-redirect characterization.

`addressing/tests/file_path.rs` qualifies native conversion, exact decoding,
query/fragment exclusion, localhost/nonlocal authority behavior, non-UTF-8 Unix
bytes, fixed errors and NUL rejection. Windows drive/UNC/error tests are gated to
Windows and require native execution before claiming Windows qualification.
`net/tests/local_file.rs` executes the public typed Fetch path against distinct
space/literal-percent/Unicode files, checks metadata/correlation for all three
current routes, and proves failed conversion cannot open the literal fallback.

Browser `fetch_requests` tests exercise actual production commands, context,
address-bar compatibility, canonical exact provenance, navigation/queue rejection
without state loss, same-document identity, slots/routes and nested command Debug.
Existing AH6/AH8 and lifecycle assertions are retained with typed extraction and
explicit valid issuance. Runtime tests cover rejection without misclassified
events and actual local-file delivery. A feature-gated Browser runtime smoke test uses
the existing network, parser and CSS workers to load a local document, stylesheet
and image, verifies opaque source context and installation, and accepts a rendered
frame. Its only additional dependencies are the existing runtime crates for tests.
Offline AG addressing fixtures remain
independent expected evidence; they do not qualify hidden redirect execution.

Targeted, subsystem, runtime smoke and full-CI commands/results are reported in
the final review packet against the actual working tree. A constructed request
and no-I/O producer tests can establish AI2 semantics without network requests;
local-network execution regressions remain separate evidence. No previous result
is presented as a fresh run, and failures/host limitations must remain visible.

### Focused correction qualification (2026-11-06)

The unchanged base `92ab29634c29b8b68d202d9bda8becbed8e52c6a` was extracted with
`git archive` to an isolated temporary source directory. Base and candidate ran
the identical command outside the execution sandbox, sequentially on the same
Darwin arm64/macOS 27.0.1 host with Rust/Cargo 1.92.0 and the same Cargo target
cache. The archive explicitly set `CARGO_TARGET_DIR` to the candidate's existing
target directory; the candidate used that default directory. The environment was
otherwise inherited without test/policy overrides:

```sh
cargo test -p borrowser-conformance --bin borrowser-conformance --locked --offline \
  chromium::tests::native_ -- --test-threads=1
```

Both runs returned failure: **1 passed, 9 failed, 1 ignored**, with the same nine
native ownership failures listed in the review. Seven reported an unavailable
live-process executable during ownership inspection; the injected-mask lifecycle
case retained identity/timeout cleanup failures; the process/pipe case failed its
expected Launch-error classification. All 12 native Chromium source files were
byte-identical (combined path/content SHA-256
`ea40642ffb23261d2d277261f8be1b42d3fa633debac72bedd9b8308dcc3a2ee`).
Locked package versions, sources and checksums were identical; AI2's additional
local dependency edges remain intentional. No processes matching the baseline's
reported artifact paths remained before candidate execution. These observations
establish reproducible baseline failure on this host, not successful ownership
qualification or a full-CI pass. They do not identify the exact uninspectable PID.
No native ownership code, assertion, ignore or CI gate was changed.

The available Docker daemon runs Linux aarch64; its cached amd64 qualification
image would require emulation, not the native Linux x86-64 ownership qualification
required by the existing controller contract. No fresh supported-host/hosted CI
result is claimed. The final review packet records the corrected candidate's
full-CI result and individually executed remaining stages, preserving failures.

Corrected candidate results at this checkpoint:

| Validation | Result |
| --- | --- |
| Native path conversion tests | 3 pass on macOS; Windows-gated cases not executed. |
| Request/header/local-file integration tests | 13 pass; all exercised again by the affected subsystem run. |
| Addressing/Fetch/bus/network-runtime/Browser with conformance observation | 463 pass, zero failures/ignores. |
| Browser local document/CSS/image runtime and frame smoke | Pass with encoded space/Unicode/literal-percent filenames and query/fragment URLs. |
| Full `make ci` | Fails at conformance: 186 pass, the same 9 ownership failures, 13 existing ignores. Dependency checks (including 32 tests), 7 CI fixtures, 22 addressing fixtures (2 unsupported), formatting and all three lint configurations pass first. |
| Supplemental default and HTML5 workspace runs with `--no-fail-fast` | Both fail only the same conformance target; all remaining test binaries execute successfully, including the parser performance guard. No test is filtered or ignored to produce a green result. |
| Remaining parser/external fixtures/fatal failures, DOM/patch goldens, real-page/rawtext regressions, pipeline regressions and WPT tree builder | All pass. |
| Six deterministic fuzz smoke targets | Each completes 128 runs. Existing sanitizer/coverage warnings and the absence of an outer timeout/gtimeout wrapper remain in the logs; this is no sanitizer or coverage-guided qualification claim. |
| Debug/HTML5/release/release-HTML5 builds, both bench compilations, entity generation check | All pass. Release builds retain unused-variable/import warnings in unchanged CSS/HTML/runtime-parser code. |

All remaining CI stages were executed individually after the aggregate stopped;
this does not convert either failed workspace aggregate or `make ci` into a pass.
No supported-host native ownership or Windows execution qualification was added.

## AI3 response handoff update

[AI3](ai3-typed-fetch-response-status-headers-body.md) evolves the response side
in `net` while preserving AI2 request admission, route validation, exact issued
correlation, history timing and generation routing. HTTP error statuses no longer
become image network errors. Shared ordered headers remain the single field
representation. This later implementation does not retroactively qualify AI2's
backend redirects, credentials, Windows behavior or earlier validation results.

## AI5 implementation qualification (2026-11-08)

The initial typed request and Follow intent remain unchanged. AI5 disables backend redirect execution and rejects supported redirect statuses carrying any Location before Start. The prior credential-bearing redirect characterization is now a zero-target-request rejection regression; no implicit authentication is executed. AI6 still owns traversal. HttpClientPolicy no longer exposes a misleading redirect-count knob.

The [AI5 implementation contract](ai5-real-http-response-acquisition-and-bounded-body-integrity.md)
records the implemented subset, exact resource limits, controlled diagnostics,
connection safety and release-abort qualification. It supersedes conflicting
legacy execution qualifications above without rewriting these historical records.
This unmerged candidate is not a hosted-CI, merge, issue-closure or milestone-
completion claim. AI6–AI9 retain their stated independent responsibilities.
