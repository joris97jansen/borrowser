# AI10: Fetch, HTTP Networking, And Response Handling Foundation Closeout

Issue: [#1130](https://github.com/joris97jansen/borrowser/issues/1130).
Milestone AI — Fetch, HTTP Networking, Redirects, and Response Handling.
Current-status assessment: 2026-11-21.

## Scope, source identity, and completion status

AI1–AI9 implement the no-JavaScript Fetch/networking foundation described below.
This is foundation completion for a bounded static-resource subset, not complete
WHATWG Fetch, browser networking, resource loading, navigation or security policy.
AI10 consolidates existing production contracts and evidence; it changes only
documentation. It introduces no Rust type, API, dependency, execution route,
fixture, policy, loader framework or runtime behavior.

The inspected implementation baseline is merged master
`f77a6834db4f71b99b195fc43d35d13991a790b5`, tree
`ffb0a387064bfaf2b13e7cfd15377bb5873d0eed`. Local HEAD, local master,
origin/master and the live GitHub master agreed before AI10 editing. The branch
is `chore/1130-close-out-fetch-networking-foundation`.

AI1–AI9 are closed, and their implementation PRs are merged. Those lifecycle
facts do not independently prove behavior; the implementation and regression
evidence below substantiate the supported claims. AI10 is an uncommitted
documentation candidate. Its owner review, committed-source qualification,
hosted CI, merge, #1130 closure and formal Milestone AI closure have not occurred.
The successful baseline CI is not qualification of these new documentation bytes.

| Issue | Merged implementation PR | Merge commit |
| --- | --- | --- |
| AI1 #1121 | [#1452](https://github.com/joris97jansen/borrowser/pull/1452) | `92ab29634c29b8b68d202d9bda8becbed8e52c6a` |
| AI2 #1122 | [#1453](https://github.com/joris97jansen/borrowser/pull/1453) | `809a9b1ec115192e9b94411271397b8f54426e5f` |
| AI3 #1123 | [#1454](https://github.com/joris97jansen/borrowser/pull/1454) | `212690a6267762d740f9e7050b86e45675f7e756` |
| AI4 #1124 | [#1455](https://github.com/joris97jansen/borrowser/pull/1455) | `6c2789f3b90041fcc5dec2c53d5d2b1ee1c55194` |
| AI5 #1125 | [#1456](https://github.com/joris97jansen/borrowser/pull/1456) | `4a997e93cafeb9d6223b3eb034300e7f950a6bd6` |
| AI6 #1126 | [#1480](https://github.com/joris97jansen/borrowser/pull/1480) | `01827c613876e61929a92b429c7e99ccaca25f20` |
| AI7 #1127 | [#1481](https://github.com/joris97jansen/borrowser/pull/1481) | `cc65bc134f21c31e3e34937132082c583b2de4de` |
| AI8 #1128 | [#1482](https://github.com/joris97jansen/borrowser/pull/1482) | `b9df9172c58c52fc384a4553cf8222d6132f6fcb` |
| AI9 #1129 | [#1483](https://github.com/joris97jansen/borrowser/pull/1483) | `f77a6834db4f71b99b195fc43d35d13991a790b5` |

AI6's actual implementation merge above is separate from its earlier accidental
issue closure caused by an unrelated `Fixes #1126` in imported rust-url history.

## Ownership and authoritative contracts

| Owner | Implemented responsibility | Boundary |
| --- | --- | --- |
| Addressing | Immutable URLs, explicit-base resolution, origins, complete resource addresses, fragmentless retrieval identity and native file conversion | No I/O, Fetch admission, security authorization or document/base selection. |
| Fetch/Networking | Request admission, backend coordination, redirects, response facts, header interpretation, body processing, error mapping and diagnostics | No DOM/CSS traversal, resource installation, document/history decisions or format decoding. |
| HTTP backend | One admitted HTTP attempt, wire fields/framing, fallible body source, transport and TLS facts | No automatic redirects, consumer events or independent loader policy. |
| Runtime/bus | Carry typed requests/events and tab/generation/slot/route correlation; route cooperative cancellation | Hosting does not transfer HTTP or Fetch semantic ownership. |
| Browser/resource consumers | Capture issuance context, admit events, retain document/base/origin and attachment lifetimes, assemble/parse/decode/install resources and invalidate rendering | Consume typed Fetch outcomes; do not implement redirects or independently parse HTTP fields. |
| CSS | Authored reference syntax, parsing, cascade and computed style | No transport or response-header interpretation. |
| Layout / Paint | Geometry / visual output | No networking, URL admission or security policy. |
| Conformance tooling | Execute production boundaries, collect bounded observations and compare independent expectations | No duplicate URL, redirect, MIME, framing or error algorithms. |

The governing chain is [AI1 architecture](ai1-fetch-http-networking-response-contract.md),
[AI2 requests](ai2-typed-fetch-request-construction.md),
[AI3 responses](ai3-typed-fetch-response-status-headers-body.md),
[AI4 backend/fake networking](ai4-network-backend-and-deterministic-fake-network.md),
[AI5 real acquisition/body integrity](ai5-real-http-response-acquisition-and-bounded-body-integrity.md),
[AI6 redirects](ai6-redirect-handling-limits-final-url.md),
[AI7 errors/cancellation/timeouts](ai7-network-errors-cancellation-timeouts-resource-failures.md),
[AI8 metadata](ai8-response-metadata-and-loader-handoff.md), and
[AI9 networking conformance](ai9-fetch-network-conformance-fixtures-and-snapshots.md).
Later contracts qualify earlier implementation checkpoints; this closeout
identifies their aggregate current scope without rewriting historical results.

[AH10](ah10-url-origin-resource-addressing-foundation-closeout.md) records the
accepted addressing foundation and its 2026-11-05 owner decision: typed transport
was outside AH's scope. AI2 subsequently implemented that handoff. AH10's earlier
string-transport descriptions remain historical; this does not change its owner
acceptance. AH resolution, identity, origin and stylesheet-base contracts remain
authoritative. [Subtree maintenance](ai-dependency-subtree-maintenance.md) governs
the actual Borrowser-owned rust-url and ureq source and provenance.

## Typed requests and admission

[`FetchRequest`](../../crates/net/src/request.rs) retains an immutable original
AH `ResourceAddress`, complete current `BrowserUrl`, navigation-generation
`RequestId`, `RequestMethod`, ordered `HttpHeaders`, actual optional initiator URL
and requesting `Origin`, and `RedirectPolicy::Follow`. GET has no request body.
Missing source context stays absent; origin cloning preserves opaque identity.
An address is representable before execution admission and does not grant I/O
or security permission.

Admission supports exactly uppercase GET. HEAD/other method tokens, unsupported
schemes and normalized HTTP(S) URL credentials are rejected explicitly. Request
bodies and manual/error redirect modes have no input API. HTTP(S) caller fields
are singleton Accept and Accept-Language with faithfully representable ASCII
values and no boundary SP/HTAB. Fetch adds `accept: */*` when absent.
Authentication, Cookie, Referer,
Origin, validators and unfamiliar caller fields are not admitted. The backend
generates target/execution fields, including Host, configured User-Agent and gzip
negotiation; the admitted list is not a full wire capture.

`FetchRequest::validate_route` admits Document/Hyperlink → Html, Stylesheet → Css,
and Image → Image. Font/Media/Script/FormAction/Unknown purpose is representable
metadata, not an executable route. [`CoreCommand::try_fetch_stream`](../../crates/bus/src/lib.rs)
additionally requires a stylesheet slot only for Css. Runtime repeats shared
validation before creating cancellation/worker state. Requests cross the bus as
owned typed values, not a parallel public raw-string Fetch API.

Synchronous AH URL failures, field construction errors and request admission
failures remain distinct from failures of an admitted operation. Rejection does
not invent a worker, response, HTTP status or network terminal event. Browser
prepares and enqueues before committing cross-document navigation changes; queue
acceptance is not downstream acknowledgement or successful transport.

Direct file GET uses the separate local loader, with no HTTP request fields.
AH native conversion decodes once, excludes query/fragment from the filename,
and rejects unsupported conversion or decoded NUL without a serialized-path
fallback. File results have local MIME facts and no HTTP status or fabricated
HTTP headers. This is not filesystem authorization, containment or universal
Windows/other-platform qualification.

## Responses, HTTP fields, and URL provenance

[`NetworkResponseInfo`](../../crates/net/src/response.rs) shares private immutable
facts through an `Arc`; Start, completion and contextual failure retain those
facts. Equality includes actual purpose, exact reported spelling, ordered raw
admitted fields and local MIME facts, not pointer identity. Cached parsed AH
provenance and aggregate metadata cannot be independently mutated.

`HttpStatus` validates 100–599 and preserves unknown valid codes and numerical
class. Final HTTP response construction rejects informational statuses. HTTP
404/500 are ordinary response facts on every route: their bodies may complete
or fail independently, and a consumer may accept or reject their content. There
is no fabricated zero/error status, reason-phrase API or response-wide success
boolean. Local responses have no HTTP status/class.

[`HttpHeaders`](../../crates/net/src/headers.rs) retains canonical lowercase names,
original occurrence order, repetitions and admitted byte values. Append validates
before copying and is atomic on failure. The limits are 64 fields, 256 name bytes,
8 KiB per value and 64 KiB aggregate storage including field delimiters. The real
backend preserves admitted opaque values and Content-Encoding/Content-Length
through decoding, but wire-parser boundary whitespace processing means this is
not an exact raw-head capture. There is no universal comma joining or lossy UTF-8
projection; field-specific interpretation is Fetch-owned.

Keep these provenance facts separate:

- Original URL: the complete request address, unchanged across attempts.
- Current URL: the complete target of the current admitted backend attempt.
- Issued correlation: original HTML/image spelling, or original fragmentless
  CSS spelling, retained for chunks and event authentication.
- Reported/parsed response URL: acquired spelling plus its cached AH result;
  production HTTP admission requires usable HTTP(S) provenance before Start.
- Selected final URL: the selected Start/Done response URL, not a failed target
  guessed from a redirect or a fallback to the requested/document URL.
- Redirect chain: actual entered transitions, independent of URL inequality.

Supplied metadata can explicitly preserve missing/invalid provenance. That does
not make it acceptable to actual HTTP/document consumers. When a redirected
acquisition fails without new metadata, failure retains the last acquired source
response and entered history; no response/final URL is invented for the target.

## Fake and real networking backends

The private [`HttpResponseBackend::acquire`](../../crates/net/src/backend.rs)
returns existing response facts plus an owned unread fallible `Read`, or a typed
acquisition failure. It executes one attempt. Shared Fetch owns selection,
redirects, content admission, Start/chunks/terminal events, decoding and budgets.
Backend types/readers do not leak into Browser, CSS, Layout or Paint.

[`FakeNetwork`](../../crates/net/src/fake.rs) uses frozen immutable HTTP(S) routes
keyed by AH fragmentless retrieval identity plus GET. Query distinctions remain;
normalized duplicate registration fails atomically. Each execution has fresh
reader/provenance state, with no response queue, call-order dependency, cache,
DNS/socket/file/native-TLS access or real-backend fallback. Its bytes are
content-encoded content after transfer framing; gzip fixtures contain actual gzip
bytes. Typed acquisition faults and finite reader faults exercise shared
lifecycle/error handling. They do not prove real failure mapping or wire framing.

The real `UreqBackend` uses maintained ureq 2.12.1 with automatic redirects and
automatic content decompression disabled. [`agent.rs`](../../crates/net/src/agent.rs)
sets `redirects(0)`; shared Fetch performs every subsequent attempt. The qualified
subset accepts HTTP/1.0 and HTTP/1.1 responses, fixed/chunked/close-delimited
framing, and the documented strict field/framing rules. Informational responses,
upgrades, unsupported versions and unsupported transfer capabilities fail
explicitly. There is no HTTP/2, HTTP/3 or full HTTP-recipient conformance claim.

[`TlsTrustStore`](../../crates/net/src/policy.rs) and
[`tls.rs`](../../crates/net/src/tls.rs) support native roots, native plus explicit
roots, or explicit custom roots, with certificate validation and TLS 1.2/1.3.
No trust-bypass, revocation-policy, client-certificate or authentication-UI claim
is made. Default policy shares its agent; custom policy creates fresh agents.
Custom-root functionality alone therefore does not prove same-pool isolation.
Actual shared-agent HTTP/HTTPS and controlled real-rustls tests provide that
separate evidence. Reuse eligibility is a momentary observation, not a guarantee
against arbitrary future peer behavior. No hidden replay follows request writes
or response acquisition; rejecting an unused stale connection before transmission
is separate from replay.

## Redirects, limits, and entered history

[`next_target`](../../crates/net/src/redirect.rs) and
[`fetch_http_with_backend`](../../crates/net/src/fetch.rs) implement the bounded
GET redirect subset. AH alone resolves Location against the acquired response
URL. An absent target fragment inherits the current request fragment; an explicit
empty fragment stays empty. Original purpose, context, request correlation and
ordinary admitted fields survive; target-derived execution fields are regenerated.

| Response / target | Current decision |
| --- | --- |
| 301/302/303/307/308 with one Location | Resolve, admit and follow GET. |
| Missing Location | Select the acquired response with its ordinary body rules. |
| Empty Location | Resolve normally, including possible self-redirect. |
| Repeated Location, even identical | Redirect(RepeatedLocation), without another acquisition. |
| Fatal AH resolution failure | Redirect(InvalidLocation); recoverable diagnostics alone are not failure. |
| Non-HTTP(S) target or normalized userinfo | Reject before target acquisition; no file dispatch or implicit authentication. |
| Other 3xx, including 304 | Select without following; retain existing body rules. |
| Follow beyond twenty entered redirects | Redirect(LimitExceeded), before acquisition 22. |

Exactly twenty follows can complete: initial acquisition plus twenty targets,
with at most twenty retained hops. Cycles use this same budget. Repeated URL
identity does not immediately establish a loop; a stateful self-redirect can
subsequently complete.

Location adaptation preserves ASCII and existing percent spelling and escapes
original non-ASCII bytes as `%HH` before AH resolution. It does not decode as
Unicode or split commas. The AI6 contract retains the unsettled standards issues
and tentative WPT evidence; this is an explicit interoperability choice rather
than an unambiguous normative decoding claim.

A resolved/admitted target is pending until backend acquisition entry. After
intermediate-reader release, cancellation can prevent that entry without adding
a hop. Entered history is materialized on either returned acquisition outcome;
none is published while synchronous acquisition is blocked. Entry proves neither
DNS success nor connection/transmission/server receipt. Immutable history
attachment preserves separately retained snapshots.

Intermediate bodies are not delivered or decoded. Fetch releases their readers
unread before the next attempt; head acquisition may already have buffered bytes.
This does not claim intermediate-body completion. Failed/unfinished framing is
not pooled merely by draining an arbitrary amount of untrusted data.

## Body handling, completion, and resource guardrails

[`stream_reader`](../../crates/net/src/stream.rs) delivers owned byte chunks through
the existing 32 KiB reader. `BodyProgress` counts delivered decoded bytes, with a
route-derived cap. There is no universal buffered body or public streaming object.
Acquiring metadata, reading a prefix, receiving a terminal callback and consumer
acceptance are separate facts. A real HTTP Done requires backend-verified framing
completion and consistent byte accounting. Framing, read, decoding, limit
or cancellation failure does not emit Done and may retain already delivered
bytes as failure context.

| Resource route | Encoded content maximum | Delivered decoded maximum |
| --- | ---: | ---: |
| Document / Html | 20 MiB | 10 MiB |
| Stylesheet / Css | 4 MiB | 2 MiB |
| Image | 40 MiB | 20 MiB |

Encoded bytes are after transfer framing and before content decoding, not TLS
records or HTTP/chunk metadata. Exact-limit verified EOF succeeds; observing an
extra byte fails. Content-Length never authorizes body-sized allocation or proves
completion/decoded size. Identity and one gzip/x-gzip coding are supported;
malformed coding grammar, unsupported tokens/stacks, corruption, truncation and
invalid trailing gzip data fail with preserved facts/progress. Original encoding
and length headers remain unchanged.

Wire and typed metadata budgets are separate. The wire head has a 10 KiB line
limit, 64 KiB aggregate limit and 100-field cap, before the stricter typed-field
limits above. Chunk/extension and trailer lines are at most 8 KiB each; trailers
at most 64 KiB/64 nonempty fields; cumulative chunk framing metadata at most
1 MiB. Checked u64 framing lengths and chunk sizes do not allocate declared-sized
buffers. Trailers are validated/discarded, not merged into immutable initial
metadata. AI5 documents the supported grammar and prohibited trailer subset.

Fixed/chunked completion requires complete framing; premature EOF is failure.
Close-delimited transport EOF completes framing but cannot establish application
completeness when the peer's intended length is unknowable. Strict TE/CL conflict
rejection and unsupported-coding decisions are the documented Borrowser subset.

204 rejects Content-Length/Transfer-Encoding and is head-terminated. 205 verifies
actual complete framing and zero raw content before decoding/delivery; an open
close-delimited 205 must time out. 304 is head-terminated and can retain valid
representation metadata without executing its body decoder. These transport
rules do not implement active-document preservation, 304 cache merging or full
resource applicability. A complete 206 transfer remains a partial representation.

Per-operation byte limits do not bound whole-process memory/CPU, queues,
in-flight work, thread count or image decode allocation. Production still uses
thread-per-fetch execution, unbounded runtime channels and retained cancellation
records. The bounded AG collector does not change those production limitations.

## Error taxonomy, cancellation, timeouts, and diagnostics

[`NetworkErrorKind`](../../crates/net/src/event.rs) represents failures of admitted
operations, separately from `RequestAdmissionError`/AH URL failures and HTTP status.

| Outcome | Evidence / boundary |
| --- | --- |
| DnsFailure / ConnectionFailure / TlsFailure / Timeout | Inspectable concrete backend/IO/TLS causes; not URL-scheme or error-message guesses. |
| Cancelled | Cooperative token observed at a supported checkpoint. |
| MalformedResponse | Positively detected malformed response admission/framing/coding grammar. |
| ContentDecoding / UnsupportedEncoding | Actual gzip decoding failure / unsupported content coding capability. |
| UnsupportedResponse | Unsupported response/protocol/transfer capability. |
| ResourceLimit | Explicit EncodedBody, DecodedBody, ResponseHead or FramingMetadata domain. |
| Redirect(reason) | Invalid/repeated Location, forbidden scheme/credentials or follow-limit failure. |
| LocalFile | Native conversion/open failure, without fabricated HTTP status. |
| Read / Backend | Honest unclassified reader/acquisition fallback where stronger evidence is absent. |

HTTP reader wrappers preserve concrete sources through decoding; bare local/fake
reader faults do not automatically acquire HTTP framing/timeout meaning. Admitted
404/500 metadata can coexist with body failure. Retained source metadata, entered
redirects and known progress are not rewritten to describe a failed destination.

[`runtime_net`](../../crates/runtime_net/src/lib.rs) shares a cooperative token per
(TabId, RequestId) navigation generation, not per-resource cancellation. Fetch
checks before acquisitions, at redirect checkpoints, after Start before decoder
construction, and before outer body-reader calls; the raw-205 loop has its own
checks. Internal gzip/framing/TLS reads and Interrupted retries can occur before
the next checkpoint. Changes after a checkpoint may allow acquisition/completion.

Defaults are 15 seconds connect and 30 seconds read/write per backend operation.
DNS remains synchronous without a bounded resolver deadline/cancellation guarantee.
There is no whole-Fetch deadline, immediate blocked-I/O interruption, worker join
or transport-termination guarantee. One ordinary producer terminal callback is
not exactly-once acknowledged delivery; runtime still ignores channel-send errors.

Diagnostics use fixed, bounded structural categories and redact URL/credential,
header/body, server reason and arbitrary source text. Semantic typed response
facts remain available independently. Exact Debug snapshots are internal
regression contracts, not public APIs, persistent identities or wire schemas.

## Declared metadata and existing resource consumers

[`ResponseContentType`](../../crates/net/src/content_type.rs) extracts aggregate
declared MIME/charset once from ordered repeated headers, using the existing
parser and quote/escape-aware splitting. Missing, Empty, Unusable and Parsed
remain distinct; unknown/empty charset labels remain declared facts. No fallback
MIME is invented. This does not sniff bytes, resolve charset labels, inspect
BOM/meta declarations or decode text.

[`DeclaredContentLength`](../../crates/net/src/content_length.rs) exposes Missing,
Unusable, Conflicting, OutOfRange or Value(u64), comparing textual candidates
before numeric projection. `7`/`007` are conflicting metadata even though AI5's
independent numeric framing validation can accept seven bytes. Encoded declared
length, internal observed encoded bytes and delivered decoded progress are
distinct. Fake/supplied facts share metadata construction but their finite readers
do not establish HTTP wire framing or manufacture framing failures from metadata.

Browser owns resource applicability and installation:

- Documents retain the admitted response, final document URL/base/origin and
  incremental parser/publication lifecycle. Network completion is not parser
  completion. Eager navigation reset remains; 204/205 active-document preservation
  and main-response 304/206 preparation remain AJ2 #1133 / AJ3 #1134.
- Stylesheets consume typed MIME metadata, retain exact slot/generation/issued
  provenance and admitted final response bases, and require immutable completion
  plus CSS runtime progress before installation. Missing/Empty MIME is accepted
  for existing compatibility; Parsed text/css is accepted; Unusable/other MIME
  is rejected. This is not complete MIME/security policy. HTTP-error text/css and
  partial-response applicability remain AJ7 #1138 concerns.
- Images retain generation/issuance and response facts, buffer bounded bytes and
  decode only after accepted completion. Valid image bytes can decode despite
  HTTP 404/500; decode/status applicability is independent of Fetch failure.
  Current null/304/206/empty rejection and broader image results/lifetimes remain
  explicit AK4 #1145 boundaries. Local resource failure does not cancel siblings.
- Font/Media purposes can survive response construction, but no execution routes,
  loaders, format support or completed lifecycle are supplied by that fact.

## Deterministic fixtures and claim evidence

The source/test index below is the completion-claim matrix. **B** means existing
baseline implementation exercised by the exact-source hosted lanes in the next
section, not fresh AI10 committed/hosted qualification. Test names and fixtures
define the actual tested subset; no row claims broad standards equivalence.

| Claim | Implementation / owner | Independent tests / fixtures | Supported scope | Known limitations | Qualification | Location in this document |
| --- | --- | --- | --- | --- | --- | --- |
| Typed request/admission | net request.rs; AH resource.rs; bus checked command | [fetch_request.rs](../../crates/net/tests/fetch_request.rs), [Browser issuance](../../crates/browser/src/tab/tests/fetch_requests.rs) | GET, actual context and closed purpose/route/slot combinations | No HEAD/policy modes | B | Typed requests and admission |
| Immutable response/status/fields | net response.rs, headers.rs, event.rs | [fetch_response.rs](../../crates/net/tests/fetch_response.rs), [http_headers.rs](../../crates/net/tests/http_headers.rs), [status tests](../../crates/net/src/tests/status.rs) | Ordered admitted facts; HTTP error statuses remain responses | No reason API/raw wire capture | B | Responses, HTTP fields, and URL provenance |
| Shared fake/backend seam | net backend.rs, fake.rs, fetch.rs | [fake_network.rs](../../crates/net/tests/fake_network.rs), [backend tests](../../crates/net/src/tests/backend.rs) | Production Fetch with immutable no-I/O acquisition | Fake is not wire/TLS proof | B | Fake and real networking backends |
| Real HTTP/HTTPS | net backend/agent/tls; maintained ureq | [AI5](../../crates/net/src/tests/ai5.rs), [TLS](../../crates/net/src/tests/tls.rs), [TLS reuse](../../crates/net/src/tests/ai5_tls_reuse.rs), [connections](../../crates/net/src/tests/ai5_connections.rs) | HTTP/1.0/1.1 response subset, verified TLS | No HTTP/2/3 or full policy | B | Fake and real networking backends |
| Redirects/limits/provenance | Fetch coordination, net redirect.rs; AH resolution | [redirects.rs](../../crates/net/src/tests/redirects.rs), [redirect fixtures](../../crates/conformance/fixtures/networking/fetch/redirects.fixture.json), [loops](../../crates/conformance/fixtures/networking/fetch/redirect-loops.fixture.json) | Five followed statuses, twenty follows, entered history and retained failure context | No identity-only loop rejection | B | Redirects, limits, and entered history |
| Body integrity/guardrails | net stream/body/limits; backend framing | [body completion](../../crates/net/tests/backend_body_completion.rs), [response integrity](../../crates/net/tests/backend_response_integrity.rs), [fields](../../crates/net/tests/backend_response_fields.rs), [decoding](../../crates/net/src/tests/ai5_decoding.rs) | Exact/excess per-route and framing bounds | No global resource quota or complete streaming API | B | Body handling, completion, and resource guardrails |
| Typed failures/cancellation/timeouts | net event/backend/stream; runtime generation token | [backend](../../crates/net/src/tests/backend.rs), [stream tests](../../crates/net/src/stream.rs), [runtime](../../crates/runtime_net/src/tests.rs), [consumer failures](../../crates/browser/src/tab/tests/ai5_failures.rs) | Concrete causes, immutable context and cooperative checkpoints | No immediate termination/delivery acknowledgement | B | Error taxonomy, cancellation, timeouts, and diagnostics |
| Aggregate declared metadata | net response/content_type/content_length | [response_metadata.rs](../../crates/net/tests/response_metadata.rs), [allocation regression](../../crates/net/tests/mime_extraction_allocations.rs), [Browser metadata](../../crates/browser/src/tab/tests/response_metadata.rs) | Declared MIME/charset/length and purpose | Does not prove completion, sniff or decode content | B | Declared metadata and existing resource consumers |
| File retrieval | AH native conversion; net file.rs | [local_file.rs](../../crates/net/tests/local_file.rs), runtime and issuance smoke above | Native conversion and typed local facts | No filesystem security or universal platform qualification | B | Typed requests and admission |
| Existing consumer handoff | Browser tab/html/css/image and resources.rs | [runtime redirect smoke](../../crates/browser/src/tab/tests/redirects.rs), metadata/failure tests above | Document/CSS/image consumers and final bases | AJ/AK lifecycle remains partial; no Font/Media execution | B for existing handoffs | Declared metadata and existing resource consumers |
| Networking conformance | AG networking, discovery, reporting/comparison | [networking tests](../../crates/conformance/src/networking/tests.rs), [CLI](../../crates/conformance/tests/cli.rs), [fixture build](../../crates/conformance/tests/fixture_build.rs), [comparison CLI](../../crates/conformance/tests/comparison_cli.rs) | Local assertions, strict evidence and bounded collection | No Chromium networking equivalence | B | Deterministic fixtures and claim evidence |

There are 18 declarations under
[`fixtures/networking/fetch`](../../crates/conformance/fixtures/networking/fetch):
16 required runnable declarations with 36 rows, plus explicit UNSUPPORTED HTTP
cache and JavaScript Fetch declarations. They cover 200/404/500, repeated byte
headers, empty/binary/gzip bodies, redirects and twenty-follow cycles, Location
variants, unsupported schemes, body limits, network/read errors and failed
redirected acquisition. Actual acquisitions and Start/terminal/progress facts
come from production Fetch, not reconstructed fixture registrations.

Run `make conformance-networking`. The fixed reviewed suite expects 16 PASS /
2 UNSUPPORTED, exit 0, and is required by `make ci` and hosted conformance
execution. Required members cannot silently become skipped/known-failing.
Expectations are independently authored; there is no blessing command.
Missing/contradictory lifecycle evidence remains ERROR before match/known-failure
classification, including zero-byte post-Start errors. Valid semantic mismatch
retains FAIL/XFAIL, and stale known failures retain XPASS.

AI9's focused net tests separately exercise actual controlled loopback HTTP and
equivalent fake facts, including independently expected error categories.
AI5/AI6 additionally exercise real loopback HTTP/HTTPS, certificates, framing,
timeouts, connection reuse/isolation and release panic=abort behavior. Fake
success never substitutes for that transport evidence; public endpoints/DNS are
not required for these new networking regressions.

The AG collector bounds declarations, expectations, queue/projection/retained
storage, diagnostics and staged serialization. It incrementally summarizes body
bytes, drains normal delivery through disconnection, verifies producer closure
and has a ten-second collection deadline plus at most one second of failure
cleanup. Unresolved closure stops later cases and completed-report publication.
Closure proves callback-owner release, not worker join/transport termination.
See AI9 for exact independent budgets and failure protocol.

[AG5](../conformance/ag5-phase-comparison-projections.md),
[AG6](../conformance/ag6-fixture-discovery-and-metadata.md),
[AG7](../conformance/ag7-focused-external-tests-and-references.md),
[AG8](../conformance/ag8-deterministic-reporting-and-suites.md) and
the [conformance README](../../crates/conformance/README.md) retain observation,
discovery, independent-expectation, platform and reporting authority.
`borrowser.cross-engine-phases.v4` separates local networking verdicts from
unsupported Chromium networking coverage: no networking comparison fields or
executed phase documents are invented. Networking-only comparison exits 3 when
local assertions pass; capture-only networking exits 2 without launch. Neither
outcome establishes cross-engine agreement.

## Qualification evidence and historical records

### Exact-source merged baseline

[CI run 44943971694](https://github.com/joris97jansen/borrowser/actions/runs/44943971694)
completed successfully for push commit `f77a6834db4f71b99b195fc43d35d13991a790b5`.
This is the inspected merged source/tree above, not a synthetic PR merge inferred
from a feature-head status. Both aggregate **CI qualification** and required
**Rust (ubuntu-latest)** succeeded; success is not inferred from one passing job.
There were 18 successful jobs and two conditional skips: PR-only exact-head
streaming qualification and the dedicated AG4 timing-branch job. The existing
aggregate validates the required event/source contexts; no new skip is introduced.

| Baseline lane | Observed result / evidence |
| --- | --- |
| Source and provenance | [Success](https://github.com/joris97jansen/borrowser/actions/runs/44943971694/job/136967084256): committed subtree history/source, URL main/fuzz graphs and ureq selection/qualification steps. |
| Rust correctness and parser regressions | [Success](https://github.com/joris97jansen/borrowser/actions/runs/44943971694/job/136967084379): workspace default/HTML5 tests, lint/build and required parser/generated-file regressions, including networking/runtime/consumer targets. |
| Conformance execution | [Success](https://github.com/joris97jansen/borrowser/actions/runs/44943971694/job/136967084322): required addressing, networking and CI suites. |
| Release/networking abort | [Success](https://github.com/joris97jansen/borrowser/actions/runs/44943971694/job/136967084303): release lanes and `make qualify-network-release`. |
| Native Linux/pinned Chromium | [Success](https://github.com/joris97jansen/borrowser/actions/runs/44943971694/job/136967084093): native ownership, full conformance and explicit opt-in Chromium qualifications. Networking assertions still have no Chromium networking comparison. |
| Aggregate/source consistency | [CI qualification success](https://github.com/joris97jansen/borrowser/actions/runs/44943971694/job/136970140904), then [required Rust success](https://github.com/joris97jansen/borrowser/actions/runs/44943971694/job/136970170489). |

This qualifies the executed baseline lanes, not every upstream vendor test,
platform or browser capability. It is not a newly executed local `make ci` result
and does not qualify the AI10 documentation candidate.

### Retained historical failures and incomplete capabilities

The original AI contracts retain their working-tree baselines, corrections,
failures, unexecuted stages and candidate qualification statements. Their older
present-tense pending/uncommitted descriptions refer to those checkpoints. The
merged implementation and current baseline CI record above supersede their
current-status meaning, without retroactively changing any result, source
identity, acceptance decision, provenance record or qualification hash.

In particular, nine native macOS Chromium process-ownership tests failed in the
retained AI5–AI9 local records. They remain failures, not passes because Linux
later succeeded. AG7 documents the native Linux x86-64 qualification boundary and
withdrawn macOS Chromium support pending platform-runtime hardening. This
closeout does not restore macOS supervision or claim fresh Windows qualification.
Those platform failures differ from deferred engine capabilities below.

### AI10 working-tree validation and remaining gates

The following checks ran on 2026-11-21 on Darwin arm64, with HEAD still at the
merged implementation baseline and only the eight approved documentation files
changed. These are local working-tree results. No AI10 commit, staging, source
verifier bypass or acceptance-record change was used.

| Level / exact command or check | Observed result |
| --- | --- |
| 1: `git diff --check`; private documentation validation | PASS: exact eight-file scope, empty staged diff, six original contract bodies preserved byte-for-byte after added notes, 210 local links/anchors, 38 implementation symbols/constraints, deferred-capability coverage and Markdown tables. Counts independently checked: 18 declarations / 16 runnable / 36 rows / 2 unsupported. |
| 1: `make conformance-networking` | PASS, exit 0: 16 PASS / 2 UNSUPPORTED; no FAIL/XFAIL/XPASS/SKIP/ERROR/INCOMPLETE; harness_errors=0. |
| 1: `cargo test -p net --test fetch_request --test fetch_response --test http_headers --test response_metadata --test fake_network --locked` | PASS, exit 0: 54 passed, no failures/ignores. |
| 2: `cargo test -p net -p runtime_net --locked` | PASS, exit 0: 284 passed, including one net doctest; no failures/ignores. Includes real loopback HTTP/HTTPS and typed runtime forwarding. |
| 2: `cargo test -p browser --lib --features conformance-observation --locked` | PASS, exit 0: 390 passed, no failures/ignores; includes deterministic document/CSS/image runtime smoke and final-response-base checks. |
| 2: `cargo test -p borrowser-conformance ai9 --locked` | PASS, exit 0: 29 unit plus 5 integration tests; no failures/ignores. This is the AI9 filter, not the full conformance package. |
| 2: `make qualify-network-release` | PASS, exit 0: 36 HTTP/HTTPS workers, actual intentional SIGABRT and supervisor readiness/termination/drainage/output controls. Existing html/css/runtime_parse release warnings remain visible in the log; none was fixed or suppressed. |
| 2 within `make ci`: `make check-url-dependencies`, then `make check-network-dependencies` | PASS: committed protected subtree/source/test inputs and index verified; exact local URL main/fuzz graphs/edges/features; 246 verifier regressions passed in 394.320 s; ureq current source, historical reconstruction, Git mapping, selection and features verified. |
| 2 within `make ci`: `make conformance-ci`, `make conformance-addressing`, `make conformance-networking` | PASS: 7 PASS; 22 PASS / 2 UNSUPPORTED; 16 PASS / 2 UNSUPPORTED, respectively. No harness errors or failing verdicts. |
| 2 within `make ci`: `make fmt-check`, `make lint`, `make lint-html5`, `make lint-html5-hardening` | PASS: formatting and all three workspace/all-target warnings-denied Clippy lanes completed before workspace testing. |
| 3: interactive runtime smoke | Not needed: no visible/runtime behavior changed. Existing deterministic runtime smoke executed in the Browser/net/runtime lanes above; no live website was used. |
| 4: `make ci` | FAIL, exit 2, 519.305 s. The default workspace test command stopped at the conformance binary: 218 passed / 9 failed / 13 ignored; Cargo exit 101. Completed earlier stages are not a green aggregate. |

The nine failed tests are:

```text
chromium::tests::native_capture_failure_diagnostics
chromium::tests::native_cleanup_diagnostic_failures
chromium::tests::native_lifecycle_regressions
chromium::tests::native_macos_bsd_ownership_states
chromium::tests::native_process_and_pipe_contract
chromium::tests::native_resource_completion_and_shutdown
chromium::tests::native_shutdown_uses_capture_deadline
chromium::tests::native_startup_exit_event_classification
chromium::tests::native_temporary_artifact_containment
```

Their names match the retained macOS failure set. Current diagnostics report
`live process executable unavailable during ownership scan`, with incomplete
cleanup identity and timeout diagnostics. Full output, including reported
temporary artifact paths, is retained; no manual cleanup was used to alter the
evidence. No new skip, expectation change, reclassification or suppression was
introduced; those failing tests were not fixed.
These are fresh local failures, distinct from the successful Linux baseline.

The workspace aggregate did not finish. Subsequent explicit HTML5 runtime,
parser/external/fatal/golden/smoke/WPT/fuzz lanes, builds, release stages,
benchmark compilation and generated-entity verification were not reached by
this `make ci` attempt. The separately executed release probe above passed;
that does not qualify the unreached aggregate stages. No independent rerun or
baseline success is presented as completion of this local CI invocation.

The protected-input checks above bind unchanged implementation/qualification
inputs to HEAD; they do not attest the new, uncommitted AI10 documentation bytes.
No committed AI10 source or hosted AI10 result exists. Final documentation review,
committed-source/integration qualification and hosted aggregate CI remain pending.

Before formal closeout: review the exact eight-file documentation diff, preserve
all failure evidence, qualify the final authorized committed source and required
hosted aggregate/integration contexts, review and merge, and verify resulting
master status. Do not use temporary commits, source-verifier bypasses, rewritten
acceptance records or relaxed expectations to manufacture qualification.

## Remaining capabilities and future integration boundaries

The [feature gap tracker](../engine-feature-gap-tracker.md#browser-runtime--platform)
marks the supported AI foundation implemented while retaining these gaps:

- HTTP cache, cookies, storage and service workers.
- CORS enforcement, CSP, mixed-content policy, referrer policy, HSTS and broader
  browser/security/filesystem policy. Retained origins and inactive Set-Cookie
  headers are facts, not enforcement or a cookie store.
- Authentication/client-certificate UI and policy, downloads and download UI.
- Full document loading, navigation/history/session and resource lifecycles.
- JavaScript `fetch()`, XMLHttpRequest, WebSockets and JavaScript platform/event
  loop integration.
- HTTP/2, HTTP/3, informational-response continuation/upgrades, HEAD and other
  request methods/bodies, broader transfer/content encodings and scheme support.
- Full MIME sniffing, charset determination/text decoding and complete response
  applicability/processing; trailers are not exposed through a general API.
- Complete streaming-body APIs/backpressure, per-resource cancellation,
  acknowledged delivery, bounded DNS/whole-Fetch deadlines and process-wide
  resource/concurrency scheduling.
- Resource prioritization, preload and prefetch behavior.
- Font/Media execution/loaders, broader image formats and complete decode/resource
  lifetimes. Representable purpose or typed metadata alone does not implement them.

| Future owner | Existing integration inputs | Work remaining with that owner |
| --- | --- | --- |
| AJ document lifecycle | Typed admitted request, selected response facts, original/final provenance, chunks and verified terminal outcome | Creation/document/base/origin/history lifetime, 204/205 active-document behavior, main-response applicability, parser/publication and preview lifecycle; AJ2 [#1133](https://github.com/joris97jansen/borrowser/issues/1133), AJ3 [#1134](https://github.com/joris97jansen/borrowser/issues/1134). |
| AJ stylesheet loading / CSS | Typed aggregate metadata, exact generation/slot/issuance, final response base, completion/failure | Browser applicability/attachment/install lifecycle and recoverable failures, AJ7 [#1138](https://github.com/joris97jansen/borrowser/issues/1138); CSS keeps syntax/cascade/computation. |
| AK image/media resources | Existing response facts, bytes/progress and typed failures | Selected binding/result lifetime, format decoding and intrinsic invalidation; AK4 [#1145](https://github.com/joris97jansen/borrowser/issues/1145). Media requires a concrete execution route/loader. |
| Future fonts | Reusable request/response/header/body contracts and producer-known purpose | Actual route, loader, format acceptance/decoding, selection and lifecycle; none is implied by AI. |
| AT cache/cookies/network hardening | Fetch coordination plus request/response/provenance and encoding/length facts | Storage, credential/partition/reuse/validation policy; keys alone are insufficient. AT5 [#1245](https://github.com/joris97jansen/borrowser/issues/1245) owns revalidation/304 merging; AT6 [#1246](https://github.com/joris97jansen/borrowser/issues/1246) owns cache-aware integration. |
| Future security/authentication | Actual captured initiator/requesting-origin context, target/hops and response facts | Concrete policy, credential sourcing/scope, header generation, admission/exposure and tests; missing context is not guessed from target origin. |
| Future navigation | Fetch's original/current/selected-final provenance and terminal outcomes | Browser history/session/document association, timing, activation and lifetime decisions; no transport-owned navigation model. |
| JavaScript platform APIs, if scope reopens | Existing internal Fetch substrate | Web-exposed requests/responses/streams, promise/task/abort/security semantics and API-specific tests. No speculative modes or interfaces are added now. |

An implementation phase is not a GitHub issue. An abstraction is not justified
by the possibility that it may be useful later. These are responsibilities and
existing handoffs, not proposed traits, registries, managers or loader frameworks.

## AI10 requirements and exit assessment

| Requirement / exit criterion | Coverage / current assessment |
| --- | --- |
| Milestone AI closeout document | This candidate consolidates implemented foundation scope; final AI10 review/qualification/merge remains pending. |
| Fetch request construction/admission | Typed requests and admission; AI2 and request/producer/runtime evidence. |
| Typed response and HTTP status/header/body handling | Responses/provenance and body sections; AI3/AI5; immutable facts and independent body completion. |
| Fake and real HTTP/HTTPS backend behavior/limits | Backend section, AI4/AI5/AI7 and separate no-I/O/loopback/TLS evidence. |
| Redirects, limits, loops and final URLs | Redirect section and AI6; twenty follows, actual entered history and selected-final provenance. |
| Network errors/cancellation/timeouts | Error section and AI7; admission/status/transport/consumer distinctions and explicit latency/delivery limits. |
| Metadata extraction and guardrails | Metadata/body sections and AI5/AI8; declared MIME/length versus decoding/progress; independent byte/framing bounds. |
| AG fixtures/local server tests documented | Fixture section and claim matrix; sixteen runnable declarations/thirty-six rows plus two unsupported capabilities; controlled real transport evidence remains distinct. |
| Future document/stylesheet/image/font consumers can use typed responses | Existing immutable response/events expose reusable facts; consumer ownership and required future Font/Media routes/loaders are explicit. This is not a claim that those future loaders exist. |
| Feature gaps narrowed honestly | Tracker's current AI entry and active remaining-functionality list; historical checkpoints and unimplemented capabilities retained. |
| Future AJ/AK/AT/security/navigation/JavaScript boundaries | Integration matrix names existing inputs and actual remaining responsibilities; no extension infrastructure added. |
| Close without claiming full browser networking or deferred capabilities | Implemented AI foundation can be described as complete; #1130/Milestone AI cannot yet be formally closed before final documentation review, committed/hosted qualification and merge. |
