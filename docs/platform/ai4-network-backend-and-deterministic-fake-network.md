# AI4: HTTP Response Acquisition And Deterministic Fake Networking

Issue: [#1124](https://github.com/joris97jansen/borrowser/issues/1124).
Milestone AI — Fetch, HTTP Networking, Redirects, and Response Handling.
Implementation baseline: master `212690a6267762d740f9e7050b86e45675f7e756`.

## Scope and authority

[AI1](ai1-fetch-http-networking-response-contract.md),
[AI2](ai2-typed-fetch-request-construction.md),
[AI3](ai3-typed-fetch-response-status-headers-body.md), and
[AH6](ah6-resource-addresses-and-identity-keys.md) govern this change.

AI4 makes the existing production Fetch lifecycle testable with no-I/O
acquisition. Its private `HttpResponseBackend` acquires a **selected response**,
not necessarily one observable HTTP attempt. The current ureq backend still
follows redirects internally. This implementation does not establish AI5/AI6
attempt visibility, per-hop admission, or credential protection.

Implementation, local qualification, hosted CI, PR merge, and GitHub closure
are separate states. This contract does not establish publication or milestone
completion.

## Ownership and dependency direction

`net` owns acquisition, response processing, fake fixtures, and network errors.
Addressing owns normalization and fragmentless retrieval identity. Browser
retains actual issuance context, navigation/document lifetime, consumer response
admission and attachment identity. Runtime/bus retain command/event routing and
cancellation correlation. CSS, Layout and Paint own no transport policy.

```text
typed admitted FetchRequest
  -> net HTTP worker -> HttpResponseBackend::acquire
                         UreqBackend | FakeNetwork
  <- NetworkResponseInfo + owned fallible Read | acquisition error
  -> shared response validation -> Start -> bounded chunks -> Done | Error
```

The private backend receives a borrowed request and existing execution route.
It cannot publish NetEvents. `AcquiredHttpResponse` only bundles existing AI3
metadata with an unread body source. `HttpAcquireError` carries either an
existing response-admission error or an existing network category and diagnostic.
There is no second request/metadata model, backend registry or factory hierarchy.

Static generic dispatch moves a concrete backend into the existing worker.
The trait requires Send and an owned lifetime, not Sync. The boxed Read is
created and consumed inside that worker and borrows no request/fixture lifetime;
it needs no Send/Sync requirement. The immutable fake is itself Send + Sync.

## Shared production lifecycle

`fetch_http_with_backend` validates route and HTTP(S) scheme before spawning.
It retains `legacy_issued_url`: HTML/images keep complete normalized original
spelling, while CSS keeps its fragmentless issued spelling. A fake route key
never replaces event correlation or original provenance.

The worker checks pre-execution cancellation, acquires a response, and applies
`NetworkResponseInfo::validate_backend_http` before Start. Real extraction uses
the moved, otherwise preserved `response_metadata`; fake acquisition constructs
the same `NetworkResponseInfo::http` with HttpStatus and ordered HttpHeaders.
Both paths share response-admission error mapping.

Start clones AI3's immutable metadata. The same production `stream_reader`
enforces cancellation checkpoints, byte ordering, 32 KiB reads and route-derived
10/2/20 MiB document/stylesheet/image delivery caps. Exact-limit EOF succeeds;
observing another byte fails. Read/limit/cancellation failures retain acquired
metadata and known BodyProgress, with no Done. Success repeats the immutable
metadata with actual progress. Acquiring headers never establishes body completion.

204/205/304 discard the supplied reader and process an empty body through the
same reader/cancellation path. This is existing consumer-visible null-body
behavior, not HTTP wire-framing or connection-reuse qualification. Finite fixture
buffers do not create a process-wide memory cap. Content-Length is neither an
allocation request nor proof of body completion.

Default `fetch_stream` remains the real entry. Its policy wrapper dispatches
direct file resources separately with the existing cancellation/file behavior.
`UreqBackend::new` only stores policy. Agent/native TLS initialization occurs
inside real HTTP acquisition after the pre-cancellation check; file loads no
longer initialize an unused HTTP agent. The file loader and shared reader are
unchanged. No fake filesystem is introduced.

## Fixture API and route identity

`net::fake` exports `FakeNetworkBuilder`, `FakeNetwork`, `FakeResponse`, and
`FakeFixtureError` for concrete test use; the backend trait remains private.

- Register typed BrowserUrl plus RequestMethod::Get. Derive registration keys
  through AH's ResourceAddress retrieval projection; lookup uses the admitted
  request's `transport_target` and method. There is no new parser, query sorting,
  percent decoding, string splitting or filesystem canonicalization.
- Query order/repetition/escaping and absent-versus-empty query remain significant.
  Fragment differences share a route while remaining in original request facts.
- Registration accepts only HTTP(S). It is fixture construction, not request
  admission; AI2's actual credential/method/header admission remains authoritative.
  HEAD and other methods remain unsupported.
- Duplicates, including normalized equivalents and response/error replacements,
  fail with DuplicateRoute before modifying the existing outcome.
- `build` consumes the builder and freezes a small linear table in an immutable
  Arc slice. Clones share only fixture data. There is no interior mutability,
  response queue, call-order dependency, retry state, or lock.
- Every execution constructs facts from the actual request and creates a fresh
  body cursor. Sharing a route does not share request IDs, provenance, attachment
  identity or resource lifetime. This is **not an HTTP cache key or cache**.

FakeResponse takes a typed status, existing ordered bounded HttpHeaders, owned
binary bytes and an explicit `Option<String>` reported response URL. That raw
report is preserved and parsed by the existing AI3 constructor. Missing, empty,
invalid and unsupported-scheme reports reach shared validation without repair or
request-URL fallback. An informational status reaches existing constructor
rejection. Invalid status/header construction still uses AI3's validated types.

404/500 are response facts and can complete bodies on every route. Location is
an ordinary retained header; the fake never joins or follows it. Supplying a
different reported URL is fixture data, not evidence of redirect execution.

## Failures and determinism

Missing routes produce Transport with `fake network route not found`.
`register_network_error` produces Transport with `fake network failure`.
Both are pre-response failures with no invented metadata, progress, or HTTP status.
Results are repeatable and there is no real-backend fallback.

`with_read_failure_after(n)` supplies a narrowly controlled fallible reader.
Offsets from zero through body length are valid; larger offsets fail fixture
configuration. The reader delivers exactly the configured prefix and then returns
a fixed I/O error, mapped by the existing reader to Read. Offset equal to length
fails instead of EOF. Repeated execution restarts at byte zero. Cancellation at
the next reader checkpoint takes its existing precedence over another read.

This tests AI3's existing streaming failure contract without introducing AI7's
staged failure taxonomy or timeout policy. No clock, artificial sleep, timer,
scheduler, generic fault language, or retry framework exists. Deterministic
fixtures do not promise ordering between independently scheduled Fetch workers.

Default Debug on new fake values exposes bounded structural facts only. Route
URLs, reported URL contents, arbitrary header names/values and body contents are
withheld. Existing backend logging is not claimed globally sanitized.

## How tests use fake networking

1. Parse fixture targets through Addressing and build ordinary AI2 requests.
2. Register FakeResponses or fixed pre-response failures, then freeze the builder.
3. Call `FakeNetwork::fetch_stream` with the actual request, compatible route,
   existing cancellation token, and callback.
4. Observe production NetEvents through callback-owned channels. Collect until
   sender disconnection and assert the complete lifecycle and terminal count.
   A deadline detects a hung test; a timeout never establishes successful closure.
5. Trigger cancellation from a Start/Chunk callback for an explicit causal boundary.

The executable module rustdoc demonstrates registration. Integration tests in
`crates/net/tests/fake_network.rs` exercise full production execution. Private
backend tests verify cancellation/admission before acquisition and compare a
real loopback response with independently supplied equivalent fake facts.

The fake path is closed: fake facade -> HTTP-only worker -> immutable lookup,
typed metadata and memory reader. It contains no call to agent/TLS initialization,
socket/DNS APIs, file opening, or file dispatch. Fake file requests are rejected
synchronously with UnsupportedScheme, and mismatched routes retain the existing
admission precedence. HTTP/HTTPS .invalid tests and loopback request counts
support this source-level guarantee; .invalid success alone is not a DNS-isolation
proof. The memory-only integration test target passes inside the execution sandbox;
real loopback tests require local socket permission.

## Preserved qualifications and later ownership

- AI5 #1125 retains real HTTP/TLS, framing/encoding, field fidelity and hidden
  execution qualification. Existing ureq transformations and transport diagnostics
  remain explicit limitations.
- AI6 #1126 retains observable individual attempts, AH redirect resolution,
  per-hop fields/admission and redirect budgets. Real ureq still follows internally
  with the existing configured default of ten.
- AI2 rejects initial credential-bearing requests, but a redirect can introduce
  credentials and ureq can generate Basic authentication internally. The existing
  loopback characterization is preserved. AI4 does not prevent this or pretend
  a post-response check can prevent an already executed request.
- AI7 #1127 retains detailed failures, timeouts, blocking-I/O cancellation latency,
  terminal races and broader runtime delivery/acknowledgement policy.
- AI8 #1128 retains aggregate metadata interpretation; AI9 #1129 retains AG
  networking integration and fixture/report expansion.
- Browser/consumer policy, HTTP cache/cookies/security, document/navigation
  behavior, JS networking and image decoding remain with their existing owners.

## Validation record

Local candidate qualification (2026-11-07): Rust/Cargo 1.92.0, Darwin arm64,
macOS 27.0.1. All results refer to this working tree based on the commit above.

- `cargo test -p net --test fake_network --locked --offline`: **21 passed**,
  no failures or ignores, inside the execution sandbox without socket permission.
- `cargo test -p net --lib ai4_ --locked --offline`: **3 passed**, including
  the paired real/fake lifecycle test, with loopback permission outside the sandbox.
- `cargo test -p net --doc --locked --offline`: **1 passed**.
- `cargo test -p net -p runtime_net -p bus -p addressing -p browser --locked
  --offline`: **506 passed**, no failures or ignores, including existing HTTP,
  TLS, header, file, consumer admission and cancellation regressions.
- `cargo test -p browser --features conformance-observation --locked --offline
  ai2_runtime_local_document_stylesheet_image_and_frame_smoke`: **1 passed**.
- `make ci` passed URL/network dependency checks (including 37 CI-tool tests),
  CI conformance (7 PASS), addressing conformance (22 PASS / 2 explicitly
  unsupported), formatting, and all three workspace lint lanes. It then failed
  in the native macOS Chromium ownership tests: 186 passed, 9 failed, 13 ignored
  in that binary. **The CI aggregate did not pass.**

Both workspace feature lanes completed with `--no-fail-fast` and the same
`--lib --bins --tests --examples --locked --offline` selection. Each failed only
the native conformance binary; every other target completed successfully. The
HTML5 lane additionally used `--features html5`.

All remaining independent CI targets passed: parser conformance, external/fatal
fixtures, DOM/patch goldens, real-page/rawtext regressions, six fuzz smoke runners,
pipeline regressions, WPT tree builder, default/HTML5 debug and release builds,
both benchmark compilations and entity generation checks. These passes do not
retroactively pass the failed aggregate. Fuzz runners retain existing warnings
about sanitizer callbacks/coverage and missing timeout tools; executing the
bounded smoke does not establish sanitizer or coverage qualification.

The unchanged dependency retains one unused-parentheses warning. Release builds
also retain twelve HTML, two CSS and four parser-runtime warnings; those source
files are unchanged. No warning allowance or unrelated fix was added.

After the other qualification runners completed, the native command ran
sequentially on an unmodified archive of baseline master and then the candidate:

```sh
cargo test -p borrowser-conformance --bin borrowser-conformance --locked --offline \
  chromium::tests::native_ -- --test-threads=1
```

Both returned **1 passed, 9 failed, 1 ignored**, with the identical nine failing
wrappers. All twelve native Chromium Rust sources and workspace dependency
lockfiles match baseline. Failures include unavailable live-process executable
identity and cleanup/timeouts. This establishes reproduced baseline failure,
not native macOS qualification or a green CI aggregate. No native assertion,
ignore, allowance, or CI gate changed. No hosted CI, PR merge, or GitHub issue
closure is claimed.

## AI5 implementation qualification (2026-11-08)

The private selected-response seam and deterministic fake remain unchanged in ownership. AI5 makes real acquisition single-attempt without hidden redirect traversal/replay or backend decompression. FakeResponse bytes are content-encoded bytes after transfer framing, so gzip fixtures provide gzip data; shared Fetch applies the same coding/redirect admission and budgets to both backends. Earlier redirect and diagnostic limitations in this document describe AI4 only.

The [AI5 implementation contract](ai5-real-http-response-acquisition-and-bounded-body-integrity.md)
records the implemented subset, exact resource limits, controlled diagnostics,
connection safety and release-abort qualification. It supersedes conflicting
legacy execution qualifications above without rewriting these historical records.
This unmerged candidate is not a hosted-CI, merge, issue-closure or milestone-
completion claim. AI6–AI9 retain their stated independent responsibilities.
