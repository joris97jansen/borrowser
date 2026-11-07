# AI1: Fetch, HTTP Networking, And Response-Handling Architecture Contract

Issue: [#1121](https://github.com/joris97jansen/borrowser/issues/1121).
Milestone AI — Fetch, HTTP Networking, Redirects, and Response Handling.
Authored: 2026-11-05. Inspected code baseline: master
`10c8041c7916600ad48151b730b3ac3f44698c95`.

## 1. Scope, status, and authority

AI1 establishes the architecture contract for static document and subresource
fetching. It changes documentation only. Existing HTTP/HTTPS/file execution,
runtime events, parsing, resource loading, and rendering retain their behavior.
An authored contract, reviewed or merged documentation, implemented behavior,
qualified behavior, and milestone completion are separate states. This working
document does not establish merge, GitHub closure, or Milestone AI completion.

| Label | Meaning throughout this contract |
| --- | --- |
| Standards baseline | Normative behavior of the cited standard, subject to its stated conditions. |
| AI contract | Intended Borrowser ownership or invariant; executable implementation remains with the identified follow-up issues. |
| Initial subset / compatibility limitation | A deliberate capability restriction or recorded difference; no claim of full standards conformance. |
| Current behavior | Inspected production code, including existing limitations. |
| Deferred | Outside AI1 or outside Milestone AI's supported implementation scope. |

The following are the standards authorities inspected on 2026-11-05. WHATWG
Fetch and URL identified their living-standard update as 2026-10-19; RFC 9110
and RFC 9112 are the June 2022 publications. Section links identify authority,
not a claim that the current backend implements every algorithm.

- [WHATWG Fetch](https://fetch.spec.whatwg.org/): coordination, requests and
  responses, redirects, body processing, and later network/cache composition.
- [WHATWG URL](https://url.spec.whatwg.org/#url-parsing): URL interpretation;
  [serialization](https://url.spec.whatwg.org/#url-serializing) and
  [origins](https://url.spec.whatwg.org/#origin) remain addressing-owned.
- [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#section-5): HTTP field
  semantics; [redirect handling](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.4)
  and [userinfo](https://www.rfc-editor.org/rfc/rfc9110.html#section-4.2.4).
- [RFC 9112](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3): HTTP/1.1
  framing and [incomplete messages](https://www.rfc-editor.org/rfc/rfc9112.html#section-8).

Prior Borrowser authority is the AH contract chain: [AH1](ah1-url-origin-resource-addressing-contract.md),
[AH2/AH3](ah2-typed-url-parsing-and-diagnostics.md),
[AH4](ah4-document-url-and-base-state.md),
[AH5](ah5-origin-representation-and-same-origin.md),
[AH6](ah6-resource-addresses-and-identity-keys.md),
[AH7](ah7-html-resource-reference-discovery.md),
[AH8](ah8-css-resource-reference-discovery.md),
[AH9](ah9-addressing-conformance-fixtures-and-snapshots.md), and
[AH10](ah10-url-origin-resource-addressing-foundation-closeout.md).
Later AH contracts and AH10 qualify historical unresolved statements in earlier
documents. AH10's dated owner acceptance leaves actual typed transport integration
to AI; AI1 does not rewrite that decision or claim the integration exists.

## 2. Ownership and dependency direction

**AI contract:** Fetch/Networking is the semantic owner of fetch request
construction from normalized resource addresses, request metadata, HTTP execution
coordination, redirects, response metadata and body representation, network/fetch
errors, diagnostics, and deterministic fetch debug output. Runtime hosting does
not transfer semantic ownership to the thread, event router, or resource consumer.

| Owner | Responsibility | Boundary |
| --- | --- | --- |
| Addressing | Normalized URLs/resource addresses, explicit-base resolution, URL-origin semantics, capability classification, retrieval identity, and addressing diagnostics | No I/O, Fetch admission, authentication, HTTP processing, or document/base selection. |
| Fetch coordination | Build/admit requests with supplied context; select backend execution; coordinate hops, response/body lifecycle, errors and diagnostics | No DOM traversal, CSS grammar, document creation/history decisions, image/font decoding, or independent URL parser. |
| Networking backend | Execute an individual admitted transport attempt; expose HTTP facts and body progress; map transport/framing/encoding failures | Does not hide redirect policy, authorize credentials, or reinterpret authored references. |
| Rust runtime/bus | Schedule/route commands and events, preserve correlation and ordering, carry cancellation | Execution hosting, not a second Fetch or HTTP implementation. |
| Browser/document/resource loading | Capture issuance context; retain document/base/origin lifetime, request and attachment associations; admit consumer events; parse/decode/install resources and request invalidation | Consume Fetch results; do not implement redirects or independently interpret HTTP fields. |
| CSS | Authored reference syntax, CSS parsing, cascade, computed style and CSS semantic invalidation | Fetch provides transport metadata/bytes; Browser retains stylesheet attachment order and response bases. |
| Layout / Paint | Geometry and visual output, respectively | No HTTP, address admission, resource discovery, or network policy. |
| Conformance tooling | Exercise production boundaries and report independently expected results | No duplicate fetch/redirect/header semantics. |

Logical flow:

```text
producer reference + explicit source context
  -> AH normalized resource address
  -> Browser captures issuance/lifecycle context
  -> Fetch admission and coordination
  -> backend attempt(s)
  -> immutable response metadata + body progress + terminal outcome
  -> document/resource consumer acceptance and lifecycle
```

Physical placement must follow existing dependencies and concrete consumer needs.
Evolve `net`, shared types, and runtime handoffs where appropriate; AI1 selects no
new crate, public trait hierarchy, or bus redesign. Fetch cannot depend on private
Browser, HTML/CSS, Layout, or Paint state to interpret an already resolved target.

## 3. Vocabulary and identity

| Concept | Meaning |
| --- | --- |
| Resource address | AH `ResourceAddress`: complete normalized `BrowserUrl` plus producer-known purpose; construction is not admission or I/O. |
| Fetch request | One coordinated resource operation: admitted target, method/headers, actual issuance context, redirect behavior and correlation. It may require multiple transport attempts. |
| Individual network request | One backend attempt against the current target. A redirect creates a subsequent attempt without changing the owning Fetch operation or attachment. |
| HTTP response | Status, optional reason metadata, fields, URL provenance, and body lifecycle from HTTP execution, including 3xx/4xx/5xx responses. Acquiring headers does not prove body completion. |
| Future policy-filtered response | A later policy-controlled view of an underlying response. Vocabulary only; AI1 creates no filtered-response type, mode, or pretend security decision. |
| Network/fetch failure | Inability to admit, execute, or complete the operation: for example unsupported operation, transport failure, redirect failure, cancellation, malformed framing or body-limit failure. Separate from an HTTP status and consumer rejection. |

Fetch's standards vocabulary includes an internal error-response representation;
that does not make status zero a real HTTP status. Borrowser's internal typed
failure contract need not reproduce web-exposed Response objects or fake HTTP
responses.

**AI contract:** preserve each identity's existing meaning:

- AH `ResourceKind` is producer purpose. `core_types::ResourceKind::{Html,Css,Image}`
  is current runtime routing/limit classification. Neither is automatically a
  complete Fetch destination. Unknown purpose does not justify guessing a route.
- `ResourceIdentityKey` excludes the fragment and preserves AH serialization,
  including query distinctions. It is not request, response, decoded-resource,
  navigation, attachment, or complete HTTP cache identity.
- Current `RequestId` is a navigation generation, qualified by `TabId`.
  Stylesheet requests additionally retain `StylesheetSlotId` and exact issued
  retrieval provenance. A generation alone does not distinguish all subresources.
- Equal resource keys never merge stylesheet attachments or change cascade order.
  Original issued provenance authenticates events; an equivalent URL spelling or
  the redirect destination is not a substitute for the recorded issuance facts.
- Requesting origin is captured at issuance, preserving AH opaque identity through
  existing values. Target origin and document creation origin remain distinct;
  serialized `null`, address equality, or same-origin classification grants no
  execution permission.

## 4. Existing implementation and migration inventory

**Current behavior at the inspected baseline:** these are migration points, not
evidence that the target contracts in later sections are already implemented.

| Path / symbol | Existing behavior and limitation | Existing owner for later implementation |
| --- | --- | --- |
| [`addressing/src/lib.rs`](../../crates/addressing/src/lib.rs): `BrowserUrl::parse`, `resolve`; [`resource.rs`](../../crates/addressing/src/resource.rs) | Typed immutable URLs/addresses and fragmentless keys exist. Capability admits http/https/file for AH operations, independently of Fetch support. | Reuse AH in AI2/AI6; keep URL semantics in addressing. |
| [`core_types/src/lib.rs`](../../crates/core_types/src/lib.rs): `NetworkResponseInfo`, `NetworkErrorKind` | Requested/final URL strings, optional status and content type; coarse errors include `HttpStatus`. No general HTTP header/body/redirect model. | AI2/AI3/AI7/AI8. |
| [`bus/src/lib.rs`](../../crates/bus/src/lib.rs): `FetchStream`, `NetworkStart/Chunk/Done/Error`; [`runtime_net`](../../crates/runtime_net/src/lib.rs): `start_net_runtime` | String input and routed streaming events; cancellation flags keyed by tab/generation. Sends ignore delivery errors; cancellation records remain in the map. No per-resource cancellation or worker acknowledgement protocol. | AI2/AI5/AI7; broader resource lifecycle in AJ/AK. |
| [`net/src/fetch.rs`](../../crates/net/src/fetch.rs): `fetch_stream`, `fetch_stream_with_policy`; [`agent.rs`](../../crates/net/src/agent.rs) | Real GET transport. `ureq` follows redirects internally; configured default is 10. Only final response start is emitted; completion repeats its snapshot, chunks/errors use the original issued string. | AI4/AI5/AI6. |
| [`net/src/limits.rs`](../../crates/net/src/limits.rs): `should_stream_http_status` | HTML/CSS 4xx/5xx bodies stream; image HTTP errors become `NetworkErrorKind::HttpStatus`, losing the common response path. | AI3/AI5/AI7; preserve consumer rejection in AK. |
| [`net/src/stream.rs`](../../crates/net/src/stream.rs): `stream_reader`; [`policy.rs`](../../crates/net/src/policy.rs) | 32 KiB reads, resource byte caps, cancellation checks between reads, no Done after reported read/limit failure. Connect/read/write defaults are 15/30/30 seconds, not a total-operation deadline. Bytes already delivered can precede failure. | AI5/AI7. |
| [`net/src/file.rs`](../../crates/net/src/file.rs): `fetch_file_url`; [`content_type.rs`](../../crates/net/src/content_type.rs) | Existing local-file streaming uses status `None` and extension-derived content type. Manual prefix/path conversion has known query/fragment/path limitations. | AI records compatibility; filesystem/resource hardening remains later scope. |
| [`tab/nav.rs`](../../crates/browser/src/tab/nav.rs): `start_fetch`, `activate_link`; [`page/addressing.rs`](../../crates/browser/src/page/addressing.rs) | Navigation/history and document/base/origin state are Browser-owned. Typed hyperlink admission still hands strings to document fetch issuance. | AI2 integration; full document/navigation lifecycle in AJ. |
| [`tab/html.rs`](../../crates/browser/src/tab/html.rs), [`tab/state.rs`](../../crates/browser/src/tab/state.rs), [`tab/events.rs`](../../crates/browser/src/tab/events.rs) | Start admission and immutable completion checks protect incremental parsing and document state. Parser completion is distinct; this is not a complete independent transport-terminal state machine. | AI producer contracts; broader document lifecycle in AJ. |
| [`tab/discovery.rs`](../../crates/browser/src/tab/discovery.rs), [`tab/css.rs`](../../crates/browser/src/tab/css.rs), [`document_style.rs`](../../crates/browser/src/document_style.rs) | Actual issued records, exact keys/generations/slots, MIME admission and runtime phases protect installation. CSS currently normalizes content type locally. AH8 retains admitted final-response bases. | AI2/AI8 integration; preserve AH6/AH8. |
| [`tab/image.rs`](../../crates/browser/src/tab/image.rs), [`resources.rs`](../../crates/browser/src/resources.rs): `ResourceManager`; [`page/image_sources.rs`](../../crates/browser/src/page/image_sources.rs) | String image keys, buffered bytes and async decode; image start metadata is not admitted, typed error details are discarded, cancellation matches `"cancelled"`. Old-generation filtering can leave cancelled Loading state stranded. | AI7 typed failure handoff; broader image lifecycle in AK. |
| [`net/src/log.rs`](../../crates/net/src/log.rs), [`tab/status.rs`](../../crates/browser/src/tab/status.rs) | Raw legacy URLs/error details can reach diagnostics. AH redaction does not make these paths safe automatically. | AI2/AI3/AI7/AI9 diagnostics, not an AI1 runtime fix. |

The [`Cargo.lock`](../../Cargo.lock) backend is `ureq 2.12.1`, with gzip enabled
through the workspace declaration in [`Cargo.toml`](../../Cargo.toml). Inspection
of that selected dependency shows automatic redirect decisions and URL-userinfo
Basic authentication, silent skipping of malformed header lines, and removal of
`Content-Encoding`/`Content-Length` when automatically decompressing gzip.
Consequently, present metadata is not a complete byte-preserving HTTP record.
The current byte cap counts what the selected reader returns; it caps decoded
gzip output when that decoder is active, without establishing universal encoding
support or separate encoded-size accounting. AI5 must qualify/adapt backend
behavior against the intended contract rather than equate library success with
conformance. AI1 neither replaces the HTTP library nor adds an HTTP parser.

## 5. Fetch and response lifecycle

**AI contract:**

```text
issued -> response metadata -> ordered chunks -> completed | failed/cancelled
   \---------- failure/cancellation before metadata ----------/
```

1. Issuance captures actual operation context and correlation. Queue acceptance
   is not response admission or successful execution. Failed dispatch must settle
   the owning operation without phantom work.
2. Admit one immutable metadata snapshot for the final selected response before
   consuming its body. Redirect attempts remain internal Fetch progress. Empty
   bodies are valid. Under supported follow behavior, a redirect response with no
   `Location` is itself selected; invalid/rejected redirect targets still fail
   according to section 7.
3. Deliver bytes incrementally, in order, under exact issuance correlation.
   Chunk boundaries do not determine HTML/CSS/UTF-8 semantics. A bounded collector
   is permissible for a consumer needing a complete resource, but a fully buffered
   `Vec<u8>` is not the universal Fetch contract.
4. Completion requires backend-verified end-of-body and consistent accounting.
   Completing the body cannot replace admitted URL/status/header provenance.
5. Failure may occur before headers or after partial delivery. Preserve acquired
   metadata as failure context; do not reinterpret its status as a transport
   category or publish truncated bytes as a completed resource.
6. Settle a network operation once. Late/duplicate delivery cannot reopen it;
   cancellation after completion cannot reverse completion. Cancellation racing
   completion is resolved by the owning operation's accepted terminal transition.
   Delivery loss or worker shutdown is not successful EOF. Current ignored sends
   and lack of acknowledgements remain explicit limitations, not promised fixes.
7. Distinguish body completion from parser completion, DOM publication, stylesheet
   installation, image decode, and consumer acceptance. Rust async delivery and
   generation/attachment filtering remain necessary.

**Preserved AH/Browser invariants:**

- HTML requires admitted metadata before parser input; chunks use exact original
  issued provenance, including after redirects. Completion validates the same
  snapshot. Failure may retain already committed previews without claiming a
  completed load or changing retained origin/base/history through late events.
- CSS registration precedes dispatch. Matching tab/generation/issued key/slot,
  immutable metadata, fixed MIME admission, validated network completion, and
  expected CSS runtime progress are all required for installation. Duplicate
  starts cannot replace the response. Unknown/stale/retired events are inert;
  invalid unfinished events settle the owning attachment.
- Attachment order, identity and lifetime remain Browser-owned. AH8's nonempty
  admitted final response URL supplies the external stylesheet base. Missing or
  invalid provenance remains explicitly unavailable/failed; request URL and
  document base are not fallbacks. A base-only change does not retarget a loaded
  attachment.
- CSS-local failure clears pending state and its buffer without cancelling sibling
  transfers. `CssAbort` is buffer retirement, not per-resource HTTP cancellation.
  Existing document failure retains loaded previews and validated network
  completions awaiting CSS under AH rules.
- Images buffer before decoding and clear partial data on failure. Preserve their
  current selected bindings/lookup identity until coordinated AK work changes it;
  do not claim they already have CSS's admission strength.

**HTTP outcome distinction:** acquiring a valid 404/500 response is HTTP response
acquisition. Its body can complete or subsequently fail. A loader may reject
status/MIME suitability or fail parsing/decoding; these are consumer outcomes.
Fetch exposes the HTTP facts consistently, while consumers own resource-specific
acceptance. The current image `HttpStatus` path requires eventual migration.

## 6. Conceptual request, response, and backend contracts

These are minimum logical contracts, not Rust declarations or six mandatory
types. Each is justified by existing limitations and actual consumers. Reuse
existing owned values; add no placeholder modes/configuration for deferred work.

| Contract / semantic owner | Concrete need | Minimum preserved information and reuse | Implementation |
| --- | --- | --- | --- |
| Request / Fetch | Document/CSS/image string commands omit typed context. | AH `ResourceAddress`/`BrowserUrl`, explicit method and no-body baseline, ordered headers, producer purpose, actual available initiator/origin context, supported redirect behavior and correlation. Reuse `Origin`, tab/generation/slot facts; do not guess missing purpose. | AI2. |
| Response and body progress / Fetch | HTML streams; CSS/images require validated completion before install/decode. | Evolve `NetworkResponseInfo` and `NetEvent`: requested/current/final provenance and hop trace, HTTP status/reason where applicable, headers, centralized available metadata, chunks, byte units and terminal outcome. File results remain distinguishable. | AI3, AI5, AI7, AI8. |
| Ordered HTTP fields / Fetch | One content-type string cannot preserve repeated fields or extraction provenance. | Canonical names, ordered byte-preserving values, bounded deterministic representation; field extraction independent of consumers. | AI3/AI8. |
| Individual attempt / backend under Fetch | Concrete library execution hides hops and prevents isolated orchestration tests. | Current target/method/headers, execution limits/cancellation, response facts/body progress or failure. Reuse useful `HttpClientPolicy` facts. A minimal AI4 fake uses the same semantics without I/O. | AI4/AI5/AI6. |
| Failure context / Fetch with backend mapping | Coarse transport errors and image cancellation strings lose failure meaning. | Stable category, stage, request correlation, acquired response context and bounded safe diagnostics; evolve `NetworkErrorKind`. | AI7. |
| Debug projection / Fetch | Raw logging is unsafe; requests/responses/hops need regression evidence. | Fixed order/categories and case-local labels over actual facts, content redaction and output bounds. No public wire schema or independent semantic model. | AI2/AI3/AI6/AI7/AI9. |

Consumer handoffs must not leak `ureq` types or require loaders to call a backend
directly. A status representation may reuse its existing integer facts; vocabulary
alone does not justify a public wrapper. Individual attempts need not become a
second public request type. Cancellation mechanics and physical storage are
selected by their implementation issues, without a generic streaming framework.

## 7. Supported HTTP and scheme behavior

**Initial AI subset:** GET with no request body is required. HEAD is conditional
on a concrete AI2 consumer and correct bodyless modeling; otherwise it is explicitly
unsupported. Other methods/body modes and unimplemented redirect policies fail
admission deterministically. No Fetch-level automatic retry is introduced;
AI5 must disclose any backend-internal replay behavior.

| Scheme/context | AI contract and compatibility boundary |
| --- | --- |
| HTTP / HTTPS | Typed HTTP execution through the controlled backend. AH capability still does not grant admission or policy permission. |
| Direct `file:` resource retrieval | Preserve existing separate local-resource handling, with no fabricated HTTP status/fields. Extension-derived MIME is a local-loader fact. Existing path/query/fragment limitations and AH opaque file origins remain explicit; no new filesystem authorization. |
| HTTP redirect to `file:` or another non-HTTP(S) scheme | Unsupported target; do not dispatch to the local-file loader. |
| `data:`, `blob:`, `about:`, `ws:`, `wss:`, `ftp:`, `javascript:`, unknown schemes | No AI execution route. AH's deferred/reserved/unsupported classifications and representable parsed values remain unchanged. |

**Standards baseline for the intended redirect subset:** follow 301/302/303/307/308;
missing `Location` returns the response, invalid location fails. Resolve relative
targets against the current response URL through AH and inherit an absent fragment;
an explicit empty fragment stays empty. Follow at most 20 redirects, then fail
before another attempt. GET/HEAD preserve their methods here. Other 3xx statuses
are not followed. [Fetch redirect processing](https://fetch.spec.whatwg.org/#http-redirect-fetch)

**AI decisions and qualifications:** AI6 includes 308, reconciling its existing
test requirement with the omission in its status bullet list. Reject repeated
`Location` fields, even identical ones, as a conservative Borrowser subset
decision with compatibility implications; do not describe this as universal
browser recovery behavior. An empty value is not absence: resolve it normally.
Use the redirect budget to bound loops; repeated URLs alone do not prove a loop.
Keep per-hop URL/status and the final selected URL, without sending intermediate
bodies to a resource consumer. Release intermediate response resources through
bounded backend handling. Separate complete URLs from AH's fragmentless network
projection. A malformed location means parse/extraction failure, not every
recoverable AH URL diagnostic. No transport-local URL repair or base guessing.

Current library following must give way to observable individual attempts under
Fetch coordination in AI5/AI6. Merely raising `ureq`'s current configured limit of
10 to 20 would not implement this contract or its counting semantics.

**Redirect header handling:** origin changes do not justify blanket removal of
ordinary caller metadata. Fetch removes `Authorization` across origins; RFC 9110
distinguishes field regeneration from security-based removal.
[Fetch](https://fetch.spec.whatwg.org/#http-redirect-fetch),
[RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.4).

| Header category | AI contract for each hop |
| --- | --- |
| Ordinary admitted metadata, such as `Accept` / `Accept-Language` | Preserve unless a specific method/resource/header rule changes it; preserve meaningful repetition order. |
| `Authorization` and other sensitive authorization metadata | Remove authorization before a cross-origin attempt, including HTTPS-to-HTTP; never restore it from the original request on later hops. Do not forward origin-bound secrets to another origin. Explicit authentication headers are unsupported in the initial AI admission subset. Additional credential-bearing fields require concrete rules/tests before admission broadens. |
| Target-derived fields, including `Host` | Derive from the current target; never copy the old authority as a caller override. |
| Cookies, proxy authentication, referrer information | Dedicated policies are deferred. Initial AI admits no explicit `Cookie`, `Proxy-Authorization`, or `Referer` behavior and generates none implicitly. Response `Set-Cookie` values can be retained inertly; they create no cookie store or subsequent request fields. Captured origin/initiator facts do not authorize policy-header generation. |
| Connection, framing, encoding negotiation, and other execution-derived fields | Reevaluate/regenerate against the current connection, method, actual body and backend capabilities. Cache-generated validators and future policy fields require their owners' coordination, not indiscriminate copying. |

AI2 documents the admitted request fields and their handling; unfamiliar extension
names are not automatically safe. Adding a field carrying secrets or binding a
resource/origin needs an explicit rule, not a generic sensitivity registry. AI6
applies these rules while retaining harmless admitted metadata. HTTP-to-HTTPS and
HTTPS-to-HTTP are protocol-supported with the same credential protections; that
does not implement mixed-content, HSTS, CORS, or full browser security policy.

**URL userinfo and authentication admission:** AH continues to parse/represent
valid URLs containing username/password components. AI neither turns them into
syntax failures nor silently removes credentials to make admission succeed. The
initial AI subset rejects credential-bearing HTTP/HTTPS requests and redirect
targets before backend execution as unsupported admission. Parsing userinfo never
implicitly enables Basic authentication. Prevent `ureq`'s current automatic
userinfo-to-Authorization behavior from bypassing that boundary in AI2/AI5/AI6.
Use AH's normalized credential components; do not recover discarded authored
delimiters by scanning URL strings or change AH equality to enforce admission.
RFC 9110 separately restricts userinfo in HTTP(S) message URI references and
recommends rejection from untrusted sources; it does not require AH to stop
representing such URLs.
[RFC 9110 userinfo](https://www.rfc-editor.org/rfc/rfc9110.html#section-4.2.4)

This admission limitation is not a permanent ban on authentication. A future
concrete feature must specify credential sourcing, authority/protection scope,
redirect handling, redaction and tests before widening support. AI1 implements
no authentication, credential mode, challenge handling or user-facing UI.

## 8. Headers, bodies, and resource guardrails

**AI contract:** HTTP fields are a bounded ordered collection with case-insensitive
names, canonical lowercase name representation and byte-preserving values. Keep
repetitions and same-name order; do not universally comma-join, including
`Set-Cookie`, or force arbitrary values through lossy UTF-8. Reject invalid field
construction and unsafe malformed fields at the HTTP boundary. Stable debug order
is a projection of the same admitted list, not permission to reorder values.
[RFC 9110 fields](https://www.rfc-editor.org/rfc/rfc9110.html#section-5)

Fetch owns field-specific extraction for content type/MIME, charset, declared
length and status classification. AI8 defines missing/invalid/repeated metadata
outcomes centrally; loaders must not each split raw content-type strings. An
unusable optional MIME value is different from invalid body framing. Current
CSS's local extraction remains a migration point; no MIME sniffing or document
character decoding is added here.

**Framing requirement:** invalid/conflicting framing, premature termination and
incomplete transfer coding cannot produce completed bodies. Respect bodyless
response/method rules and distinguish valid close-delimited completion from
failure. Delegate wire parsing to the qualified backend, with typed error mapping;
do not add a competing HTTP parser in loaders.
[RFC 9112 framing](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3)

**AI resource contract:**

- Deliver incrementally under explicit limits. Existing
  [`tools::common`](../../crates/tools/src/common.rs) caps are the starting point:
  document 10 MiB, stylesheet 2 MiB, image 20 MiB. Exact-limit completion differs
  from an over-limit failure; partial bytes may already have been delivered.
- Count delivered content-decoded bytes as they arrive. Keep encoded-byte
  accounting, declared encoded length and delivered decoded count distinct where
  available; unknown facts stay unknown. `Content-Length` neither proves body
  completeness nor bounds decompression output. No unbounded allocation based on
  an untrusted declared length.
- Supported encodings are handled once at the documented backend/Fetch boundary;
  unsupported encodings fail explicitly. Never pass opaque compressed bytes as
  allegedly decoded resource content. Preserve declared encoding/length provenance
  separately from transformed delivery facts when the implementation can expose
  it; disclose current unavailable facts rather than inventing them. AI5 owns the
  backend adaptation required by today's gzip header removal.
- Size, timeout, read, framing, encoding and cancellation failures terminate the
  affected operation. Clear resource buffers where complete bytes are required;
  HTML's committed-preview policy remains Browser-owned. No successful-truncation
  flag or success-shaped fallback replaces failure.
- Bound header processing and redirect work; qualify actual backend limits in
  AI5. Current 15/30/30-second connect/read/write settings are not a total fetch
  deadline. Cancellation checks between reads do not promise immediate interruption
  of DNS/connect/blocking reads; AI7 records supported latency and terminal races.
- Resource byte limits do not bound process-wide memory, in-flight requests,
  queued bytes, thread count or decode allocation. Current unbounded channels,
  thread-per-fetch execution and retained cancellation records remain limitations.
  Image pixel/decode protections belong to image loading; existing pixel checks
  after decoding do not establish a pre-allocation memory bound. No scheduler or
  new global-limit framework is introduced by AI1.

## 9. Diagnostics and internal debug output

**AI contract:** Fetch owns deterministic production facts for request admission,
backend route/result, response metadata, hop decisions, final provenance and
failure stage. Use bounded output, fixed categories and field order, and
deterministic case-local labels. Exclude wall-clock timings, pointer identities,
unordered iteration and environment-specific error text from regression output.

AH's existing redaction is the minimum boundary. Default debug output withholds
URL contents (including userinfo, hosts, paths, queries, fragments and local paths),
header values, body bytes and server-controlled reason/error details. Redact
authorization/cookie/referrer data even when nested in another value. Sanitize
backend errors before exposure: their strings may repeat URLs and credentials.
Do not replace sensitive content with recoverable fingerprints or sensitive
string lengths. Safe structural facts/status categories and necessary resource
byte accounting remain distinct from private URL/header contents.

Exact semantic assertions over independently authored synthetic fixtures remain
possible under controlled test evidence. A display-redacted address never becomes
a semantic key. Internal debug snapshots are regression contracts, not public
APIs, persistent request identifiers, or transport formats. AI1 changes no logging
path, adds no serializer and claims no redaction fix in current legacy diagnostics.

## 10. Consumer handoffs and later integration

| Consumer | Fetch provides | Consumer retains |
| --- | --- | --- |
| Documents (AJ) | Admitted response provenance, HTTP facts/central metadata, incremental bytes and verified terminal outcome | Creation context, document URL/base/origin lifetime, history association, parser/publication progress and preview policy. |
| Stylesheets | Same response/body contract with exact issuance/slot correlation and final URL | AH6 attachment identity/order, AH8 retained response base, MIME applicability, CSS byte/text assembly, parsing/cascade and installation. |
| Images/resources (AK) | Response facts, bounded bytes, structured failure/completion | Selected binding, lookup/decoded-resource identity, format support, buffering/decode and intrinsic render invalidation. |
| Fonts and later static consumers | Central metadata and typed body outcome when their actual loaders exist | Format decoding, selection, resource-specific acceptance and lifecycle. No font loader is implied by AH purpose classification. |
| HTTP cache (AT) | Coordinated request/response/provenance inputs | Storage/reuse/validation/partition policy composed with Fetch coordination; resource keys alone never authorize reuse. |
| Security/authentication features | Actual captured initiator/requesting-origin context, target/hop and response facts | Concrete policy and tests before adding admission/exposure behavior. Missing context stays missing, not guessed from target origin. |

The milestone's cache layer boundary means separate capability ownership, not an
architectural requirement for an independent wrapper outside Fetch semantics.
Future AT integration must compose with Fetch; no cache or cookie stub is added.
[Fetch network-or-cache coordination](https://fetch.spec.whatwg.org/#http-network-or-cache-fetch)

Preserve actual needed context without placeholder credential/cache modes,
partition managers, referrer-policy engines, CORS objects, filtered-response
hierarchies, integrity pipelines or priority schedulers. A concrete consumer or
implemented policy must justify any later abstraction.

## 11. Architectural constraints and exclusions

**AI contract against duplication:** future work must use Fetch's request,
response, redirect, header extraction and failure boundaries. Do not add raw
HTTP/library-response interpretation, independent status-to-network-error rules,
redirect loops, MIME-header splitting, or URL joining in document/CSS/image/font
loaders, Layout, Paint or tooling. Authored HTML/CSS syntax processing and
resource-specific acceptance stay with their existing owners. Existing string
transport is the recorded migration baseline, not permission to expand it.

No new type/trait is justified solely by future possibility. Six concepts do not
require six Rust types. Reuse/evolve existing implementations and add only the
minimal seam needed by real consumers and deterministic tests. An implementation
phase is not a GitHub issue; use the existing issue structure below.

**Deliberate exclusions:** no JavaScript `fetch()` API, XMLHttpRequest, service
workers, or JavaScript event-loop/promise/task/microtask integration. Existing
Rust runtime workers, commands, network events, platform event routing and
asynchronous resource handling remain necessary and supported.

AI1 implements no networking behavior. Milestone AI additionally does not claim
HTTP cache, cookies, storage, full CORS, CSP, mixed-content enforcement, HSTS,
authentication UI, full authentication policy, full navigation/history/session
lifecycle, downloads UI, HTTP/2, HTTP/3, WebSockets or full browser security policy.
No MIME sniffing, new decompression/character decoding, generic streaming
framework, speculative middleware, plugin registry, new crate, CI/fixture/schema
change or unrelated runtime refactor is introduced by AI1.

## 12. Evidence, acceptance mapping, and follow-up ownership

Existing tests are source evidence for current behavior, not newly executed
qualification of this document's future contracts:

| Evidence | Current contract demonstrated by the named tests |
| --- | --- |
| [`net/tests/fetch.rs`](../../crates/net/src/tests/fetch.rs): `follows_redirects_and_reports_final_url` | Existing backend following and requested/final metadata; not AI's full redirect algorithm. |
| [`net/tests/status.rs`](../../crates/net/src/tests/status.rs): `streams_html_404_body_but_classifies_image_404_as_http_error` | Existing status-handling difference requiring migration. |
| [`net/tests/limits.rs`](../../crates/net/src/tests/limits.rs): `allows_document_response_exactly_at_limit`, `oversized_document_response_emits_resource_limit_without_done`, stylesheet/image counterparts | Existing caps, partial delivery and no successful Done after a reported limit failure. |
| [`document_lifecycle.rs`](../../crates/browser/src/tab/tests/document_lifecycle.rs): `document_lifecycle_response_admission_is_required_before_chunk_or_done`, `document_lifecycle_response_completion_cannot_replace_accepted_provenance`, `document_lifecycle_published_preview_remains_active_during_parsing_and_after_failure` | HTML admission, immutable response facts, and retained previews. Other cases cover exact chunks and file-origin lifetime. |
| [`css_provenance.rs`](../../crates/browser/src/tab/tests/css_provenance.rs): `ah6_css_done_requires_the_immutable_response_snapshot`, `ah6_css_runtime_delivery_failures_settle_without_reopening_or_global_cancellation`, `ah6_css_fragment_keys_preserve_attachment_isolation_and_original_retirement_generation` | Exact issued provenance, local settlement, runtime phases and attachment isolation. Other cases cover stale/duplicate events and debug redaction. |
| [`css_resources.rs`](../../crates/browser/src/tab/tests/css_resources.rs): `css_resource_response_base_is_distinct_from_request_identity`, `css_resource_unavailable_bases_do_not_guess_and_keep_failures_separate` | AH8 final-response bases and explicit unavailable/invalid provenance. |
| [`network.rs`](../../crates/browser/src/tab/tests/network.rs): `parser_completion_is_distinct_from_response_and_publication_and_failure_stays_latched` | Response, parser and publication outcomes are separate. |
| [AG guide](../../crates/conformance/README.md), [AH9](ah9-addressing-conformance-fixtures-and-snapshots.md) | Offline injected response/addressing observations do not exercise redirect execution or qualify the real backend. Existing phase subresource restrictions remain intact. |

AI1 documentation validation consists of requirement/exit traceability, inspection
of referenced production symbols and behavior, relative path/anchor checks,
standards consistency, complete diff/scope review and `git diff --check`.
Artificial tests reproducing Markdown are unnecessary. Rust subsystem and runtime
smoke tests cannot prove unimplemented Fetch behavior. Full CI is separate
PR/merge qualification, not evidence of these future semantics; it is not needed
for this documentation-only working-tree acceptance. Actual commands/results
belong in the review packet; no previous test result is presented as a fresh run.

| AI1 requirement / exit criterion | Contract evidence |
| --- | --- |
| Add Milestone AI architecture documentation; architecture documented | This twelve-section contract and architecture overview link. |
| Fetch owns address-to-request construction, request metadata and execution coordination | Sections 2, 4 and 6. |
| Fetch owns redirects, response metadata/body representation, errors, diagnostics and deterministic debug output | Sections 2 and 5–9. |
| AH provides normalized resource addresses; AH relationship documented | Sections 1–4 and 6; existing AH ownership is preserved. |
| Future document loading consumes document responses without owning HTTP | Sections 2, 5, 10 and 11. |
| Future stylesheet/image/font loading consumes subresource responses without owning HTTP | Sections 2, 5, 10 and 11. |
| Future cache layer boundary; cookies excluded; security context preserved without enforcement | Sections 7, 10 and 11; cache capability composes with Fetch rather than mandating an outside wrapper. |
| Distinguish resource address, fetch request, network request, HTTP response, future filtered response and network error | Sections 3 and 6. |
| Define AI schemes, methods and redirect behavior | Section 7, including file compatibility, GET baseline and 308. |
| Define body-size/resource guardrail philosophy | Sections 5 and 8, including partial failure and process-wide limitations. |
| No JavaScript fetch, XHR, service workers or JS event-loop integration | Section 11; existing Rust delivery is explicitly retained. |
| Tests or docs prevent raw HTTP/string handling spreading | Sections 4, 6 and 11 provide the baseline and concrete ownership constraints. |
| Explicit ownership and clear future document/resource response contract | Sections 2, 5, 6 and 10. |
| Unsupported features explicit; closeable without network execution implementation | Sections 1, 11 and 12; contract completion is distinct from implementation/merge/milestone completion. |

Follow-up implementation stays within existing deliverables:

| Existing issue / milestone | Responsibility and executable evidence |
| --- | --- |
| [AI2 #1122](https://github.com/joris97jansen/borrowser/issues/1122) | Typed requests, actual context, method/scheme/header/userinfo admission, defaults and request debug tests. |
| [AI3 #1123](https://github.com/joris97jansen/borrowser/issues/1123) | Typed status/headers/response/body contracts; repeated-field, response-versus-error and representation tests. |
| [AI4 #1124](https://github.com/joris97jansen/borrowser/issues/1124) | Minimal backend seam and deterministic no-I/O fake, including missing-route/failure coverage. |
| [AI5 #1125](https://github.com/joris97jansen/borrowser/issues/1125) | Real backend qualification/adaptation, TLS, framing/encoding/byte limits, hidden authentication/redirect prevention, local server evidence. |
| [AI6 #1126](https://github.com/joris97jansen/borrowser/issues/1126) | Supported redirects including 308, relative/fragment/Location cases, per-hop headers and credential boundaries, 20-hop count/loops and final provenance. |
| [AI7 #1127](https://github.com/joris97jansen/borrowser/issues/1127) | Typed staged failures, cancellation/timeouts, terminal races/partial failure and explicit retry behavior. |
| [AI8 #1128](https://github.com/joris97jansen/borrowser/issues/1128) | Central MIME/charset/length extraction, absent/invalid/repeated-field outcomes and loader handoff tests. |
| [AI9 #1129](https://github.com/joris97jansen/borrowser/issues/1129) | Deterministic fake/local networking fixtures, snapshots, AG discovery/reporting and unsupported coverage; no public internet dependency. |
| [AI10 #1130](https://github.com/joris97jansen/borrowser/issues/1130) | Reconcile implemented scope, qualification and remaining gaps before milestone closeout. |
| AJ / AK / AT and later security features | Broader document/resource lifecycle, image decode/identity, HTTP cache/cookies/hardening and policy features. Preserve this response handoff without implementing those capabilities in AI1. |

The [feature-gap tracker](../engine-feature-gap-tracker.md) records an authored
AI1 contract separately from pending executable AI scope. Existing addressing and
networking limitations remain open. No new issue is required for an implementation
step, a prospective abstraction, or a qualification phase.

## AI3 implementation reference

[AI3 typed response/status/header/body delivery](ai3-typed-fetch-response-status-headers-body.md)
evolves the production response events and consumers under this contract. Its
backend-fidelity and consumer-lifecycle qualifications are explicit. Historical
implementation observations above remain baseline evidence, not a claim of
current end-to-end conformance or milestone completion.

## AI4 implementation reference

[AI4 response acquisition and deterministic fake networking](ai4-network-backend-and-deterministic-fake-network.md)
introduces a private selected-response acquisition seam exercised by the existing
production Fetch lifecycle. Real ureq acquisition still follows redirects internally;
this is not the observable individual-attempt contract retained by AI5/AI6.
The fake uses AH retrieval identity, existing request/response/body/error types,
and shared response validation, streaming, cancellation, limits and terminal events.
Direct local-file handling stays separate. No registry, runtime redesign, new
timeout policy, credential-hop protection, or networking conformance expansion
is claimed. The implementation reference records qualification independently.
